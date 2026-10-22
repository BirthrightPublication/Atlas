//! Expand local calendar recurrence without turning a day into 24 elapsed hours.
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Debug)]
struct Rule {
    frequency: String,
    interval: i64,
    weekdays: BTreeSet<u32>,
    until: Option<NaiveDate>,
}

fn date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("Invalid recurrence date {value}; use YYYY-MM-DD"))
}

fn weekday(value: &Value) -> Result<u32, String> {
    if let Some(number) = value.as_u64() {
        if (1..=7).contains(&number) {
            return Ok(number as u32);
        }
    }
    match value.as_str().unwrap_or("").to_ascii_lowercase().as_str() {
        "mo" | "mon" | "monday" => Ok(1),
        "tu" | "tue" | "tuesday" => Ok(2),
        "we" | "wed" | "wednesday" => Ok(3),
        "th" | "thu" | "thursday" => Ok(4),
        "fr" | "fri" | "friday" => Ok(5),
        "sa" | "sat" | "saturday" => Ok(6),
        "su" | "sun" | "sunday" => Ok(7),
        _ => Err("Recurrence weekdays must be Monday=1 through Sunday=7, or weekday names".into()),
    }
}

fn parse_rule(record: &Value) -> Result<Option<Rule>, String> {
    let recurrence = &record["recurrence"];
    if recurrence.is_null() || recurrence.as_str().is_some_and(str::is_empty) {
        return Ok(None);
    }
    let mut frequency = recurrence.as_str().unwrap_or("").to_ascii_lowercase();
    let mut interval = recurrence["interval"].as_i64().unwrap_or(1);
    let mut weekdays = BTreeSet::new();
    let mut until = record["recurrenceUntil"]
        .as_str()
        .or_else(|| recurrence["until"].as_str())
        .filter(|s| !s.is_empty())
        .map(date)
        .transpose()?;
    if let Some(name) = recurrence["frequency"].as_str() {
        frequency = name.to_ascii_lowercase();
    }
    if let Some(days) = recurrence["weekdays"].as_array() {
        for day in days {
            weekdays.insert(weekday(day)?);
        }
    }
    if let Some(ics) = recurrence["rule"].as_str() {
        for part in ics.split(';') {
            let (key, value) = part.split_once('=').ok_or("Malformed recurrence rule")?;
            match key.to_ascii_uppercase().as_str() {
                "FREQ" => frequency=value.to_ascii_lowercase(),
                "INTERVAL" => interval=value.parse().map_err(|_| "Invalid recurrence interval")?,
                "UNTIL" => {
                    let d=value.get(..8).and_then(|s| NaiveDate::parse_from_str(s,"%Y%m%d").ok()).ok_or("Invalid recurrence UNTIL date")?;
                    // Only date UNTIL is unambiguous for the supported local-day rules.
                    if value.len()!=8 { return Err("Timed RRULE UNTIL requires review; set an explicit local recurrenceUntil date".into()); }
                    if until.is_none() { until=Some(d); }
                }
                "BYDAY" => for day in value.split(',') { weekdays.insert(weekday(&json!(day))?); },
                _ => return Err(format!("Recurrence rule component {key} is not supported; use daily, weekly or monthly with explicit local dates")),
            }
        }
    }
    if !matches!(frequency.as_str(), "daily" | "weekly" | "monthly") {
        return Err("Recurrence frequency must be daily, weekly or monthly".into());
    }
    if !(1..=366).contains(&interval) {
        return Err("Recurrence interval must be between 1 and 366".into());
    }
    if !weekdays.is_empty() && frequency != "weekly" {
        return Err("Weekday selectors are supported for weekly recurrence only".into());
    }
    Ok(Some(Rule {
        frequency,
        interval,
        weekdays,
        until,
    }))
}

fn anchor(record: &Value) -> Result<NaiveDate, String> {
    if let Some(day) = record["date"].as_str().filter(|s| !s.is_empty()) {
        return date(day);
    }
    let instant = record["startAt"]
        .as_str()
        .or_else(|| record["start"].as_str())
        .ok_or("A recurring item needs a start date")?;
    let instant = DateTime::parse_from_rfc3339(instant)
        .map_err(|_| "Recurring start must include its timezone offset")?;
    let tz: Tz = record["timezone"]
        .as_str()
        .ok_or("Recurring timed items need an IANA timezone")?
        .parse()
        .map_err(|_| "Recurring timezone is invalid")?;
    Ok(instant.with_timezone(&tz).date_naive())
}

fn matches(rule: &Rule, anchor: NaiveDate, day: NaiveDate) -> bool {
    if day < anchor || rule.until.is_some_and(|until| day > until) {
        return false;
    }
    match rule.frequency.as_str() {
        "daily" => (day - anchor).num_days() % rule.interval == 0,
        "weekly" => {
            let start_week =
                anchor - Duration::days(anchor.weekday().num_days_from_monday().into());
            let week = (day - start_week).num_days() / 7;
            let day_match = if rule.weekdays.is_empty() {
                day.weekday() == anchor.weekday()
            } else {
                rule.weekdays.contains(&day.weekday().number_from_monday())
            };
            week % rule.interval == 0 && day_match
        }
        "monthly" => {
            let months = i64::from(day.year() - anchor.year()) * 12 + i64::from(day.month())
                - i64::from(anchor.month());
            months % rule.interval == 0 && day.day() == anchor.day()
        }
        _ => false,
    }
}

fn local_times(record: &Value, tz: Tz) -> Result<Option<(NaiveTime, NaiveTime, i64)>, String> {
    let from_text = |key: &str| -> Result<Option<NaiveTime>, String> {
        record[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| {
                NaiveTime::parse_from_str(s, "%H:%M")
                    .map_err(|_| format!("Invalid recurring {key}"))
            })
            .transpose()
    };
    let start_time = from_text("startTime")?;
    let end_time = from_text("endTime")?;
    if let (Some(a), Some(b)) = (start_time, end_time) {
        return Ok(Some((a, b, if b <= a { 1 } else { 0 })));
    }
    let start = record["startAt"]
        .as_str()
        .or_else(|| record["start"].as_str());
    let end = record["endAt"].as_str().or_else(|| record["end"].as_str());
    if let (Some(a), Some(b)) = (start, end) {
        if a.contains('T') && b.contains('T') {
            let a = DateTime::parse_from_rfc3339(a)
                .map_err(|_| "Invalid recurring start instant")?
                .with_timezone(&tz);
            let b = DateTime::parse_from_rfc3339(b)
                .map_err(|_| "Invalid recurring end instant")?
                .with_timezone(&tz);
            if b <= a {
                return Err("Recurring end must follow start".into());
            }
            return Ok(Some((
                a.time(),
                b.time(),
                (b.date_naive() - a.date_naive()).num_days(),
            )));
        }
    }
    Ok(None)
}

/// Nonrecurring records pass through unchanged. Recurring templates become only
/// the occurrences inside [start,end), retaining their template ID for editing.
pub fn expand(records: &[Value], start: &str, end: &str) -> Result<Vec<Value>, String> {
    let start = date(start)?;
    let end = date(end)?;
    let days = (end - start).num_days();
    if !(1..=366).contains(&days) {
        return Err("Recurrence range must span 1–366 days, with an exclusive end date".into());
    }
    let mut out = Vec::new();
    for record in records {
        if !matches!(record["kind"].as_str(), Some("objective" | "workBlock")) {
            out.push(record.clone());
            continue;
        }
        let Some(rule) = parse_rule(record)? else {
            out.push(record.clone());
            continue;
        };
        let base = anchor(record)?;
        let id = record["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Recurring records require stable IDs")?;
        let mut exceptions = BTreeSet::new();
        if let Some(values) = record["recurrenceExceptions"].as_array() {
            for value in values {
                exceptions.insert(date(
                    value
                        .as_str()
                        .ok_or("Recurrence exceptions must be local dates")?,
                )?);
            }
        }
        let tz = record["timezone"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<Tz>()
                    .map_err(|_| "Invalid recurrence timezone".to_owned())
            })
            .transpose()?;
        let times = tz.map(|tz| local_times(record, tz)).transpose()?.flatten();
        for offset in 0..days {
            let day = start + Duration::days(offset);
            if !matches(&rule, base, day) || exceptions.contains(&day) {
                continue;
            }
            let mut occurrence = record.clone();
            occurrence["date"] = json!(day.to_string());
            occurrence["occurrenceDate"] = json!(day.to_string());
            occurrence["occurrenceId"] = json!(format!("{id}:{day}"));
            occurrence["recurrenceParentId"] = json!(id);
            if let (Some(tz), Some((a, b, delta))) = (tz, times) {
                let ending = day
                    .checked_add_signed(Duration::days(delta))
                    .ok_or("Recurring end date overflow")?;
                let local_start = tz.from_local_datetime(&day.and_time(a)).single();
                let local_end = tz.from_local_datetime(&ending.and_time(b)).single();
                if let (Some(a), Some(b)) = (local_start, local_end) {
                    occurrence["startAt"] = json!(a.with_timezone(&Utc).to_rfc3339());
                    occurrence["endAt"] = json!(b.with_timezone(&Utc).to_rfc3339());
                    occurrence["start"] = occurrence["startAt"].clone();
                    occurrence["end"] = occurrence["endAt"].clone();
                    occurrence["startTime"] = json!(a.format("%H:%M").to_string());
                    occurrence["endTime"] = json!(b.format("%H:%M").to_string());
                } else {
                    return Err(format!("Recurring item {id} has an ambiguous or nonexistent local time on {day} in {tz}; change its time or add a recurrence exception."));
                }
            }
            if record["recurrenceCompletedDates"]
                .as_array()
                .or_else(|| record["completedOccurrences"].as_array())
                .is_some_and(|dates| dates.iter().any(|v| v.as_str() == Some(&day.to_string())))
            {
                occurrence["status"] = json!("done");
            }
            out.push(occurrence);
            if out.len() > 50_000 {
                return Err("Too many recurring occurrences; request a smaller date range".into());
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monthly_day_31_skips_short_months_without_drifting() {
        let r =
            json!({"id":"monthly","kind":"objective","date":"2026-01-31","recurrence":"monthly"});
        let result = expand(&[r], "2026-01-01", "2026-05-01").unwrap();
        assert_eq!(
            result
                .iter()
                .map(|v| v["date"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["2026-01-31", "2026-03-31"]
        );
    }
    #[test]
    fn until_is_inclusive_exceptions_and_range_end_are_exclusive() {
        let r = json!({"id":"daily","kind":"objective","date":"2026-10-01","recurrence":"daily","recurrenceUntil":"2026-10-04","recurrenceExceptions":["2026-10-02"]});
        let result = expand(&[r], "2026-10-01", "2026-10-04").unwrap();
        assert_eq!(
            result
                .iter()
                .map(|v| v["date"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["2026-10-01", "2026-10-03"]
        );
        assert_eq!(result[1]["id"], "daily");
        assert_eq!(result[1]["occurrenceId"], "daily:2026-10-03");
    }
    #[test]
    fn weekly_intervals_use_local_calendar_weeks() {
        let r = json!({"id":"weekly","kind":"objective","date":"2026-10-05","recurrence":{"frequency":"weekly","interval":2,"weekdays":[1,5]}});
        let result = expand(&[r], "2026-10-01", "2026-10-25").unwrap();
        assert_eq!(
            result
                .iter()
                .map(|v| v["date"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["2026-10-05", "2026-10-09", "2026-10-19", "2026-10-23"]
        );
    }
    #[test]
    fn work_local_time_survives_dst_instead_of_adding_24_hours() {
        let r = json!({"id":"work","kind":"workBlock","date":"2026-03-06","startTime":"09:00","endTime":"17:00","timezone":"America/Los_Angeles","recurrence":"daily"});
        let result = expand(&[r], "2026-03-07", "2026-03-10").unwrap();
        assert_eq!(result[0]["startAt"], "2026-03-07T17:00:00+00:00");
        assert_eq!(result[1]["startAt"], "2026-03-08T16:00:00+00:00");
        assert_eq!(result[1]["startTime"], "09:00");
    }
    #[test]
    fn actual_instant_work_templates_recur_in_the_named_local_timezone() {
        let r = json!({"id":"night","kind":"workBlock","start":"2026-03-07T06:00:00Z","end":"2026-03-07T10:00:00Z","timezone":"America/Los_Angeles","recurrence":"daily"});
        let result = expand(&[r], "2026-03-08", "2026-03-09").unwrap();
        assert_eq!(result[0]["startAt"], "2026-03-09T05:00:00+00:00");
        assert_eq!(result[0]["endAt"], "2026-03-09T09:00:00+00:00");
    }
    #[test]
    fn dst_gap_requires_explicit_review_and_can_be_excepted() {
        let mut r = json!({"id":"gap","kind":"objective","date":"2026-03-01","startTime":"02:30","endTime":"03:30","timezone":"America/Los_Angeles","recurrence":"daily"});
        assert!(expand(&[r.clone()], "2026-03-08", "2026-03-09")
            .unwrap_err()
            .contains("nonexistent"));
        r["recurrenceExceptions"] = json!(["2026-03-08"]);
        assert!(expand(&[r], "2026-03-08", "2026-03-09").unwrap().is_empty());
    }
    #[test]
    fn leap_monthly_date_and_nonrecurring_data_are_preserved() {
        let normal = json!({"id":"stay","kind":"stay","checkIn":"2026-01-01"});
        let r = json!({"id":"leap","kind":"objective","date":"2028-01-29","recurrence":"monthly"});
        let result = expand(&[normal.clone(), r], "2028-02-01", "2028-03-01").unwrap();
        assert_eq!(result[0], normal);
        assert_eq!(result[1]["date"], "2028-02-29");
        assert!(expand(&[], "2026-01-01", "2028-01-01").is_err());
    }
}
