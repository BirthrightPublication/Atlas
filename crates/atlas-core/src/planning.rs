//! Pure planning from saved evidence. These services never fetch inventory,
//! invent quotes, change a booking, or turn an estimate into a commitment.
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_AMOUNT: i64 = 9_007_199_254_740_991;

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn date(s: &str) -> Result<NaiveDate, String> {
    if s.len() != 10 {
        return Err("Use YYYY-MM-DD calendar dates".into());
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("Invalid calendar date: {s}"))
}
fn minor(v: &Value, key: &str) -> Result<i64, String> {
    v[key]
        .as_i64()
        .filter(|n| *n >= 0 && *n <= MAX_AMOUNT)
        .ok_or_else(|| format!("{key} must be a nonnegative exact amount in minor currency units"))
}
fn add(a: i64, b: i64) -> Result<i64, String> {
    a.checked_add(b)
        .filter(|n| n.unsigned_abs() <= MAX_AMOUNT as u64)
        .ok_or("Planning amount exceeds the exact range".into())
}
fn currency(s: &str) -> Result<&str, String> {
    if s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase()) {
        Ok(s)
    } else {
        Err("Use an uppercase three-letter currency code".into())
    }
}
fn interval(v: &Value) -> Result<(NaiveDate, NaiveDate, i64), String> {
    let a = date(text(v, "checkIn"))?;
    let b = date(text(v, "checkOut"))?;
    let nights = (b - a).num_days();
    if !(1..=3660).contains(&nights) {
        return Err("Stay dates must cover 1–3660 occupied nights with exclusive checkout".into());
    }
    Ok((a, b, nights))
}
fn month_end(day: NaiveDate) -> Result<NaiveDate, String> {
    if day.month() == 12 {
        NaiveDate::from_ymd_opt(day.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(day.year(), day.month() + 1, 1)
    }
    .ok_or("Date is outside the supported range".into())
}
fn date_target(monthly: i64, start: NaiveDate, end: NaiveDate) -> Result<i64, String> {
    let mut cursor = start;
    let mut total = 0;
    while cursor < end {
        let first =
            NaiveDate::from_ymd_opt(cursor.year(), cursor.month(), 1).ok_or("Invalid month")?;
        let next = month_end(cursor)?;
        let stop = next.min(end);
        total = add(
            total,
            crate::money::allocate(
                monthly,
                (next - first).num_days(),
                (cursor - first).num_days(),
                (stop - first).num_days(),
            )?,
        )?;
        cursor = next;
    }
    Ok(total)
}
fn preferences(settings: &Value, payload: &Value) -> Result<Value, String> {
    let mut p = settings["preferences"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(overrides) = payload.get("preferences") {
        for (key, value) in overrides
            .as_object()
            .ok_or("Temporary preferences must be an object")?
        {
            p.insert(key.clone(), value.clone());
        }
    }
    for key in ["monthlyTargetMinor", "stretchCapMinor"] {
        if let Some(value) = p.get(key) {
            if !value.is_null() {
                minor(&json!({key:value}), key)?;
            }
        }
    }
    let target = p
        .get("monthlyTargetMinor")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cap = p
        .get("stretchCapMinor")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if target > 0 && cap > 0 && cap < target {
        return Err("The stretch cap cannot be below the monthly target".into());
    }
    Ok(Value::Object(p))
}
fn words(value: &Value) -> BTreeSet<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        Value::String(s) => s
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => BTreeSet::new(),
    }
}
fn source(record: &Value) -> Value {
    json!({"recordId":record["id"],"provider":record["provider"],"url":record.get("sourceUrl").unwrap_or(&record["url"]),
        "checkedAt":record["checkedAt"],"sourceIds":record["sourceIds"],"basis":"Saved record; no live provider request"})
}

/// A saved total already includes all amounts marked included. Only explicitly
/// excluded, mandatory components are added. Missing amounts remain unknown.
fn payable(record: &Value) -> Result<(i64, bool, Vec<String>), String> {
    let mut total = minor(record, "totalMinor")?;
    let mut complete = record["feesComplete"].as_bool() == Some(true);
    let mut missing = Vec::new();
    if let Some(components) = record["priceComponents"].as_array() {
        for part in components {
            if part["mandatory"].as_bool() != Some(true) {
                continue;
            }
            match part["includedInTotal"].as_bool() {
                Some(true) => {}
                Some(false) => {
                    let c = part["currency"]
                        .as_str()
                        .unwrap_or(text(record, "currency"));
                    if c != text(record, "currency") {
                        complete = false;
                        missing.push(format!(
                            "{} has a different currency; record the payable total separately",
                            text(part, "name")
                        ));
                        continue;
                    }
                    match minor(part, "amountMinor") {
                        Ok(n) => total = add(total, n)?,
                        Err(_) => {
                            complete = false;
                            missing.push(format!("{} amount is unknown", text(part, "name")));
                        }
                    }
                }
                None => {
                    complete = false;
                    missing.push(format!(
                        "{} inclusion in total is unknown",
                        text(part, "name")
                    ));
                }
            }
        }
    }
    if record["feesComplete"].as_bool() != Some(true) {
        missing.push("Mandatory fees or taxes are not confirmed complete".into());
    }
    Ok((total, complete, missing))
}

fn linked(records: &[Value], anchor: &Value) -> Vec<Value> {
    let id = text(anchor, "id");
    let trip = text(anchor, "tripId");
    let ids: BTreeSet<&str> = ["transportIds", "additionalStayIds", "journeyCostIds"]
        .iter()
        .flat_map(|key| {
            anchor[*key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect();
    let mut found = BTreeMap::new();
    for record in records {
        let rid = text(record, "id");
        let kind = text(record, "kind");
        if rid == id
            || text(record, "status") == "cancelled"
            || !matches!(kind, "stay" | "transport" | "expense")
        {
            continue;
        }
        let explicit = ids.contains(rid)
            || (!id.is_empty() && (text(record, "quoteId") == id || text(record, "stayId") == id));
        let trip_transport =
            kind == "transport" && !trip.is_empty() && text(record, "tripId") == trip;
        if explicit || trip_transport {
            found.insert(rid.to_owned(), record.clone());
        }
    }
    found.into_values().collect()
}

fn journey_costs(records: &[Value], anchor: &Value, base_minor: i64) -> Result<Value, String> {
    let base_currency = currency(text(anchor, "currency"))?;
    let mut totals = BTreeMap::from([(base_currency.to_owned(), base_minor)]);
    let mut components = vec![
        json!({"recordId":anchor["id"],"kind":anchor["kind"],"name":anchor["name"],"currency":base_currency,"amountMinor":base_minor,"source":source(anchor)}),
    ];
    let mut unknown = vec![];
    for record in linked(records, anchor) {
        let c = match currency(text(&record, "currency")) {
            Ok(v) => v,
            Err(_) => {
                unknown.push(json!({"recordId":record["id"],"reason":"Currency is unknown"}));
                continue;
            }
        };
        let result = if text(&record, "kind") == "expense" {
            minor(&record, "amountMinor").map(|v| (v, true, Vec::new()))
        } else {
            payable(&record)
        };
        match result {
            Ok((amount, complete, issues)) => {
                let previous = *totals.get(c).unwrap_or(&0);
                totals.insert(c.into(), add(previous, amount)?);
                components.push(json!({"recordId":record["id"],"kind":record["kind"],"name":record["name"],"currency":c,"amountMinor":amount,"feesComplete":complete,"source":source(&record)}));
                for issue in issues {
                    unknown.push(json!({"recordId":record["id"],"reason":issue}));
                }
                if text(&record, "kind") == "transport" {
                    for (key, label) in [
                        ("baggageIncluded", "Baggage inclusion/cost"),
                        ("transfersIncluded", "Required transfer inclusion/cost"),
                    ] {
                        if record[key].as_bool() != Some(true) {
                            unknown.push(json!({"recordId":record["id"],"reason":format!("{label} is not confirmed in the recorded total")}));
                        }
                    }
                }
            }
            Err(_) => unknown.push(json!({"recordId":record["id"],"reason":"Cost is unknown"})),
        }
    }
    Ok(
        json!({"totalsByCurrency":totals,"components":components,"unknownComponents":unknown,
        "coverage":"Saved accommodation plus explicitly linked transport, transfers, expenses and additional stays. Unrecorded necessities and live availability are not assessed.",
        "isCompleteJourneyQuote":false}),
    )
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct RankingKey {
    requirement: i32,
    workspace: i32,
    stale: i32,
    affordability: i32,
    complete: i32,
    known_cost: i64,
    interest: i32,
    surroundings: i32,
    name: String,
    id: String,
}

/// Rank saved, date-specific quotes within comparable date/currency groups.
pub fn rank(records: &[Value], settings: &Value, payload: &Value) -> Result<Value, String> {
    let prefs = preferences(settings, payload)?;
    let requested_dates = match (payload["checkIn"].as_str(), payload["checkOut"].as_str()) {
        (None, None) => None,
        (Some(_), Some(_)) => Some(interval(payload)?),
        _ => return Err("Supply both checkIn and checkOut for date-specific comparison".into()),
    };
    let requested_currency = payload["currency"].as_str().map(currency).transpose()?;
    let preference_currency = prefs["currency"]
        .as_str()
        .or_else(|| settings["currency"].as_str())
        .unwrap_or("USD");
    currency(preference_currency)?;
    let quote_ids: Option<BTreeSet<&str>> = payload["quoteIds"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect());
    let now = payload["now"]
        .as_str()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|v| v.with_timezone(&Utc))
                .map_err(|_| "now must be an ISO instant".to_owned())
        })
        .transpose()?
        .unwrap_or_else(Utc::now);
    let max_age = payload["maxAgeDays"].as_i64();
    if max_age.is_some_and(|n| n < 0 || n > 3660) {
        return Err("maxAgeDays must be between 0 and 3660".into());
    }
    let required: Vec<&str> = [
        ("refrigeratorRequired", "refrigerator"),
        ("workspaceRequired", "workspace"),
        ("kitchenRequired", "kitchen"),
        ("internetRequired", "internet"),
    ]
    .into_iter()
    .filter_map(|(setting, field)| {
        if prefs[setting].as_bool() == Some(true) {
            Some(field)
        } else {
            None
        }
    })
    .collect();
    let interest_prefs = words(&prefs["interests"]);
    let mut groups: BTreeMap<(String, String, String), Vec<(RankingKey, Value)>> = BTreeMap::new();
    let mut skipped = vec![];
    for quote in records.iter().filter(|r| text(r, "kind") == "quote") {
        if quote_ids
            .as_ref()
            .is_some_and(|ids| !ids.contains(text(quote, "id")))
        {
            continue;
        }
        let skip = |reason: String| json!({"id":quote["id"],"name":quote["name"],"reason":reason});
        if text(quote, "status") == "cancelled" {
            skipped.push(skip("Offer is cancelled".into()));
            continue;
        }
        let (start, end, nights) = match interval(quote) {
            Ok(v) => v,
            Err(e) => {
                skipped.push(skip(e));
                continue;
            }
        };
        if requested_dates.is_some_and(|(a, b, _)| a != start || b != end) {
            skipped.push(skip("Quoted dates differ from the requested dates; no price was prorated or discount invented".into()));
            continue;
        }
        let c = match currency(text(quote, "currency")) {
            Ok(v) => v,
            Err(e) => {
                skipped.push(skip(e));
                continue;
            }
        };
        if requested_currency.is_some_and(|desired| desired != c) {
            skipped.push(skip(format!(
                "Quote is in {c}; no implicit currency conversion applied"
            )));
            continue;
        }
        let (payable_minor, fees_complete, fee_issues) = match payable(quote) {
            Ok(v) => v,
            Err(e) => {
                skipped.push(skip(e));
                continue;
            }
        };
        let mut missing = vec![];
        let mut unavailable = vec![];
        for field in &required {
            match quote["facilities"][*field].as_bool() {
                Some(true) => {}
                Some(false) => unavailable.push(*field),
                None => missing.push(*field),
            }
        }
        let requirement = if !unavailable.is_empty() {
            2
        } else if !missing.is_empty() {
            1
        } else {
            0
        };
        let workspace = if prefs["workspacePriority"].as_bool() == Some(true) {
            match quote["facilities"]["workspace"].as_bool() {
                Some(true) => 0,
                None => 1,
                Some(false) => 2,
            }
        } else {
            0
        };
        let mut reasons = Vec::new();
        if !missing.is_empty() {
            reasons.push(format!(
                "Required facilities need verification: {}",
                missing.join(", ")
            ));
        }
        if !unavailable.is_empty() {
            reasons.push(format!(
                "Required facilities unavailable: {}",
                unavailable.join(", ")
            ));
        }
        if workspace > 0 {
            reasons.push(if workspace == 1 {
                "Laptop workspace is unverified".into()
            } else {
                "Laptop workspace is marked unavailable".into()
            });
        }
        let target = if c == preference_currency {
            prefs["monthlyTargetMinor"]
                .as_i64()
                .filter(|n| *n > 0)
                .map(|n| date_target(n, start, end))
                .transpose()?
        } else {
            None
        };
        let cap = if c == preference_currency {
            prefs["stretchCapMinor"]
                .as_i64()
                .filter(|n| *n > 0)
                .map(|n| date_target(n, start, end))
                .transpose()?
        } else {
            None
        };
        if c != preference_currency {
            reasons.push(format!("Personal target is in {preference_currency}; affordability was not converted into {c}"));
        }
        let stretch_reason = text(quote, "stretchReason").trim();
        let (affordability, affordability_label) = if cap.is_some_and(|n| payable_minor > n) {
            (4, "above_stretch_cap")
        } else if target.is_some_and(|n| payable_minor > n) {
            if cap.is_none() {
                (3, "over_target_no_stretch_cap")
            } else if stretch_reason.is_empty() {
                (3, "stretch_requires_reason")
            } else {
                (2, "stretch_with_recorded_reason")
            }
        } else if target.is_some() {
            (0, "within_personal_target")
        } else {
            (1, "target_not_comparable_or_unset")
        };
        match affordability_label {
            "above_stretch_cap"=>reasons.push("The full known total exceeds the saved stretch cap; the cap was not raised".into()),
            "over_target_no_stretch_cap"=>reasons.push("The known total exceeds the personal target and no stretch cap is configured".into()),
            "stretch_requires_reason"=>reasons.push("The total is above target; a preference-based reason is required before recommending this stretch option".into()),
            "stretch_with_recorded_reason"=>reasons.push(format!("Recorded stretch justification: {stretch_reason}")),
            _=>{},
        }
        if !fees_complete {
            reasons.push("Known total is a lower bound; missing mandatory fees/taxes may change affordability".into());
        }
        let mut tags = words(&quote["interests"]);
        tags.extend(words(&quote["tags"]));
        let matched: Vec<_> = interest_prefs.intersection(&tags).cloned().collect();
        let scenic = prefs["scenicPreferred"].as_bool() == Some(true)
            && quote["facilities"]["scenicView"]
                .as_bool()
                .or_else(|| quote["scenicView"].as_bool())
                == Some(true);
        let surroundings = text(&prefs, "surroundings");
        let surroundings_match = surroundings.is_empty()
            || surroundings == "flexible"
            || words(&quote["surroundings"]).contains(&surroundings.to_lowercase());
        let preferred_type = text(&prefs, "preferredType");
        let type_match = match preferred_type {
            "entire" => matches!(
                text(quote, "type"),
                "apartment" | "home" | "entire_apartment" | "entire_home"
            ),
            "room" => text(quote, "type") == "room",
            "vehicle" => text(quote, "type") == "vehicle",
            _ => true,
        };
        let checked = text(quote, "checkedAt");
        let checked_day = if checked.len() == 10 {
            date(checked).ok()
        } else {
            DateTime::parse_from_rfc3339(checked)
                .ok()
                .map(|v| v.date_naive())
        };
        let expires = quote["expiresAt"]
            .as_str()
            .filter(|v| !v.is_empty())
            .map(|v| {
                DateTime::parse_from_rfc3339(v)
                    .map(|d| d.with_timezone(&Utc))
                    .map_err(|_| "Quote expiration must be a timezone-aware instant".to_owned())
            })
            .transpose()?;
        let stale = expires.is_some_and(|d| d <= now)
            || max_age.is_some_and(|age| {
                checked_day.is_some_and(|d| (now.date_naive() - d).num_days() > age)
            });
        let freshness = if stale {
            "stale"
        } else if checked_day.is_some_and(|d| d > now.date_naive()) {
            "invalid_future_check_date"
        } else if checked_day.is_some() {
            "saved_timestamp_only"
        } else {
            "unknown"
        };
        if stale {
            reasons.push(
                "Recorded quote is expired or older than your requested freshness limit".into(),
            );
        }
        let journey = journey_costs(records, quote, payable_minor)?;
        let comparable_journey = journey["totalsByCurrency"]
            .as_object()
            .filter(|v| v.len() == 1)
            .and_then(|v| v.get(c))
            .and_then(Value::as_i64);
        if comparable_journey.is_none() {
            reasons.push(
                "Linked journey costs use multiple currencies; no combined total is claimed".into(),
            );
        }
        let eligible = requirement == 0
            && workspace == 0
            && fees_complete
            && affordability < 3
            && !stale
            && freshness != "invalid_future_check_date";
        let key = RankingKey {
            requirement,
            workspace,
            stale: i32::from(stale || freshness == "invalid_future_check_date"),
            affordability,
            complete: if fees_complete { 0 } else { 1 },
            known_cost: comparable_journey.unwrap_or(payable_minor),
            interest: -(matched.len() as i32 + i32::from(scenic) + i32::from(type_match)),
            surroundings: if surroundings_match { 0 } else { 1 },
            name: text(quote, "name").to_lowercase(),
            id: text(quote, "id").into(),
        };
        let result = json!({"id":quote["id"],"name":quote["name"],"record":quote,"checkIn":start.to_string(),"checkOut":end.to_string(),"currency":c,"nights":nights,
            "quotedTotalMinor":quote["totalMinor"],"knownPayableMinor":payable_minor,"nightlyMinor":payable_minor as f64/nights as f64,
            "feesComplete":fees_complete,"unknownPriceComponents":fee_issues,"requiredFacilitiesMissing":missing,"requiredFacilitiesUnavailable":unavailable,
            "requirementsVerified":requirement==0,"workspaceVerified":quote["facilities"]["workspace"].as_bool()==Some(true),
            "targetForDatesMinor":target,"stretchForDatesMinor":cap,"affordability":affordability_label,"stretchReason":if stretch_reason.is_empty(){None}else{Some(stretch_reason)},
            "interestMatches":matched,"surroundingsMatch":surroundings_match,"preferredTypeMatch":type_match,"scenicMatch":scenic,
            "freshness":freshness,"source":source(quote),"journey":journey,"recommendedForReview":eligible,"reasons":reasons});
        groups
            .entry((start.to_string(), end.to_string(), c.into()))
            .or_default()
            .push((key, result));
    }
    let mut output = vec![];
    let mut ranked = vec![];
    for ((start, end, c), mut entries) in groups {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let offers: Vec<Value> = entries
            .into_iter()
            .enumerate()
            .map(|(index, (_, mut value))| {
                value["rankWithinGroup"] = json!(index + 1);
                value
            })
            .collect();
        ranked.extend(offers.clone());
        output.push(json!({"checkIn":start,"checkOut":end,"currency":c,"nights":offers.first().map(|o|o["nights"].clone()),
            "targetForDatesMinor":offers.first().map(|o|o["targetForDatesMinor"].clone()),"stretchForDatesMinor":offers.first().map(|o|o["stretchForDatesMinor"].clone()),"offers":offers}));
    }
    Ok(
        json!({"groups":output,"ranked":ranked,"skipped":skipped,"preferences":prefs,
        "rankingBasis":"Within identical dates and currency: verified requirements, workspace suitability, affordability, known total, recorded interests and surroundings. Unknown facilities are never verified compliance.",
        "targetBasis":"Personal monthly envelopes allocated over each actual calendar month. Quotes themselves are never repriced or assumed to receive a monthly discount.",
        "coverage":"Saved date-specific offers only. Ranking is not a claim about global inventory, current availability, live crowd levels, or the cheapest provider.","liveSearchPerformed":false}),
    )
}

/// Locate the preceding plan, falling back only to an owner-entered current place.
pub fn preceding_location(
    records: &[Value],
    settings: &Value,
    payload: &Value,
) -> Result<Value, String> {
    let selected = date(text(payload, "date"))?;
    let exclude = text(payload, "excludeStayId");
    let trip = text(payload, "tripId");
    let mut stays: Vec<(&Value, NaiveDate, NaiveDate)> = vec![];
    for r in records.iter().filter(|r| {
        text(r, "kind") == "stay" && text(r, "status") == "confirmed" && text(r, "id") != exclude
    }) {
        if !trip.is_empty() && text(r, "tripId") != trip {
            continue;
        }
        let (a, b, _) = interval(r)?;
        if a < selected {
            stays.push((r, a, b));
        }
    }
    let previous = stays
        .iter()
        .filter(|(_, a, b)| *a < selected && *b > selected)
        .max_by_key(|(_, a, _)| *a)
        .or_else(|| {
            stays
                .iter()
                .filter(|(_, _, b)| *b <= selected)
                .max_by_key(|(_, _, b)| *b)
        });
    if let Some((stay, _, end)) = previous {
        return Ok(
            json!({"date":selected.to_string(),"basis":"preceding_confirmed_stay","location":{
            "stayId":stay["id"],"name":stay["name"],"address":stay["address"],"latitude":stay["latitude"],"longitude":stay["longitude"],"timezone":stay["timezone"]},
            "planned":true,"currentLocationChanged":false,"gapNights":(selected-*end).num_days().max(0),"overlapsSelectedDate":*end>selected,
            "note":"This is a stored planned origin, not proof of current physical location. Transfers and intervening unrecorded travel need review."}),
        );
    }
    let current = settings["currentLocation"]
        .as_str()
        .filter(|s| !s.trim().is_empty());
    Ok(
        json!({"date":selected.to_string(),"basis":if current.is_some(){"owner_entered_current_location"}else{"unknown"},
        "location":current.map(|name|json!({"name":name,"timezone":settings["currentTimezone"]})),"planned":false,"currentLocationChanged":false,
        "note":if current.is_some(){"No preceding confirmed stay matches. This fallback is the owner's current-location setting; confirm the trip origin."}else{"No preceding confirmed stay or owner-entered current location is available. Choose an origin before route search."}}),
    )
}

/// A what-if estimate deliberately leaves records, payments and provider bookings
/// unchanged. Historical average nightly price is a scenario basis, not an offer.
pub fn what_if(records: &[Value], _settings: &Value, payload: &Value) -> Result<Value, String> {
    if let Some(id) = payload["transportId"].as_str() {
        let original = records
            .iter()
            .find(|r| {
                text(r, "kind") == "transport"
                    && text(r, "id") == id
                    && text(r, "status") != "cancelled"
            })
            .ok_or("Select an active saved transport record")?;
        let amount = minor(payload, "totalMinor")?;
        let c = currency(text(original, "currency"))?;
        // A separate hypothetical month-of-use projection; never rewrite payments
        // or conversion history, including on an already paid reservation.
        let request = json!({"month":payload["month"],"currency":c});
        let before = crate::money::summary(records, _settings, &request)?;
        let mut proposed = records.to_vec();
        let candidate = proposed.iter_mut().find(|r| text(r, "id") == id).unwrap();
        candidate["totalMinor"] = json!(amount);
        candidate["status"] = json!("confirmed");
        let after = crate::money::summary(&proposed, _settings, &request)?;
        return Ok(
            json!({"estimateOnly":true,"recordsChanged":false,"paymentsChanged":false,"currency":c,"month":payload["month"],"before":before,"after":after,"scenario":{"recordId":id,"totalMinor":amount,"assumption":"This transport is included at the entered estimate. Payment cash flow and historical currency conversions are not repriced."}}),
        );
    }
    let id = text(payload, "stayId");
    if id.is_empty() {
        return Err("Select a stayId to extend".into());
    }
    let stay = records
        .iter()
        .find(|r| text(r, "kind") == "stay" && text(r, "id") == id)
        .ok_or("Stay not found")?;
    if text(stay, "status") == "cancelled" {
        return Err("A cancelled stay cannot be extended; create a separate draft".into());
    }
    let (start, old_end, old_nights) = interval(stay)?;
    let new_end = if let Some(s) = payload["newCheckOut"]
        .as_str()
        .or_else(|| payload["checkOut"].as_str())
    {
        date(s)?
    } else {
        let extra = payload["additionalNights"]
            .as_i64()
            .ok_or("Choose newCheckOut or additionalNights")?;
        if !(1..=3660).contains(&extra) {
            return Err("Additional nights must be between 1 and 3660".into());
        }
        old_end
            .checked_add_signed(Duration::days(extra))
            .ok_or("Date overflow")?
    };
    let new_nights = (new_end - start).num_days();
    if new_end <= old_end || new_nights > 3660 {
        return Err(
            "An extension must increase checkout and keep the stay within 3660 nights".into(),
        );
    }
    if let Some(extra) = payload["additionalNights"].as_i64() {
        if (new_end - old_end).num_days() != extra {
            return Err("newCheckOut and additionalNights disagree".into());
        }
    }
    let c = currency(text(stay, "currency"))?;
    let total = minor(stay, "totalMinor")?;
    let numerator = i128::from(total) * i128::from(new_nights);
    let denominator = i128::from(old_nights);
    let estimated = i64::try_from((numerator + denominator / 2) / denominator)
        .ok()
        .filter(|n| *n <= MAX_AMOUNT)
        .ok_or("Extension estimate exceeds exact range")?;
    let additional = estimated - total;
    let mut months = vec![];
    let mut cursor = start;
    while cursor < new_end {
        let next = month_end(cursor)?;
        let until = next.min(new_end);
        let original = if cursor < old_end {
            crate::money::allocate(
                total,
                old_nights,
                (cursor - start).num_days(),
                (until.min(old_end) - start).num_days(),
            )?
        } else {
            0
        };
        let scenario = crate::money::allocate(
            estimated,
            new_nights,
            (cursor - start).num_days(),
            (until - start).num_days(),
        )?;
        months.push(json!({"month":cursor.format("%Y-%m").to_string(),"currency":c,"originalMinor":original,"estimatedMinor":scenario,"deltaMinor":scenario-original}));
        cursor = next;
    }
    let mut conflicts = vec![];
    let mut affected_drafts = vec![];
    for record in records {
        if text(record, "id") == id {
            continue;
        }
        if text(record, "kind") == "draft"
            && text(record, "stayId") == id
            && text(record, "status") == "pending"
        {
            affected_drafts.push(record["id"].clone());
        }
        if text(record, "status") == "cancelled" {
            continue;
        }
        if text(record, "kind") == "stay" && text(record, "status") == "confirmed" {
            let (a, b, _) = interval(record)?;
            if a < new_end && b > old_end {
                conflicts.push(json!({"type":"overlapping_stay","recordId":record["id"],"name":record["name"],"message":"Extension overlaps a confirmed stay; no booking was moved"}));
            }
        }
        if text(record, "kind") == "transport" {
            let related = text(record, "stayId") == id
                || (!text(stay, "tripId").is_empty()
                    && text(record, "tripId") == text(stay, "tripId"));
            if related {
                let instant = DateTime::parse_from_rfc3339(text(record, "departure"))
                    .map_err(|_| "Linked departure needs a timezone-aware instant")?;
                let tz = text(stay, "timezone")
                    .parse::<chrono_tz::Tz>()
                    .map_err(|_| "Stay timezone is invalid")?;
                let departure = instant.with_timezone(&tz).date_naive();
                if departure >= old_end && departure < new_end {
                    conflicts.push(json!({"type":"departure_during_extension","recordId":record["id"],"name":record["name"],"message":"Recorded departure falls during the extension; availability and change/cancellation costs need review"}));
                }
            }
        }
    }
    let journey = journey_costs(records, stay, estimated)?;
    Ok(
        json!({"stayId":id,"name":stay["name"],"currency":c,"estimateOnly":true,"requiresNewQuote":true,"recordsChanged":false,
        "original":{"checkIn":start.to_string(),"checkOut":old_end.to_string(),"nights":old_nights,"totalMinor":total},
        "scenario":{"checkIn":start.to_string(),"checkOut":new_end.to_string(),"nights":new_nights,"additionalNights":new_nights-old_nights,"estimatedAdditionalMinor":additional,"estimatedTotalMinor":estimated},
        "basis":{"source":source(stay),"numeratorMinor":total,"denominatorNights":old_nights,"rounding":"Nearest minor unit, half up","description":"Historical recorded total divided by occupied nights. No assumed new discount, provider availability or valid extension quote."},
        "monthlyEstimates":months,"journey":journey,"conflicts":conflicts,"affectedDraftIds":affected_drafts,
        "paymentsChanged":false,"confirmedPlansChanged":false,
        "limitations":["One-time fees and discounts in the original total may not scale to an extension.","Mandatory fees, taxes, utilities and internet for new nights need a fresh provider quote.","No purchase, booking change, host contact or payment was made."]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn quote(id: &str, total: i64) -> Value {
        json!({"id":id,"kind":"quote","name":id,"type":"apartment","status":"tentative","checkIn":"2027-02-01","checkOut":"2027-03-01","currency":"USD","totalMinor":total,"feesComplete":true,"facilities":{"refrigerator":true,"workspace":true},"checkedAt":"2027-01-10"})
    }
    fn prefs() -> Value {
        json!({"currency":"USD","preferences":{"refrigeratorRequired":true,"workspacePriority":true,"monthlyTargetMinor":100000,"stretchCapMinor":160000,"interests":["nature","architecture"]}})
    }
    fn ranked(records: &[Value]) -> Value {
        rank(records, &prefs(), &json!({"now":"2027-01-11T00:00:00Z"})).unwrap()
    }

    #[test]
    fn verified_requirements_and_work_suitability_outrank_a_cheaper_unknown() {
        let good = quote("verified", 90000);
        let mut unknown = quote("unknown", 30000);
        unknown["facilities"]["refrigerator"] = Value::Null;
        let mut bad = quote("unavailable", 10000);
        bad["facilities"]["refrigerator"] = json!(false);
        let r = ranked(&[unknown, bad, good]);
        assert_eq!(r["ranked"][0]["id"], "verified");
        assert_eq!(r["ranked"][1]["recommendedForReview"], false);
        assert_eq!(r["ranked"][2]["id"], "unavailable");
    }
    #[test]
    fn prices_are_never_reused_for_different_dates() {
        let r = rank(
            &[quote("one", 90000)],
            &prefs(),
            &json!({"checkIn":"2027-02-02","checkOut":"2027-03-02"}),
        )
        .unwrap();
        assert!(r["ranked"].as_array().unwrap().is_empty());
        assert!(r["skipped"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("no price was prorated"));
    }
    #[test]
    fn calendar_month_target_is_not_assumed_to_be_thirty_nights() {
        let feb = quote("feb", 95000);
        let mut jan = quote("jan", 95000);
        jan["checkIn"] = json!("2027-01-01");
        jan["checkOut"] = json!("2027-01-29");
        let r = ranked(&[feb, jan]);
        let all = r["ranked"].as_array().unwrap();
        let feb = all.iter().find(|v| v["id"] == "feb").unwrap();
        let jan = all.iter().find(|v| v["id"] == "jan").unwrap();
        assert_eq!(feb["targetForDatesMinor"], 100000);
        assert_eq!(feb["affordability"], "within_personal_target");
        assert!(jan["targetForDatesMinor"].as_i64().unwrap() < 95000);
        assert_eq!(jan["affordability"], "stretch_requires_reason");
    }
    #[test]
    fn unknown_fees_and_unjustified_stretch_never_become_verified_recommendations() {
        let mut missing = quote("fees", 80000);
        missing["feesComplete"] = json!(false);
        let stretch = quote("stretch", 140000);
        let mut allowed = quote("reason", 140000);
        allowed["stretchReason"] =
            json!("Recorded quiet workspace and verified refrigerator suit this trip");
        let mut above = quote("above", 170000);
        above["stretchReason"] = json!("Scenic view");
        let r = ranked(&[missing, stretch, allowed, above]);
        let rows = r["ranked"].as_array().unwrap();
        for id in ["fees", "stretch", "above"] {
            assert_eq!(
                rows.iter().find(|v| v["id"] == id).unwrap()["recommendedForReview"],
                false
            );
        }
        assert_eq!(
            rows.iter().find(|v| v["id"] == "reason").unwrap()["recommendedForReview"],
            true
        );
        assert_eq!(
            rows.iter().find(|v| v["id"] == "above").unwrap()["stretchForDatesMinor"],
            160000
        );
    }
    #[test]
    fn currencies_group_separately_and_interest_is_evidence_based() {
        let mut eur = quote("eur", 100);
        eur["currency"] = json!("EUR");
        let plain = quote("plain", 90000);
        let mut matched = quote("matched", 90000);
        matched["tags"] = json!(["nature"]);
        let r = ranked(&[plain, matched, eur]);
        assert_eq!(r["groups"].as_array().unwrap().len(), 2);
        let usd = r["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["currency"] == "USD")
            .unwrap();
        assert_eq!(usd["offers"][0]["id"], "matched");
        let eur = r["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["currency"] == "EUR")
            .unwrap();
        assert!(eur["targetForDatesMinor"].is_null());
    }
    #[test]
    fn explicit_mandatory_additions_and_linked_journey_costs_are_not_double_counted() {
        let mut q = quote("q", 80000);
        q["tripId"] = json!("t");
        q["priceComponents"] = json!([{"name":"Cleaning","mandatory":true,"includedInTotal":true,"amountMinor":5000},{"name":"Tax","mandatory":true,"includedInTotal":false,"amountMinor":2000}]);
        let leg = json!({"id":"leg","kind":"transport","tripId":"t","currency":"USD","totalMinor":20000,"feesComplete":true,"baggageIncluded":true,"transfersIncluded":true});
        let r = ranked(&[q, leg]);
        assert_eq!(r["ranked"][0]["knownPayableMinor"], 82000);
        assert_eq!(r["ranked"][0]["journey"]["totalsByCurrency"]["USD"], 102000);
    }
    #[test]
    fn preceding_plan_overrides_current_location_without_mutation() {
        let records = vec![
            json!({"id":"s","kind":"stay","name":"Synthetic planned origin","status":"confirmed","checkIn":"2027-02-01","checkOut":"2027-03-01","timezone":"Europe/Lisbon"}),
        ];
        let settings = json!({"currentLocation":"Synthetic current place","currentTimezone":"UTC"});
        let r = preceding_location(&records, &settings, &json!({"date":"2027-03-03"})).unwrap();
        assert_eq!(r["location"]["stayId"], "s");
        assert_eq!(r["gapNights"], 2);
        assert_eq!(r["planned"], true);
        let r = preceding_location(&[], &settings, &json!({"date":"2027-03-03"})).unwrap();
        assert_eq!(r["basis"], "owner_entered_current_location");
        assert!(
            preceding_location(&[], &json!({}), &json!({"date":"2027-03-03"})).unwrap()["location"]
                .is_null()
        );
    }

    #[test]
    fn stay_active_before_selected_trip_is_preferred_over_an_older_completed_stay() {
        let records = vec![
            json!({"id":"old","kind":"stay","status":"confirmed","checkIn":"2027-01-01","checkOut":"2027-02-01"}),
            json!({"id":"active","kind":"stay","status":"confirmed","checkIn":"2027-02-01","checkOut":"2027-03-05"}),
        ];
        let r = preceding_location(&records, &json!({}), &json!({"date":"2027-03-03"})).unwrap();
        assert_eq!(r["location"]["stayId"], "active");
        assert_eq!(r["overlapsSelectedDate"], true);
    }

    #[test]
    fn expired_price_does_not_outrank_a_current_saved_quote_with_same_requirements() {
        let mut expired = quote("expired", 10000);
        expired["expiresAt"] = json!("2027-01-01T00:00:00Z");
        let r = ranked(&[expired, quote("current", 90000)]);
        assert_eq!(r["ranked"][0]["id"], "current");
        assert_eq!(r["ranked"][1]["freshness"], "stale");
    }
    #[test]
    fn extension_is_explicit_estimate_conserves_month_allocation_and_preserves_records() {
        let records = vec![
            json!({"id":"s","kind":"stay","name":"Synthetic apartment","status":"confirmed","currency":"USD","totalMinor":90000,"checkIn":"2026-12-12","checkOut":"2027-01-11","timezone":"Europe/Lisbon"}),
            json!({"id":"next","kind":"stay","name":"Next confirmed stay","status":"confirmed","checkIn":"2027-01-11","checkOut":"2027-01-20"}),
        ];
        let before = records.clone();
        let r = what_if(
            &records,
            &json!({}),
            &json!({"stayId":"s","additionalNights":7}),
        )
        .unwrap();
        assert_eq!(r["scenario"]["estimatedAdditionalMinor"], 21000);
        assert_eq!(r["scenario"]["estimatedTotalMinor"], 111000);
        assert_eq!(r["estimateOnly"], true);
        assert_eq!(r["requiresNewQuote"], true);
        assert_eq!(
            r["monthlyEstimates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["estimatedMinor"].as_i64().unwrap())
                .sum::<i64>(),
            111000
        );
        assert_eq!(r["conflicts"][0]["recordId"], "next");
        assert_eq!(records, before);
    }
    #[test]
    fn extension_reports_departure_conflicts_and_keeps_foreign_journey_totals_separate() {
        let records = vec![
            json!({"id":"s","kind":"stay","name":"Synthetic stay","status":"confirmed","tripId":"t","currency":"USD","totalMinor":10000,"checkIn":"2027-02-01","checkOut":"2027-02-11","timezone":"Europe/Lisbon"}),
            json!({"id":"train","kind":"transport","name":"Synthetic departure","status":"confirmed","tripId":"t","departure":"2027-02-11T12:00:00Z","currency":"EUR","totalMinor":2500}),
        ];
        let r = what_if(
            &records,
            &json!({}),
            &json!({"stayId":"s","additionalNights":2}),
        )
        .unwrap();
        assert_eq!(r["conflicts"][0]["type"], "departure_during_extension");
        assert_eq!(r["journey"]["totalsByCurrency"]["USD"], 12000);
        assert_eq!(r["journey"]["totalsByCurrency"]["EUR"], 2500);
        assert_eq!(r["paymentsChanged"], false);
    }
    #[test]
    fn unsupported_or_inconsistent_scenarios_fail_without_side_effects() {
        assert!(what_if(
            &[],
            &json!({}),
            &json!({"stayId":"missing","additionalNights":7})
        )
        .is_err());
        assert!(rank(
            &[],
            &prefs(),
            &json!({"preferences":{"stretchCapMinor":50000}})
        )
        .is_err());
        let mut q = quote("expired", 80000);
        q["expiresAt"] = json!("2027-01-01T00:00:00Z");
        assert_eq!(ranked(&[q])["ranked"][0]["recommendedForReview"], false);
    }
}
