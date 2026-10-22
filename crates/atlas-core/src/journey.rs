//! Read-only travel analysis. Unknown evidence never establishes suitability.
use crate::{parse_date, schedule, string as text};
use chrono::{DateTime, Duration, NaiveDate};
use chrono_tz::Tz;
use serde_json::{json, Value};

fn active(r: &Value) -> bool {
    text(r, "status") != "cancelled"
}
fn transport_night(r: &Value, day: NaiveDate) -> bool {
    if text(r, "kind") != "transport" || text(r, "status") != "confirmed" || r["overnight"] != true
    {
        return false;
    }
    let Ok(start) = DateTime::parse_from_rfc3339(text(r, "departure")) else {
        return false;
    };
    let Ok(end) = DateTime::parse_from_rfc3339(text(r, "arrival")) else {
        return false;
    };
    let Ok(zone) = text(r, "departureTimezone").parse::<Tz>() else {
        return false;
    };
    // Use one timezone for the continuous trip, avoiding date-line reversal.
    let first = start.with_timezone(&zone).date_naive();
    let last = end.with_timezone(&zone).date_naive();
    end > start && (day == first || (day > first && day < last))
}

pub fn gaps(records: &[Value], p: &Value) -> Result<Value, String> {
    let start = parse_date(text(p, "start"))?;
    let end = parse_date(text(p, "end"))?;
    if end <= start || (end - start).num_days() > 1096 {
        return Err("Choose an exclusive end within three years".into());
    }
    let trip = text(p, "tripId");
    let relevant: Vec<_> = records
        .iter()
        .filter(|r| active(r) && (trip.is_empty() || text(r, "tripId") == trip))
        .collect();
    let stays: Vec<_> = relevant
        .iter()
        .copied()
        .filter(|r| text(r, "kind") == "stay")
        .collect();
    let mut nights = vec![];
    let mut ranges: Vec<Value> = vec![];
    let mut day = start;
    while day < end {
        let key = day.to_string();
        let cover: Vec<_> = stays
            .iter()
            .copied()
            .filter(|r| text(r, "checkIn") <= key.as_str() && text(r, "checkOut") > key.as_str())
            .collect();
        let confirmed: Vec<_> = cover
            .iter()
            .copied()
            .filter(|r| text(r, "status") == "confirmed")
            .collect();
        let transit: Vec<_> = relevant
            .iter()
            .copied()
            .filter(|r| transport_night(r, day))
            .collect();
        let status = if confirmed.len() > 1 {
            "overlap"
        } else if !confirmed.is_empty() {
            "confirmed"
        } else if !transit.is_empty() {
            "overnight_transport"
        } else if !cover.is_empty() {
            "tentative"
        } else {
            "gap"
        };
        nights.push(json!({"date":key,"status":status,"stayIds":cover.iter().map(|r|r["id"].clone()).collect::<Vec<_>>(),"transportIds":transit.iter().map(|r|r["id"].clone()).collect::<Vec<_>>(),"noCost":confirmed.iter().any(|r|r["totalMinor"]==0),"vehicle":confirmed.iter().any(|r|text(r,"type")=="vehicle")}));
        if ["gap", "tentative", "overlap"].contains(&status) {
            let next = (day + Duration::days(1)).to_string();
            if let Some(last) = ranges
                .last_mut()
                .filter(|r| text(r, "end") == key && text(r, "status") == status)
            {
                last["end"] = json!(next);
                last["nights"] = json!(last["nights"].as_i64().unwrap() + 1);
            } else {
                let previous = stays
                    .iter()
                    .filter(|r| text(r, "checkOut") <= key.as_str())
                    .max_by_key(|r| text(r, "checkOut"));
                let following = stays
                    .iter()
                    .filter(|r| text(r, "checkIn") > key.as_str())
                    .min_by_key(|r| text(r, "checkIn"));
                ranges.push(json!({"start":key,"end":next,"nights":1,"status":status,"previousStay":previous,"nextStay":following,"target":{"module":"Calendar","date":key}}));
            }
        }
        day += Duration::days(1);
    }
    Ok(
        json!({"start":start.to_string(),"end":end.to_string(),"nights":nights,"ranges":ranges,"basis":"Exclusive checkout; confirmed stays and explicitly intentional overnight transportation cover nights. Tentative stays remain unconfirmed."}),
    )
}

pub fn workspace(records: &[Value], record: &Value) -> Value {
    let mut unknown = vec![];
    let mut conflicts = vec![];
    let mut verified = vec![];
    for (field, label) in [
        ("workspace", "Desk or laptop table"),
        ("internet", "Internet access"),
        ("quiet", "Quiet working environment"),
        ("power", "Power access"),
    ] {
        match record["facilities"][field].as_bool() {
            Some(true) => verified.push(label),
            Some(false) => conflicts.push(label),
            None => unknown.push(label),
        }
    }
    if text(record, "internetEvidence").trim().is_empty() {
        unknown.push("Internet reliability evidence");
    } else {
        verified.push("Owner-recorded internet evidence");
    }
    let alternate = records.iter().find(|r| {
        text(r, "id") == text(record, "alternateWorkspaceId") && text(r, "kind") == "place"
    });
    json!({"status":if !conflicts.is_empty(){"unsuitable"}else if !unknown.is_empty(){"potential"}else{"suitable"},"verified":verified,"unknown":unknown,"conflicts":conflicts,"internetEvidence":record["internetEvidence"],"alternate":alternate,"commuteMinutes":record["workspaceCommuteMinutes"],"openingHours":alternate.map(|r|&r["workspaceHours"]),"note":"Based only on saved, owner-confirmed evidence. Alternate venue hours and commute remain unknown unless entered."})
}

pub fn preview(records: &[Value], settings: &Value, p: &Value) -> Result<Value, String> {
    let r = &p["record"];
    let kind = text(r, "kind");
    let mut warnings: Vec<Value> = vec![];
    let mut value = json!({"estimateOnly":true,"recordsChanged":false,"currency":r["currency"]});
    let end_context: Option<(String, String)>;
    let (date, zone) = if ["stay", "quote"].contains(&kind) {
        let a = parse_date(text(r, "checkIn"))?;
        let b = parse_date(text(r, "checkOut"))?;
        let nights = (b - a).num_days();
        if !(1..=3660).contains(&nights) {
            return Err("Checkout must follow check-in within 3660 nights".into());
        }
        let total = r["totalMinor"]
            .as_i64()
            .ok_or("Enter an exact total in minor units")?;
        value["nights"] = json!(nights);
        value["nightly"] = json!({"numeratorMinor":total,"denominatorNights":nights});
        value["workspace"] = workspace(records, r);
        end_context = Some((b.to_string(), text(r, "timezone").to_owned()));
        (a.to_string(), text(r, "timezone").to_owned())
    } else if kind == "transport" {
        let a = DateTime::parse_from_rfc3339(text(r, "departure"))
            .map_err(|_| "Departure needs an ISO instant with offset")?;
        let b = DateTime::parse_from_rfc3339(text(r, "arrival"))
            .map_err(|_| "Arrival needs an ISO instant with offset")?;
        if b <= a {
            return Err("Arrival must follow departure in actual time".into());
        }
        let departure_zone = text(r, "departureTimezone")
            .parse::<Tz>()
            .map_err(|_| "Invalid departure timezone")?;
        let arrival_zone = text(r, "arrivalTimezone")
            .parse::<Tz>()
            .map_err(|_| "Invalid arrival timezone")?;
        value["durationMinutes"] = json!((b - a).num_minutes());
        value["departureLocal"] = json!(a.with_timezone(&departure_zone).to_rfc3339());
        value["arrivalLocal"] = json!(b.with_timezone(&arrival_zone).to_rfc3339());
        for stay in records.iter().filter(|s| {
            text(s, "kind") == "stay"
                && active(s)
                && (text(s, "id") == text(r, "destinationStayId")
                    || text(s, "id") == text(r, "stayId"))
        }) {
            let checkin = schedule::resolve_local(
                text(stay, "checkIn"),
                stay["checkInTime"].as_str().unwrap_or("15:00"),
                text(stay, "timezone"),
            )?;
            if b.with_timezone(&chrono::Utc) < checkin {
                warnings.push(json!({"type":"early_arrival","recordId":stay["id"],"message":"Arrival precedes recorded check-in; early access is unconfirmed."}));
            }
        }
        end_context = Some((
            b.with_timezone(&arrival_zone).date_naive().to_string(),
            arrival_zone.to_string(),
        ));
        if let Some(required) = r["arrivalRequiredAt"].as_str().filter(|v| !v.is_empty()) {
            let deadline = DateTime::parse_from_rfc3339(required)
                .map_err(|_| "Arrival requirement needs an ISO instant")?;
            if b > deadline {
                warnings.push(json!({"type":"late_arrival","message":"Arrival is after the explicitly recorded arrival requirement."}));
            }
            if a > deadline {
                warnings.push(json!({"type":"departure_after_deadline","message":"Departure is already after the required arrival instant."}));
            }
        }
        for stay in records.iter().filter(|s| {
            text(s, "kind") == "stay"
                && active(s)
                && !text(r, "originStayId").is_empty()
                && text(s, "id") == text(r, "originStayId")
        }) {
            let checkout = schedule::resolve_local(
                text(stay, "checkOut"),
                stay["checkOutTime"].as_str().unwrap_or("11:00"),
                text(stay, "timezone"),
            )?;
            let minutes = (a.with_timezone(&chrono::Utc) - checkout).num_minutes();
            warnings.push(json!({"type":"checkout_departure","recordId":stay["id"],"minutes":minutes,"message":if minutes>=0{format!("{minutes} minutes from recorded checkout to departure. Transfer duration, luggage storage and access are unverified unless separately recorded.")}else{format!("Departure is {} minutes before recorded checkout; review your departure and access plan.",-minutes)}}));
        }
        for other in records.iter().filter(|s| {
            text(s, "kind") == "transport"
                && text(s, "status") == "confirmed"
                && text(s, "id") != text(r, "id")
        }) {
            if let (Ok(start), Ok(end)) = (
                DateTime::parse_from_rfc3339(text(other, "departure")),
                DateTime::parse_from_rfc3339(text(other, "arrival")),
            ) {
                if start < b && end > a {
                    warnings.push(json!({"type":"overlapping_transport","recordId":other["id"],"message":"The proposed journey overlaps another confirmed transport segment."}));
                }
            }
        }
        let prior = records
            .iter()
            .filter(|s| {
                text(s, "kind") == "transport"
                    && text(s, "id") != text(r, "id")
                    && text(s, "status") == "confirmed"
                    && (text(s, "destination") == text(r, "origin")
                        || (!text(r, "tripId").is_empty()
                            && text(s, "tripId") == text(r, "tripId")))
            })
            .filter_map(|s| {
                DateTime::parse_from_rfc3339(text(s, "arrival"))
                    .ok()
                    .map(|at| (s, at))
            })
            .filter(|(_, at)| *at <= a)
            .max_by_key(|(_, at)| *at);
        if let Some((s, at)) = prior {
            let gap = (a - at).num_minutes();
            value["transferMinutes"] = json!(gap);
            if let Some(required) = r["minimumTransferMinutes"].as_i64() {
                if gap < required {
                    warnings.push(json!({"type":"short_transfer","recordId":s["id"],"message":"Transfer is shorter than your recorded minimum."}));
                }
            } else {
                warnings.push(json!({"type":"unknown_transfer","message":"Minimum transfer duration is unknown; connection feasibility is unverified."}));
            }
        }
        (
            a.with_timezone(&departure_zone).date_naive().to_string(),
            departure_zone.to_string(),
        )
    } else {
        return Err("Preview a stay, quote or transport record".into());
    };
    let mut proposed: Vec<Value> = records
        .iter()
        .filter(|v| text(v, "id") != text(r, "id") || text(r, "id").is_empty())
        .cloned()
        .collect();
    let mut draft = r.clone();
    draft["id"] = json!(r["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("unsaved-preview"));
    // Evaluate proposed activity without changing its saved confirmation status.
    draft["status"] = json!("confirmed");
    proposed.push(draft);
    let work = schedule::window(
        &proposed,
        settings,
        &json!({"date":date,"timezone":zone,"dateBasis":"destination"}),
    )?;
    value["work"] = work.clone();
    let mut checks = vec![json!({"date":date,"timezone":zone,"result":work})];
    if let Some((end_date, end_zone)) = end_context {
        if end_date != date || end_zone != zone {
            let end_work = schedule::window(
                &proposed,
                settings,
                &json!({"date":end_date,"timezone":end_zone,"dateBasis":"destination"}),
            )?;
            checks.push(json!({"date":end_date,"timezone":end_zone,"result":end_work}));
        }
    }
    value["workChecks"] = json!(checks);
    value["warnings"] = json!(warnings);
    Ok(value)
}
