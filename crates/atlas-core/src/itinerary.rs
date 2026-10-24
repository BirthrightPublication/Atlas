//! Reviewed tentative journeys, deterministic schedule proposals and selected offline packs.
use crate::{error, id, journey, money, schedule, string as text, Atlas};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::{json, Value};
fn date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| "Use a valid YYYY-MM-DD date".into())
}
pub(crate) fn validate_stops(value: &Value) -> Result<(), String> {
    let stops = value
        .as_array()
        .filter(|s| !s.is_empty() && s.len() <= 40)
        .ok_or("Use 1–40 stops")?;
    let mut previous = None;
    let mut days = 0;
    let first = date(text(&stops[0], "arrival"))?;
    for s in stops {
        if text(s, "name").trim().is_empty() || text(s, "name").len() > 200 {
            return Err("Each stop needs a destination name".into());
        }
        let a = date(text(s, "arrival"))?;
        let b = date(text(s, "departure"))?;
        if (b - first).num_days() > 366 {
            return Err("A journey date range is limited to 366 days".into());
        }
        if b <= a {
            return Err("Each departure must follow arrival; checkout is exclusive".into());
        }
        if previous.is_some_and(|v| a < v) {
            return Err("Stops must be chronological without overlapping dates. Edit the tentative dates before reordering.".into());
        }
        days += (b - a).num_days();
        if days > 366 {
            return Err("A planning draft is limited to 366 occupied days".into());
        }
        previous = Some(b);
        text(s, "timezone")
            .parse::<chrono_tz::Tz>()
            .map_err(|_| "Each stop needs an IANA timezone")?;
        if !["on_foot", "on_road"].contains(&text(s, "context")) {
            return Err("Choose On Foot or On Road for each stop".into());
        }
        for field in ["totalMinor", "dailyBudgetMinor"] {
            if let Some(v) = s.get(field).filter(|v| !v.is_null()) {
                let n = v.as_i64().ok_or("Costs must be integer minor units")?;
                if !(0..=1_000_000_000_000).contains(&n) {
                    return Err("Invalid cost range".into());
                }
            }
        }
        let currency = text(s, "currency");
        if currency.len() != 3 || !currency.bytes().all(|c| c.is_ascii_uppercase()) {
            return Err("Each stop needs a three-letter currency".into());
        }
    }
    Ok(())
}
fn preview(records: &[Value], settings: &Value, p: &Value) -> Result<Value, String> {
    validate_stops(&p["stops"])?;
    let stops = p["stops"].as_array().ok_or("Stops required")?;
    let mut issues = vec![];
    let mut months = std::collections::BTreeSet::new();
    let mut hypothetical = vec![];
    let mut currencies = std::collections::BTreeSet::new();
    for (index, s) in stops.iter().enumerate() {
        let a = date(text(s, "arrival"))?;
        let b = date(text(s, "departure"))?;
        let stay = records
            .iter()
            .find(|r| r["id"] == s["stayId"] && text(r, "kind") == "stay");
        if let Some(r) = stay {
            if text(r, "status") == "confirmed"
                && (text(r, "checkIn") != text(s, "arrival")
                    || text(r, "checkOut") != text(s, "departure"))
            {
                issues.push(json!({"type":"confirmed_dates","stop":index,"recordId":r["id"],"message":"Draft dates differ from the confirmed stay. The booking is not moved."}));
            }
        } else {
            issues.push(json!({"type":"missing_accommodation","stop":index,"message":"No saved accommodation linked"}));
        }
        if index > 0
            && !records
                .iter()
                .any(|r| r["id"] == s["transportId"] && text(r, "kind") == "transport")
        {
            issues.push(json!({"type":"missing_transport","stop":index,"message":"Connection duration, cost and availability remain unknown until recorded"}));
        }
        let total = s["totalMinor"].as_i64().or_else(|| {
            stay.filter(|r| r["currency"] == s["currency"])
                .and_then(|r| r["totalMinor"].as_i64())
                .filter(|_| {
                    stay.is_some_and(|r| {
                        text(r, "checkIn") == text(s, "arrival")
                            && text(r, "checkOut") == text(s, "departure")
                    })
                })
        });
        if let Some(total) = total {
            hypothetical.push(json!({"id":format!("scenario-{index}"),"kind":"stay","name":s["name"],"checkIn":s["arrival"],"checkOut":s["departure"],"currency":s["currency"],"totalMinor":total,"status":"confirmed"}));
        } else {
            issues.push(json!({"type":"unknown_cost","stop":index,"message":"Accommodation cost is unknown; totals include known amounts only"}));
        }
        if let Some(daily) = s["dailyBudgetMinor"].as_i64() {
            for offset in 0..(b - a).num_days() {
                hypothetical.push(json!({"id":format!("scenario-daily-{index}-{offset}"),"kind":"expense","name":"Hypothetical daily spending","date":(a+Duration::days(offset)).to_string(),"currency":s["currency"],"amountMinor":daily,"status":"planned"}));
            }
        }
        currencies.insert(text(s, "currency").to_owned());
        for offset in 0..(b - a).num_days() {
            months.insert((a + Duration::days(offset)).format("%Y-%m").to_string());
        }
        let work = schedule::window(
            records,
            settings,
            &json!({"date":s["arrival"],"timezone":s["timezone"],"dateBasis":"destination"}),
        )?;
        for conflict in work["conflicts"].as_array().into_iter().flatten() {
            issues.push(conflict.clone());
        }
        if stay.is_some_and(|r| r["facilities"]["internet"] != true) {
            issues.push(json!({"type":"unknown_connectivity","stop":index,"message":"Workspace connectivity is unverified"}));
        }
    }
    let mut costs = vec![];
    for currency in currencies {
        for month in &months {
            costs.push(money::summary(
                &hypothetical,
                settings,
                &json!({"month":month,"currency":currency}),
            )?);
        }
    }
    let first = &stops[0];
    let last = stops.last().ok_or("Stops required")?;
    Ok(
        json!({"stops":stops,"issues":issues,"monthlyScenarioCosts":costs,"savedCoverage":journey::gaps(records,&json!({"start":first["arrival"],"end":last["departure"]}))?,"hypothetical":true,"recordsChanged":false,"routeVerification":"External provider links only; no calculated route duration or distance."}),
    )
}
fn proposals(records: &[Value], settings: &Value, p: &Value) -> Result<Vec<Value>, String> {
    let start = date(text(p, "start"))?;
    let days = p["days"].as_i64().unwrap_or(14);
    if !(1..=31).contains(&days) {
        return Err("Choose 1–31 days".into());
    }
    let timezone = text(p, "timezone");
    let zone: chrono_tz::Tz = timezone
        .parse()
        .map_err(|_| "Choose a destination timezone")?;
    let office: chrono_tz::Tz = settings["officeTimezone"]
        .as_str()
        .unwrap_or("America/Los_Angeles")
        .parse()
        .map_err(|_| "Invalid employer timezone")?;
    let mut items = vec![];
    for offset in 0..days {
        let day = (start + Duration::days(offset)).to_string();
        let w = schedule::window(
            records,
            settings,
            &json!({"date":day,"dateBasis":"destination","timezone":timezone}),
        )?;
        let mut busy: Vec<(DateTime<Utc>, DateTime<Utc>)> = vec![];
        for s in w["suggestedBlocks"].as_array().into_iter().flatten().chain(
            w["blocks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|b| &b["window"]),
        ) {
            if let (Ok(a), Ok(b)) = (
                DateTime::parse_from_rfc3339(text(s, "start")),
                DateTime::parse_from_rfc3339(text(s, "end")),
            ) {
                busy.push((a.with_timezone(&Utc), b.with_timezone(&Utc)));
            }
        }
        let expanded = crate::recurrence::expand(
            records,
            &day,
            &(start + Duration::days(offset + 1)).to_string(),
        )?;
        for r in &expanded {
            if text(r, "status") == "cancelled" {
                continue;
            }
            if text(r, "kind") == "transport" {
                if let (Ok(a), Ok(b)) = (
                    DateTime::parse_from_rfc3339(text(r, "departure")),
                    DateTime::parse_from_rfc3339(text(r, "arrival")),
                ) {
                    busy.push((a.with_timezone(&Utc), b.with_timezone(&Utc)));
                }
            } else if ["objective", "workBlock"].contains(&text(r, "kind"))
                && text(r, "date") == day
                && !text(r, "startTime").is_empty()
            {
                if let (Ok(a), Ok(b)) = (
                    schedule::resolve_local(&day, text(r, "startTime"), text(r, "timezone")),
                    schedule::resolve_local(&day, text(r, "endTime"), text(r, "timezone")),
                ) {
                    busy.push((a, b));
                }
            } else if text(r, "kind") == "stay" && text(r, "checkOut") == day {
                if let Ok(a) = schedule::resolve_local(
                    &day,
                    r["checkOutTime"].as_str().unwrap_or("11:00"),
                    text(r, "timezone"),
                ) {
                    busy.push((a - Duration::hours(1), a + Duration::hours(1)));
                }
            }
        }
        let begin = schedule::resolve_local(&day, "08:00", timezone)?;
        let end = schedule::resolve_local(&day, "21:00", timezone)?;
        let mut cursor = begin;
        while cursor + Duration::minutes(90) <= end {
            let finish = cursor + Duration::minutes(90);
            if !busy.iter().any(|(a, b)| cursor < *b && finish > *a) {
                items.push(json!({"id":id(),"state":"pending","record":{"kind":"objective","name":p["purpose"].as_str().unwrap_or("Explore destination — choose a saved attraction"),"category":"sightseeing","date":day,"startTime":cursor.with_timezone(&zone).format("%H:%M").to_string(),"endTime":finish.with_timezone(&zone).format("%H:%M").to_string(),"timezone":timezone,"status":"planned","context":"on_foot","notes":"Reviewed scheduling proposal; attraction availability, accessibility and travel time are unverified."},"localStart":cursor.with_timezone(&zone).to_rfc3339(),"localEnd":finish.with_timezone(&zone).to_rfc3339(),"officeStart":cursor.with_timezone(&office).to_rfc3339(),"officeEnd":finish.with_timezone(&office).to_rfc3339(),"work":w,"routeVerified":false}));
                break;
            }
            cursor += Duration::minutes(30);
        }
    }
    Ok(items)
}
impl Atlas {
    pub(crate) fn itinerary(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "journeys.preview" => preview(&self.records(None)?, &self.settings()?, p),
            "journeys.save" => {
                let mut r = p["record"].clone();
                if r["kind"] != "trip" {
                    return Err("Expected a tentative journey".into());
                }
                validate_stops(&r["stops"])?;
                if !text(&r, "id").is_empty() && p["expectedVersion"].as_i64().is_none() {
                    return Err("Reload the saved journey and supply its expected version".into());
                }
                r["status"] = json!("draft");
                self.save_record(r, p["expectedVersion"].as_i64())
            }
            "journeys.propose" => {
                let items = proposals(&self.records(None)?, &self.settings()?, p)?;
                self.save_record(json!({"kind":"proposal","name":"Day-by-day itinerary proposal","status":"pending","items":items,"request":p,"basis":"deterministic work and saved schedule; not a reservation"}),None)
            }
            "journeys.accept" | "journeys.reject" => {
                let mut proposal = self.get_required(text(p, "id"))?;
                if proposal["kind"] != "proposal" {
                    return Err("Select a journey proposal".into());
                }
                if p["expectedVersion"].as_i64() != proposal["version"].as_i64() {
                    return Err("Proposal changed; reload before deciding".into());
                }
                let position = proposal["items"]
                    .as_array()
                    .ok_or("Invalid proposal")?
                    .iter()
                    .position(|v| v["id"] == p["itemId"])
                    .ok_or("Proposal item missing")?;
                let item = proposal["items"][position].clone();
                if item["state"] != "pending" {
                    return Ok(proposal);
                }
                if method == "journeys.reject" {
                    proposal["items"][position]["state"] = json!("rejected");
                    return self.save_record(proposal, p["expectedVersion"].as_i64());
                }
                let mut record = item["record"].clone();
                if let Some(name) = p["name"].as_str() {
                    record["name"] = json!(name);
                }
                // Recompute against current protected work and activities before each acceptance.
                let mut request = proposal["request"].clone();
                request["start"] = record["date"].clone();
                request["days"] = json!(1);
                let current = proposals(&self.records(None)?, &self.settings()?, &request)?;
                if !current.iter().any(|v| {
                    v["record"]["startTime"] == record["startTime"]
                        && v["record"]["endTime"] == record["endTime"]
                }) {
                    return Err(
                        "Schedule changed; regenerate the proposal before accepting this time"
                            .into(),
                    );
                }
                self.db
                    .execute_batch("SAVEPOINT itinerary_accept")
                    .map_err(error)?;
                let result = (|| {
                    let saved = self.save_record(record, None)?;
                    proposal["items"][position]["state"] = json!("accepted");
                    proposal["items"][position]["recordId"] = saved["id"].clone();
                    self.save_record(proposal, p["expectedVersion"].as_i64())
                })();
                match result {
                    Ok(v) => {
                        self.db
                            .execute_batch("RELEASE itinerary_accept")
                            .map_err(error)?;
                        Ok(v)
                    }
                    Err(e) => {
                        let _ = self.db.execute_batch(
                            "ROLLBACK TO itinerary_accept; RELEASE itinerary_accept",
                        );
                        Err(e)
                    }
                }
            }
            "routes.draft" => {
                let records = self.records(None)?;
                let from = records
                    .iter()
                    .find(|r| r["id"] == p["fromId"])
                    .ok_or("Select a saved departure place")?;
                let to = records
                    .iter()
                    .find(|r| r["id"] == p["toId"])
                    .ok_or("Select a saved destination place")?;
                let location = |r: &Value| -> Result<String, String> {
                    if !text(r, "address").is_empty() {
                        Ok(text(r, "address").into())
                    } else if let (Some(a), Some(b)) =
                        (r["latitude"].as_f64(), r["longitude"].as_f64())
                    {
                        if a.abs() > 90. || b.abs() > 180. {
                            return Err("Invalid saved coordinates".into());
                        }
                        Ok(format!("{a},{b}"))
                    } else {
                        Err("Both places need an address or coordinates".into())
                    }
                };
                let mode = text(p, "mode");
                if !["driving", "walking", "transit", "flight"].contains(&mode) {
                    return Err("Choose a supported travel mode".into());
                }
                date(text(p, "date"))?;
                let mut url = url::Url::parse("https://www.google.com/maps/dir/").map_err(error)?;
                if mode == "flight" {
                    url = url::Url::parse("https://www.rome2rio.com/map/").map_err(error)?;
                    url.path_segments_mut()
                        .map_err(|_| "Invalid directions URL")?
                        .push(&location(from)?)
                        .push(&location(to)?);
                } else {
                    url.query_pairs_mut()
                        .append_pair("api", "1")
                        .append_pair("origin", &location(from)?)
                        .append_pair("destination", &location(to)?)
                        .append_pair("travelmode", mode);
                }
                self.save_record(json!({"kind":"routeDraft","name":format!("{} → {}",text(from,"name"),text(to,"name")),"fromId":from["id"],"toId":to["id"],"date":p["date"],"mode":mode,"context":if mode=="driving"{"on_road"}else{"on_foot"},"directionsUrl":url.to_string(),"notes":p["notes"],"durationMinutes":null,"distanceKm":null,"verified":false,"status":"draft"}),None)
            }
            "journeys.offline" => {
                let trip = self.get_required(text(p, "id"))?;
                if trip["kind"] != "trip" {
                    return Err("Select a saved journey".into());
                }
                validate_stops(&trip["stops"])?;
                let stops = trip["stops"].as_array().ok_or("Invalid stops")?;
                let start = text(&stops[0], "arrival");
                let end = text(stops.last().ok_or("Invalid stops")?, "departure");
                let records = self.records(None)?;
                let selected: Vec<_> = records
                    .iter()
                    .filter(|r| {
                        if ["source", "draft", "connection", "document", "proposal"]
                            .contains(&text(r, "kind"))
                        {
                            return false;
                        }
                        stops.iter().any(|s| {
                            ["stayId", "transportId", "placeId"]
                                .iter()
                                .any(|key| !text(s, key).is_empty() && s[*key] == r["id"])
                        }) || ["date", "checkIn", "departure", "dueAt"].iter().any(|k| {
                            let d = text(r, k);
                            d.get(..10).is_some_and(|d| d >= start && d < end)
                        })
                    })
                    .cloned()
                    .collect();
                let ids = p["documentIds"].as_array().cloned().unwrap_or_default();
                if ids.len() > 10 {
                    return Err("Select at most ten documents per offline package".into());
                }
                let mut documents = vec![];
                let mut bytes = 0;
                for id in ids {
                    let d = self.read_document(id.as_str().ok_or("Invalid document ID")?)?;
                    bytes += text(&d, "contentBase64").len();
                    if bytes > 28 * 1024 * 1024 {
                        return Err(
                            "Selected documents exceed the offline package size limit".into()
                        );
                    }
                    documents.push(d);
                }
                let mut budgets = vec![];
                let mut months = std::collections::BTreeSet::new();
                let mut d = date(start)?;
                while d < date(end)? {
                    months.insert(d.format("%Y-%m").to_string());
                    d += Duration::days(1);
                }
                for month in months {
                    budgets.push(money::summary(
                        &records,
                        &self.settings()?,
                        &json!({"month":month,"currency":p["currency"].as_str().unwrap_or("USD")}),
                    )?);
                }
                Ok(
                    json!({"format":"atlas-offline-trip-v1","generatedAt":crate::now(),"trip":trip,"records":selected,"documents":documents,"budget":budgets,"workRequirements":{"officeTimezone":self.settings()?["officeTimezone"],"workHours":self.settings()?["workHours"],"overlapHours":self.settings()?["overlapHours"]},"privacy":"Only explicitly chosen document bytes are included. Email bodies and credentials are excluded.","routes":"Saved evidence only; no offline routing engine."}),
                )
            }
            _ => Err("Unknown itinerary operation".into()),
        }
    }
}
