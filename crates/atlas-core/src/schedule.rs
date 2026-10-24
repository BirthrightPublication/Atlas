//! Employer-day work rules and timezone-aware planning. Every scheduled boundary
//! resolves through IANA rules; ambiguous/nonexistent local times require a choice.
use chrono::{
    DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveTime, Offset, TimeZone,
    Utc,
};
use chrono_tz::Tz;
use serde_json::{json, Value};

type Interval = (DateTime<Utc>, DateTime<Utc>);

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

fn date(value: &str) -> Result<NaiveDate, String> {
    if value.len() != 10 {
        return Err("Date must use YYYY-MM-DD".into());
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| format!("Invalid date: {value}"))
}

fn time(value: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(value, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M:%S"))
        .map_err(|_| format!("Invalid local time: {value}"))
}

/// Never guesses during a DST fold or gap. The caller can instead supply a
/// validated explicit-offset instant when the user resolves the ambiguity.
pub fn resolve_local(day: &str, local_time: &str, timezone: &str) -> Result<DateTime<Utc>, String> {
    let tz = timezone
        .parse::<Tz>()
        .map_err(|_| format!("Unknown IANA timezone: {timezone}"))?;
    match tz.from_local_datetime(&date(day)?.and_time(time(local_time)?)) {
        LocalResult::Single(v) => Ok(v.with_timezone(&Utc)),
        LocalResult::None => Err(format!(
            "{day} {local_time} does not exist in {timezone} due to a clock change"
        )),
        LocalResult::Ambiguous(a, b) => Err(format!(
            "{day} {local_time} occurs twice in {timezone}; choose {} or {}",
            a.to_rfc3339(),
            b.to_rfc3339()
        )),
    }
}

#[derive(Clone, Copy)]
enum Zone {
    Iana(Tz),
    Fixed(FixedOffset),
}

impl Zone {
    fn local(self, instant: DateTime<Utc>) -> DateTime<FixedOffset> {
        match self {
            Self::Iana(z) => instant.with_timezone(&z).fixed_offset(),
            Self::Fixed(z) => instant.with_timezone(&z),
        }
    }
    fn at(self, day: NaiveDate, local_time: NaiveTime) -> Result<DateTime<Utc>, String> {
        match self {
            Self::Iana(z) => resolve_local(
                &day.to_string(),
                &local_time.format("%H:%M:%S").to_string(),
                z.name(),
            ),
            Self::Fixed(z) => z
                .from_local_datetime(&day.and_time(local_time))
                .single()
                .map(|v| v.with_timezone(&Utc))
                .ok_or("Invalid fixed-offset date".into()),
        }
    }
    fn label(self) -> String {
        match self {
            Self::Iana(z) => z.name().into(),
            Self::Fixed(z) => format!("UTC{z}"),
        }
    }
}

fn office_zone(settings: &Value) -> Result<Zone, String> {
    if settings["officeTimezoneMode"].as_str() == Some("fixed") {
        let minutes = settings["officeFixedOffsetMinutes"]
            .as_i64()
            .ok_or("Fixed office timezone requires an explicit offset in minutes")?;
        if !(-1439..=1439).contains(&minutes) {
            return Err("Invalid fixed UTC offset".into());
        }
        return FixedOffset::east_opt(minutes as i32 * 60)
            .map(Zone::Fixed)
            .ok_or("Invalid fixed UTC offset".into());
    }
    settings["officeTimezone"]
        .as_str()
        .unwrap_or("America/Los_Angeles")
        .parse::<Tz>()
        .map(Zone::Iana)
        .map_err(|_| "Invalid office IANA timezone".into())
}

pub(crate) fn employer_label(instant: DateTime<Utc>, settings: &Value) -> Result<String, String> {
    Ok(office_zone(settings)?.local(instant).to_rfc3339())
}

fn exception<'a>(settings: &'a Value, day: NaiveDate) -> Option<&'a Value> {
    settings["workExceptions"]
        .as_array()?
        .iter()
        .find(|v| text(v, "date") == day.to_string())
}

fn weekday_matches(v: &Value, day: NaiveDate) -> bool {
    if v.as_u64() == Some(day.weekday().number_from_monday() as u64) {
        return true;
    }
    let name = day.format("%a").to_string().to_ascii_lowercase();
    v.as_str()
        .map(|s| s.to_ascii_lowercase().starts_with(&name))
        .unwrap_or(false)
}

fn working_day(settings: &Value, day: NaiveDate) -> bool {
    if let Some(e) = exception(settings, day) {
        if let Some(working) = e["working"].as_bool().or_else(|| e["isWorkday"].as_bool()) {
            return working;
        }
    }
    if settings["holidays"]
        .as_array()
        .map(|days| {
            days.iter()
                .any(|v| v.as_str() == Some(day.to_string().as_str()))
        })
        .unwrap_or(false)
    {
        return false;
    }
    match settings["workdays"].as_array() {
        Some(days) => days.iter().any(|v| weekday_matches(v, day)),
        None => day.weekday().number_from_monday() <= 5,
    }
}

fn hours(settings: &Value, e: Option<&Value>, key: &str, default: f64) -> Result<i64, String> {
    let value = e
        .and_then(|v| v[key].as_f64())
        .or_else(|| settings[key].as_f64())
        .unwrap_or(default);
    if !value.is_finite() || !(0.0..=24.0).contains(&value) {
        return Err(format!("{key} must be between 0 and 24"));
    }
    Ok((value * 60.0).round() as i64)
}

fn intersection(a: Interval, b: Interval) -> Option<Interval> {
    let result = (a.0.max(b.0), a.1.min(b.1));
    if result.0 < result.1 {
        Some(result)
    } else {
        None
    }
}

fn merge(mut spans: Vec<Interval>) -> Vec<Interval> {
    spans.sort_by_key(|s| s.0);
    let mut result: Vec<Interval> = vec![];
    for span in spans {
        if let Some(last) = result.last_mut() {
            if span.0 <= last.1 {
                last.1 = last.1.max(span.1);
                continue;
            }
        }
        result.push(span);
    }
    result
}

fn subtract(spans: Vec<Interval>, blocked: &[Interval]) -> Vec<Interval> {
    let mut result = spans;
    for block in blocked {
        result = result
            .into_iter()
            .flat_map(|span| {
                let mut pieces = vec![];
                if let Some(overlap) = intersection(span, *block) {
                    if span.0 < overlap.0 {
                        pieces.push((span.0, overlap.0));
                    }
                    if overlap.1 < span.1 {
                        pieces.push((overlap.1, span.1));
                    }
                } else {
                    pieces.push(span);
                }
                pieces
            })
            .collect();
    }
    result
}

fn duration(spans: &[Interval]) -> i64 {
    merge(spans.to_vec())
        .iter()
        .map(|v| (v.1 - v.0).num_minutes())
        .sum()
}

fn office_intervals(
    settings: &Value,
    zone: Zone,
    day: NaiveDate,
    apply_work_breaks: bool,
) -> Result<Vec<Interval>, String> {
    if !working_day(settings, day) {
        return Ok(vec![]);
    }
    let e = exception(settings, day);
    let start_time = time(
        e.and_then(|v| v["workStart"].as_str())
            .or_else(|| settings["workStart"].as_str())
            .unwrap_or("08:00"),
    )?;
    let end_time = time(
        e.and_then(|v| v["workEnd"].as_str())
            .or_else(|| settings["workEnd"].as_str())
            .unwrap_or("17:00"),
    )?;
    let end_day = if end_time <= start_time {
        day.succ_opt().ok_or("Date overflow")?
    } else {
        day
    };
    let span = (zone.at(day, start_time)?, zone.at(end_day, end_time)?);
    if !apply_work_breaks {
        return Ok(vec![span]);
    }
    let mut breaks = vec![];
    if let Some(items) = e
        .and_then(|v| v["breaks"].as_array())
        .or_else(|| settings["workBreaks"].as_array())
    {
        for item in items {
            let a = time(text(item, "start"))?;
            let b = time(text(item, "end"))?;
            let a_day = if a < start_time && end_day != day {
                end_day
            } else {
                day
            };
            let b_day = if b <= a {
                a_day.succ_opt().ok_or("Date overflow")?
            } else {
                a_day
            };
            if let Some(br) = intersection(span, (zone.at(a_day, a)?, zone.at(b_day, b)?)) {
                breaks.push(br);
            }
        }
    } else if let Some(minutes) = e
        .and_then(|v| v["breakMinutes"].as_i64())
        .or_else(|| settings["breakMinutes"].as_i64())
    {
        if !(0..=240).contains(&minutes) {
            return Err("Daily break must be between 0 and 240 minutes".into());
        }
        if minutes > 0 {
            let length = (span.1 - span.0).num_minutes();
            let before = (length - minutes).max(0) / 2;
            let a = span.0 + Duration::minutes(before);
            breaks.push((a, (a + Duration::minutes(minutes)).min(span.1)));
        }
    }
    Ok(subtract(vec![span], &breaks))
}

fn local_interval(record: &Value, fallback_timezone: &str) -> Result<Option<Interval>, String> {
    let parse_instant = |value: &str| {
        DateTime::parse_from_rfc3339(value)
            .map(|v| v.with_timezone(&Utc))
            .map_err(|_| "Scheduled instants require a timezone offset".to_owned())
    };
    let a = record["startAt"]
        .as_str()
        .or_else(|| record["start"].as_str());
    let b = record["endAt"].as_str().or_else(|| record["end"].as_str());
    if let (Some(a), Some(b)) = (a, b) {
        if a.contains('T') && b.contains('T') {
            let pair = (parse_instant(a)?, parse_instant(b)?);
            return if pair.1 > pair.0 {
                Ok(Some(pair))
            } else {
                Err("Scheduled end must follow start".into())
            };
        }
    }
    let day = text(record, "date");
    let start = record["startTime"].as_str().or(a).unwrap_or("");
    let end = record["endTime"].as_str().or(b).unwrap_or("");
    if day.is_empty() || start.is_empty() || end.is_empty() {
        return Ok(None);
    }
    let d = date(day)?;
    let end_day = if time(end)? <= time(start)? {
        d.succ_opt().ok_or("Date overflow")?
    } else {
        d
    };
    let tz = record["timezone"].as_str().unwrap_or(fallback_timezone);
    Ok(Some((
        resolve_local(day, start, tz)?,
        resolve_local(&end_day.to_string(), end, tz)?,
    )))
}

fn personal_windows(settings: &Value, tz: Tz, day_span: Interval) -> Result<Vec<Interval>, String> {
    let restrictions = &settings["localDayRestrictions"];
    let start = restrictions["start"]
        .as_str()
        .or_else(|| settings["localDayStart"].as_str());
    let end = restrictions["end"]
        .as_str()
        .or_else(|| settings["localDayEnd"].as_str());
    if start.is_none() && end.is_none() {
        return Ok(vec![day_span]);
    }
    let a = time(start.unwrap_or("00:00"))?;
    let b = time(end.unwrap_or("00:00"))?;
    let mut d = day_span
        .0
        .with_timezone(&tz)
        .date_naive()
        .pred_opt()
        .ok_or("Date overflow")?;
    let last = day_span.1.with_timezone(&tz).date_naive();
    let mut spans = vec![];
    while d <= last {
        if restrictions["weekdays"]
            .as_array()
            .map(|days| days.iter().any(|v| weekday_matches(v, d)))
            .unwrap_or(true)
        {
            let until = if b <= a {
                d.succ_opt().ok_or("Date overflow")?
            } else {
                d
            };
            if let Some(s) = intersection(
                day_span,
                (Zone::Iana(tz).at(d, a)?, Zone::Iana(tz).at(until, b)?),
            ) {
                spans.push(s);
            }
        }
        d = d.succ_opt().ok_or("Date overflow")?;
    }
    Ok(merge(spans))
}

fn display_span(span: Interval, office: Zone, destination: Tz) -> Value {
    json!({"start":span.0.to_rfc3339(),"end":span.1.to_rfc3339(),"minutes":(span.1-span.0).num_minutes(),
        "officeStart":office.local(span.0).to_rfc3339(),"officeEnd":office.local(span.1).to_rfc3339(),
        "localStart":span.0.with_timezone(&destination).to_rfc3339(),"localEnd":span.1.with_timezone(&destination).to_rfc3339()})
}

/// `date` denotes an employer calendar date by default. An explicit
/// `dateBasis: "destination"` maps destination noon to its employer date and
/// returns that mapping so the UI never hides a cross-date workday.
pub fn window(records: &[Value], settings: &Value, payload: &Value) -> Result<Value, String> {
    let selected = date(
        payload["date"]
            .as_str()
            .ok_or("date is required (YYYY-MM-DD)")?,
    )?;
    let office = office_zone(settings)?;
    let planned_stay = records.iter().find(|r| {
        text(r, "kind") == "stay"
            && text(r, "status") == "confirmed"
            && date(text(r, "checkIn"))
                .map(|d| d <= selected)
                .unwrap_or(false)
            && date(text(r, "checkOut"))
                .map(|d| d > selected)
                .unwrap_or(false)
    });
    let destination_name = payload["timezone"]
        .as_str()
        .or_else(|| planned_stay.and_then(|s| s["timezone"].as_str()))
        .or_else(|| settings["currentTimezone"].as_str())
        .unwrap_or("UTC");
    let destination = destination_name
        .parse::<Tz>()
        .map_err(|_| "Invalid destination IANA timezone".to_owned())?;
    let current_name = settings["currentTimezone"].as_str().unwrap_or("UTC");
    let current = current_name
        .parse::<Tz>()
        .map_err(|_| "Invalid current IANA timezone".to_owned())?;
    let basis = payload["dateBasis"].as_str().unwrap_or("office");
    if !matches!(basis, "office" | "destination") {
        return Err("dateBasis must be office or destination".into());
    }
    let office_date = if basis == "destination" {
        office
            .local(resolve_local(
                &selected.to_string(),
                "12:00",
                destination_name,
            )?)
            .date_naive()
    } else {
        selected
    };
    let midnight = NaiveTime::from_hms_opt(0, 0, 0).ok_or("Invalid midnight")?;
    let day_span = (
        office.at(office_date, midnight)?,
        office.at(office_date.succ_opt().ok_or("Date overflow")?, midnight)?,
    );
    // Include the preceding destination day for blocks crossing local midnight.
    let recurrence_start = day_span
        .0
        .with_timezone(&destination)
        .date_naive()
        .pred_opt()
        .ok_or("Date overflow")?;
    let recurrence_end = day_span
        .1
        .with_timezone(&destination)
        .date_naive()
        .succ_opt()
        .ok_or("Date overflow")?;
    let expanded = crate::recurrence::expand(
        records,
        &recurrence_start.to_string(),
        &recurrence_end.to_string(),
    )?;
    let records = expanded.as_slice();
    let is_workday = working_day(settings, office_date);
    let e = exception(settings, office_date);
    let required = if is_workday {
        hours(settings, e, "workHours", 8.0)?
    } else {
        0
    };
    let overlap_required = if is_workday {
        hours(settings, e, "overlapHours", 4.0)?
    } else {
        0
    };
    if overlap_required > required {
        return Err("Required office overlap cannot exceed total work hours".into());
    }
    let office_spans = office_intervals(settings, office, office_date, true)?;
    let office_hours = office_intervals(settings, office, office_date, false)?;
    let work_breaks = subtract(office_hours, &office_spans);
    let now = match payload["now"].as_str() {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map_err(|_| "now must be an ISO instant")?
            .with_timezone(&Utc),
        None => Utc::now(),
    };
    let office_today = office.local(now).date_naive();
    let mut live_spans = office_intervals(settings, office, office_today, false)?;
    if let Some(yesterday) = office_today.pred_opt() {
        live_spans.extend(office_intervals(settings, office, yesterday, false)?);
    }
    let office_open = live_spans.iter().any(|s| now >= s.0 && now < s.1);
    let mut blocks: Vec<(&Value, Interval)> = vec![];
    let mut blockers: Vec<(&Value, String, Interval)> = vec![];
    let mut warnings: Vec<Value> = vec![];
    let buffer = Duration::minutes(
        payload["transitionBufferMinutes"]
            .as_i64()
            .unwrap_or(30)
            .clamp(0, 180),
    );
    for record in records {
        if matches!(
            text(record, "status"),
            "cancelled" | "tentative" | "dismissed"
        ) {
            continue;
        }
        match text(record, "kind") {
            "workBlock" => {
                if let Some(span) = local_interval(record, destination_name)? {
                    if let Some(part) = intersection(span, day_span) {
                        blocks.push((record, part));
                    }
                }
            }
            "transport" => {
                if text(record, "departure").is_empty() || text(record, "arrival").is_empty() {
                    continue;
                }
                let a = DateTime::parse_from_rfc3339(text(record, "departure"))
                    .map_err(|_| "Invalid transport departure instant")?
                    .with_timezone(&Utc);
                let b = DateTime::parse_from_rfc3339(text(record, "arrival"))
                    .map_err(|_| "Invalid transport arrival instant")?
                    .with_timezone(&Utc);
                if b <= a {
                    return Err(
                        "Transport arrival must follow departure as an actual instant".into(),
                    );
                }
                if let Some(span) = intersection((a, b), day_span) {
                    blockers.push((record, "transit".into(), span));
                }
            }
            "objective" => {
                if let Some(span) = local_interval(record, destination_name)? {
                    if let Some(part) = intersection(span, day_span) {
                        blockers.push((record, "objective".into(), part));
                    }
                }
            }
            "stay" => {
                let tz = record["timezone"].as_str().unwrap_or(destination_name);
                for (date_key, time_key, fallback, label) in [
                    ("checkIn", "checkInTime", "15:00", "check-in"),
                    ("checkOut", "checkOutTime", "11:00", "checkout"),
                ] {
                    if text(record, date_key).is_empty() {
                        continue;
                    }
                    let instant = resolve_local(
                        text(record, date_key),
                        record[time_key].as_str().unwrap_or(fallback),
                        tz,
                    )?;
                    if let Some(span) = intersection((instant - buffer, instant + buffer), day_span)
                    {
                        blockers.push((record, label.into(), span));
                    }
                }
                let local_date = office
                    .local(day_span.0 + Duration::hours(12))
                    .with_timezone(&destination)
                    .date_naive();
                let at_stay = date(text(record, "checkIn"))
                    .map(|d| d <= local_date)
                    .unwrap_or(false)
                    && date(text(record, "checkOut"))
                        .map(|d| d > local_date)
                        .unwrap_or(false);
                if at_stay {
                    for (field, label) in [("internet", "connectivity"), ("workspace", "workspace")]
                    {
                        if record["facilities"][field].as_bool() != Some(true) {
                            let verified_missing =
                                record["facilities"][field].as_bool() == Some(false);
                            warnings.push(json!({"type":if verified_missing {format!("missing_{label}")} else {format!("unverified_{label}")},"recordId":record["id"],"severity":"warning",
                                "message":format!("{}: {} {}",text(record,"name"),label,if verified_missing {"is marked unavailable"} else {"has not been verified"})}));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut conflicts = vec![];
    for (block, span) in &blocks {
        for (record, kind, blocked) in &blockers {
            for working_part in subtract(vec![*span], &work_breaks) {
                if let Some(overlap) = intersection(working_part, *blocked) {
                    conflicts.push(json!({"type":kind,"recordId":record["id"],"workBlockId":block["id"],"severity":"conflict",
                    "message":format!("Work overlaps {}: {}",kind,text(record,"name")),"start":overlap.0.to_rfc3339(),"end":overlap.1.to_rfc3339()}));
                }
            }
        }
    }
    let work_spans = subtract(
        merge(blocks.iter().map(|(_, s)| *s).collect()),
        &work_breaks,
    );
    let scheduled = duration(&work_spans);
    let scheduled_overlap = duration(
        &work_spans
            .iter()
            .flat_map(|span| {
                office_spans
                    .iter()
                    .filter_map(|office| intersection(*span, *office))
            })
            .collect::<Vec<_>>(),
    );
    let mut restrictions = personal_windows(settings, destination, day_span)?;
    for (block, span) in &blocks {
        let permitted = duration(
            &restrictions
                .iter()
                .filter_map(|r| intersection(*r, *span))
                .collect::<Vec<_>>(),
        );
        if permitted < (span.1 - span.0).num_minutes() {
            conflicts.push(json!({"type":"local_day_restriction","workBlockId":block["id"],"severity":"conflict","message":"Work falls outside your destination-local working restrictions"}));
        }
    }
    // Workspace opening windows, when explicitly supplied, further constrain suggestions.
    if let Some(stay) = planned_stay {
        let alternate = records.iter().find(|r| {
            text(r, "kind") == "place" && text(r, "id") == text(stay, "alternateWorkspaceId")
        });
        let hours = alternate
            .map(|r| &r["workspaceHours"])
            .filter(|v| v.is_object())
            .unwrap_or(&stay["workspaceHours"]);
        if let Some(opening) = hours.as_object() {
            let opening_settings = json!({"localDayRestrictions":opening});
            let venue_zone = alternate
                .and_then(|r| text(r, "timezone").parse::<Tz>().ok())
                .unwrap_or(destination);
            let opening_spans = personal_windows(&opening_settings, venue_zone, day_span)?;
            restrictions = merge(
                restrictions
                    .iter()
                    .flat_map(|s| opening_spans.iter().filter_map(|o| intersection(*s, *o)))
                    .collect(),
            );
            for (block, span) in &blocks {
                if duration(
                    &opening_spans
                        .iter()
                        .filter_map(|s| intersection(*s, *span))
                        .collect::<Vec<_>>(),
                ) < (span.1 - span.0).num_minutes()
                {
                    conflicts.push(json!({"type":"workspace_closed","recordId":stay["id"],"workBlockId":block["id"],"severity":"conflict","message":"Work extends outside the saved workspace opening hours"}));
                }
            }
        }
    }
    let commute = planned_stay
        .filter(|s| !text(s, "alternateWorkspaceId").is_empty())
        .and_then(|s| s["workspaceCommuteMinutes"].as_i64())
        .filter(|v| (0..=240).contains(v));
    if planned_stay.is_some_and(|s| !text(s, "alternateWorkspaceId").is_empty())
        && commute.is_none()
    {
        warnings.push(json!({"type":"unknown_workspace_commute","message":"Alternate workspace commute duration is unknown; proposed times do not establish travel feasibility."}));
    }
    if let Some(minutes) = commute {
        for (block, span) in &blocks {
            let expanded = (
                span.0 - Duration::minutes(minutes),
                span.1 + Duration::minutes(minutes),
            );
            for (record, _, event) in &blockers {
                if intersection(expanded, *event).is_some() {
                    conflicts.push(json!({"type":"workspace_commute","recordId":record["id"],"workBlockId":block["id"],"message":"Recorded commute to the alternate workspace overlaps another activity."}));
                }
            }
        }
    }
    let mut busy: Vec<Interval> = blockers
        .iter()
        .map(|(_, _, s)| {
            (
                s.0 - Duration::minutes(commute.unwrap_or(0)),
                s.1 + Duration::minutes(commute.unwrap_or(0)),
            )
        })
        .collect();
    busy.extend(work_spans.iter().copied());
    busy.extend(work_breaks.iter().copied());
    let free = subtract(restrictions, &busy);
    let free_office = merge(
        free.iter()
            .flat_map(|span| {
                office_spans
                    .iter()
                    .filter_map(|office| intersection(*span, *office))
            })
            .collect(),
    );
    let mut needed = (required - scheduled)
        .max(0)
        .max((overlap_required - scheduled_overlap).max(0));
    let mut suggested = vec![];
    let mut take = |spans: &[Interval]| {
        for span in spans {
            if needed <= 0 {
                break;
            }
            let minutes = (span.1 - span.0).num_minutes().min(needed);
            if minutes > 0 {
                suggested.push((span.0, span.0 + Duration::minutes(minutes)));
                needed -= minutes;
            }
        }
    };
    take(&free_office);
    take(&subtract(free.clone(), &office_spans));
    let proposed_minutes = duration(&suggested);
    let proposed_overlap = duration(
        &suggested
            .iter()
            .flat_map(|span| {
                office_spans
                    .iter()
                    .filter_map(|office| intersection(*span, *office))
            })
            .collect::<Vec<_>>(),
    );
    let destination_start = office_spans
        .first()
        .map(|s| s.0.with_timezone(&destination).to_rfc3339());
    let destination_end = office_spans
        .last()
        .map(|s| s.1.with_timezone(&destination).to_rfc3339());
    let mut result = json!({
        "date":selected.to_string(),"dateBasis":basis,"officeDate":office_date.to_string(),
        "officeTimezone":office.label(),"destinationTimezone":destination_name,"currentTimezone":current_name,
        "isWorkday":is_workday,"officeOpen":office_open,"officeNow":office.local(now).to_rfc3339(),
        "localNow":now.with_timezone(&current).to_rfc3339(),"destinationNow":now.with_timezone(&destination).to_rfc3339(),
        "officeOffsetSeconds":office.local(now).offset().local_minus_utc(),
        "localOffsetSeconds":now.with_timezone(&current).offset().fix().local_minus_utc(),
        "requiredHours":required as f64/60.0,"minOverlapHours":overlap_required as f64/60.0,
        "requiredMinutes":required,"requiredOverlapMinutes":overlap_required,
        "officeStart":office_spans.first().map(|s|office.local(s.0).to_rfc3339()),
        "officeEnd":office_spans.last().map(|s|office.local(s.1).to_rfc3339()),
        "destinationStart":destination_start,"destinationEnd":destination_end,
        "localStart":destination_start,"localEnd":destination_end
    });
    let details = json!({
        "officeWindows":office_spans.iter().map(|s|display_span(*s,office,destination)).collect::<Vec<_>>(),
        "blocks":blocks.iter().map(|(record,span)|json!({"record":record,"window":display_span(*span,office,destination)})).collect::<Vec<_>>(),
        "scheduledMinutes":scheduled,"overlapMinutes":scheduled_overlap,
        "scheduledBreakMinutes":duration(&blocks.iter().map(|(_,s)|*s).collect::<Vec<_>>())-scheduled,
        "missingWorkMinutes":(required-scheduled).max(0),"missingOverlapMinutes":(overlap_required-scheduled_overlap).max(0),
        "meetsRequirement":scheduled>=required && scheduled_overlap>=overlap_required && conflicts.is_empty(),
        "suggestedBlocks":suggested.iter().map(|s|display_span(*s,office,destination)).collect::<Vec<_>>(),
        "suggestionMeetsHours":scheduled+proposed_minutes>=required && scheduled_overlap+proposed_overlap>=overlap_required,
        "suggestionStatus":"Draft proposal only; existing work and confirmed travel are never moved",
        "conflicts":conflicts,"warnings":warnings,
        "currentLocation":settings["currentLocation"],
        "plannedLocation":planned_stay.map(|s|json!({"stayId":s["id"],"name":s["name"],"address":s["address"],"timezone":s["timezone"],"basis":"confirmed stay on selected date"})),
        "clockBasis":"Current location is owner-configured; destination windows use the selected plan. No IP geolocation is used."
    });
    result
        .as_object_mut()
        .ok_or("Internal response error")?
        .extend(
            details
                .as_object()
                .ok_or("Internal response error")?
                .clone(),
        );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn get(day: &str, tz: &str) -> Value {
        window(
            &[],
            &json!({}),
            &json!({"date":day,"timezone":tz,"now":"2026-07-06T17:00:00Z"}),
        )
        .unwrap()
    }

    #[test]
    fn pacific_dst_changes_office_instants() {
        let winter = get("2026-01-05", "UTC");
        let summer = get("2026-07-06", "UTC");
        assert_eq!(winter["destinationStart"], "2026-01-05T16:00:00+00:00");
        assert_eq!(summer["destinationStart"], "2026-07-06T15:00:00+00:00");
        assert_eq!(summer["officeOpen"], true);
    }

    #[test]
    fn office_workday_can_be_next_destination_day() {
        let r = get("2026-07-06", "Asia/Tokyo");
        assert_eq!(r["destinationStart"], "2026-07-07T00:00:00+09:00");
        assert_eq!(r["officeDate"], "2026-07-06");
        assert_eq!(r["suggestionMeetsHours"], true);
    }

    #[test]
    fn weekend_off_and_holiday_exception() {
        assert_eq!(get("2026-07-05", "UTC")["isWorkday"], false);
        let r = window(
            &[],
            &json!({"holidays":["2026-07-06"]}),
            &json!({"date":"2026-07-06"}),
        )
        .unwrap();
        assert_eq!(r["requiredMinutes"], 0);
        let r=window(&[],&json!({"workExceptions":[{"date":"2026-07-05","working":true,"workHours":4,"overlapHours":2}]}),&json!({"date":"2026-07-05"})).unwrap();
        assert_eq!(r["requiredMinutes"], 240);
    }

    #[test]
    fn nonexistent_and_ambiguous_local_times_require_resolution() {
        assert!(resolve_local("2026-03-08", "02:30", "America/Los_Angeles")
            .unwrap_err()
            .contains("does not exist"));
        assert!(resolve_local("2026-11-01", "01:30", "America/Los_Angeles")
            .unwrap_err()
            .contains("occurs twice"));
        assert!(resolve_local("2026-03-08", "03:30", "America/Los_Angeles").is_ok());
    }

    #[test]
    fn overlapping_blocks_do_not_double_count_hours() {
        let blocks = vec![
            json!({"id":"a","kind":"workBlock","start":"2026-07-06T15:00:00Z","end":"2026-07-06T20:00:00Z"}),
            json!({"id":"b","kind":"workBlock","start":"2026-07-06T18:00:00Z","end":"2026-07-06T23:00:00Z"}),
        ];
        let r = window(&blocks, &json!({}), &json!({"date":"2026-07-06"})).unwrap();
        assert_eq!(r["scheduledMinutes"], 480);
        assert_eq!(r["overlapMinutes"], 480);
        assert_eq!(r["meetsRequirement"], true);
    }

    #[test]
    fn recurring_work_preserves_local_hours_across_daylight_saving_change() {
        let records = vec![
            json!({"id":"work-series","kind":"workBlock","date":"2026-03-06","startTime":"08:00","endTime":"16:00","timezone":"America/Los_Angeles","recurrence":"daily","recurrenceUntil":"2026-03-10"}),
        ];
        let before = window(
            &records,
            &json!({}),
            &json!({"date":"2026-03-06","timezone":"UTC"}),
        )
        .unwrap();
        let after = window(
            &records,
            &json!({}),
            &json!({"date":"2026-03-09","timezone":"UTC"}),
        )
        .unwrap();
        assert_eq!(
            before["blocks"][0]["window"]["start"],
            "2026-03-06T16:00:00+00:00"
        );
        assert_eq!(
            after["blocks"][0]["window"]["start"],
            "2026-03-09T15:00:00+00:00"
        );
        assert_eq!(after["scheduledMinutes"], 480);
        assert_eq!(after["overlapMinutes"], 480);
        assert_eq!(after["meetsRequirement"], true);
    }

    #[test]
    fn work_requires_overlap_not_only_total_hours() {
        let blocks = vec![
            json!({"id":"a","kind":"workBlock","start":"2026-07-06T07:00:00Z","end":"2026-07-06T15:00:00Z"}),
        ];
        let r = window(&blocks, &json!({}), &json!({"date":"2026-07-06"})).unwrap();
        assert_eq!(r["scheduledMinutes"], 480);
        assert_eq!(r["overlapMinutes"], 0);
        assert_eq!(r["meetsRequirement"], false);
        assert_eq!(r["missingOverlapMinutes"], 240);
    }

    #[test]
    fn transit_and_checkin_conflicts_are_visible_without_moving_records() {
        let records = vec![
            json!({"id":"work","kind":"workBlock","start":"2026-07-06T15:00:00Z","end":"2026-07-06T23:00:00Z"}),
            json!({"id":"flight","kind":"transport","name":"Synthetic flight","departure":"2026-07-06T16:00:00Z","arrival":"2026-07-06T19:00:00Z"}),
            json!({"id":"stay","kind":"stay","name":"Synthetic apartment","status":"confirmed","checkIn":"2026-07-06","checkOut":"2026-07-10","checkInTime":"15:00","timezone":"Europe/London"}),
        ];
        let r = window(&records, &json!({}), &json!({"date":"2026-07-06"})).unwrap();
        assert_eq!(r["conflicts"].as_array().unwrap().len(), 1);
        assert_eq!(r["conflicts"][0]["type"], "transit");
        assert_eq!(r["meetsRequirement"], false);
        assert!(!r["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn restricted_destination_hours_can_make_overlap_impossible() {
        let r = window(
            &[],
            &json!({"localDayRestrictions":{"start":"09:00","end":"17:00"}}),
            &json!({"date":"2026-07-06","timezone":"Asia/Tokyo"}),
        )
        .unwrap();
        assert_eq!(r["suggestionMeetsHours"], false);
    }

    #[test]
    fn breaks_are_excluded_from_work_overlap_and_proposals() {
        let r = window(
            &[],
            &json!({"workBreaks":[{"start":"12:00","end":"13:00"}]}),
            &json!({"date":"2026-07-06"}),
        )
        .unwrap();
        assert_eq!(r["officeWindows"].as_array().unwrap().len(), 2);
        assert_eq!(r["suggestedBlocks"].as_array().unwrap().len(), 2);
        assert_eq!(r["suggestionMeetsHours"], true);
    }

    #[test]
    fn configured_break_minutes_reserve_actual_time_in_the_office_window() {
        let r = window(
            &[],
            &json!({"breakMinutes":60}),
            &json!({"date":"2026-07-06","timezone":"UTC"}),
        )
        .unwrap();
        assert_eq!(r["officeWindows"].as_array().unwrap().len(), 2);
        let minutes: i64 = r["officeWindows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["minutes"].as_i64().unwrap())
            .sum();
        assert_eq!(minutes, 480);
        assert_eq!(r["suggestionMeetsHours"], true);
        let long = window(
            &[],
            &json!({"breakMinutes":120}),
            &json!({"date":"2026-07-06","timezone":"UTC"}),
        )
        .unwrap();
        assert_eq!(long["suggestedBlocks"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn personal_break_does_not_close_office_or_count_as_work_or_become_suggestion() {
        let records = vec![
            json!({"id":"work","kind":"workBlock","start":"2026-07-06T15:00:00Z","end":"2026-07-07T00:00:00Z"}),
        ];
        let settings = json!({"breakMinutes":60});
        let r = window(
            &records,
            &settings,
            &json!({"date":"2026-07-06","now":"2026-07-06T19:30:00Z"}),
        )
        .unwrap();
        assert_eq!(r["officeOpen"], true);
        assert_eq!(r["scheduledMinutes"], 480);
        assert_eq!(r["scheduledBreakMinutes"], 60);
        let restricted =
            json!({"breakMinutes":120,"localDayRestrictions":{"start":"08:00","end":"17:00"}});
        let r = window(
            &[],
            &restricted,
            &json!({"date":"2026-07-06","timezone":"America/Los_Angeles"}),
        )
        .unwrap();
        assert_eq!(r["suggestionMeetsHours"], false);
        let total: i64 = r["suggestedBlocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["minutes"].as_i64().unwrap())
            .sum();
        assert_eq!(total, 420);
    }

    #[test]
    fn objective_during_a_configured_break_does_not_conflict_with_work() {
        let records = vec![
            json!({"id":"work","kind":"workBlock","start":"2026-07-06T15:00:00Z","end":"2026-07-07T00:00:00Z"}),
            json!({"id":"errand","kind":"objective","name":"Synthetic lunch errand","date":"2026-07-06","startTime":"12:15","endTime":"12:45","timezone":"America/Los_Angeles"}),
        ];
        let r = window(
            &records,
            &json!({"breakMinutes":60}),
            &json!({"date":"2026-07-06"}),
        )
        .unwrap();
        assert!(r["conflicts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn fixed_offset_requires_explicit_setting() {
        let r = window(
            &[],
            &json!({"officeTimezoneMode":"fixed","officeFixedOffsetMinutes":-480}),
            &json!({"date":"2026-07-06","timezone":"UTC"}),
        )
        .unwrap();
        assert_eq!(r["destinationStart"], "2026-07-06T16:00:00+00:00");
    }
}
