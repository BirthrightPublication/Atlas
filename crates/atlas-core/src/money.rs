//! Exact, currency-separated forecasting. Stored amounts are integer minor units.
//! A change to conversion settings never mutates a payment's settled history.
use chrono::{Datelike, NaiveDate, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};

const MAX_MONEY: i64 = 9_007_199_254_740_991;

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn amount(v: &Value, key: &str) -> Result<i64, String> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_i64()
            .filter(|n| n.unsigned_abs() <= MAX_MONEY as u64)
            .ok_or_else(|| format!("{key} must be a safe integer amount in minor currency units")),
    }
}

fn checked_sum(a: i64, b: i64) -> Result<i64, String> {
    a.checked_add(b)
        .filter(|n| n.unsigned_abs() <= MAX_MONEY as u64)
        .ok_or_else(|| "Monetary total exceeds the supported exact range".to_owned())
}

fn date(value: &str) -> Result<NaiveDate, String> {
    if value.len() != 10 {
        return Err(format!("Expected an ISO calendar date, received {value:?}"));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("Invalid calendar date: {value}"))
}

fn month_bounds(month: &str) -> Result<(NaiveDate, NaiveDate), String> {
    if month.len() != 7 {
        return Err("month must be YYYY-MM".into());
    }
    let start = date(&format!("{month}-01"))?;
    let next = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
    }
    .ok_or_else(|| "Month is outside the supported calendar range".to_owned())?;
    Ok((start, next))
}

fn currency(settings: &Value, payload: &Value) -> Result<String, String> {
    let value = payload["currency"]
        .as_str()
        .or_else(|| settings["currency"].as_str())
        .unwrap_or("USD");
    if value.len() != 3 || !value.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err("Currency must be a three-letter uppercase ISO code".into());
    }
    Ok(value.into())
}

fn committed(record: &Value) -> bool {
    !matches!(
        text(record, "status"),
        "tentative" | "cancelled" | "dismissed"
    ) && !record["alternative"].as_bool().unwrap_or(false)
}

fn today(settings: &Value, payload: &Value) -> Result<NaiveDate, String> {
    if let Some(value) = payload["today"].as_str() {
        return date(value);
    }
    let tz = settings["currentTimezone"]
        .as_str()
        .unwrap_or("UTC")
        .parse::<chrono_tz::Tz>()
        .map_err(|_| "Invalid current timezone".to_owned())?;
    Ok(Utc::now().with_timezone(&tz).date_naive())
}

fn in_range(value: NaiveDate, start: NaiveDate, end: NaiveDate) -> bool {
    value >= start && value < end
}

/// Conserves every minor unit. Remainder units go to the earliest occupied days.
pub fn allocate(total: i64, days: i64, from_day: i64, to_day: i64) -> Result<i64, String> {
    if total < 0 || days <= 0 || from_day < 0 || to_day < from_day || to_day > days {
        return Err("Invalid allocation interval or negative cost".into());
    }
    let base = total / days;
    let remainder = total % days;
    let before =
        |n: i64| -> i128 { i128::from(base) * i128::from(n) + i128::from(n.min(remainder)) };
    i64::try_from(before(to_day) - before(from_day)).map_err(|_| "Allocation overflow".to_owned())
}

fn stay_allocation(
    record: &Value,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Option<Value>, String> {
    let check_in = date(text(record, "checkIn"))?;
    let check_out = date(text(record, "checkOut"))?;
    let nights = (check_out - check_in).num_days();
    if nights <= 0 {
        return Err(format!(
            "Stay {} must check out after check-in",
            text(record, "name")
        ));
    }
    let a = check_in.max(start);
    let b = check_out.min(end);
    if a >= b {
        return Ok(None);
    }
    let total = amount(record, "totalMinor")?;
    let allocated = allocate(
        total,
        nights,
        (a - check_in).num_days(),
        (b - check_in).num_days(),
    )?;
    Ok(Some(json!({
        "stayId": record["id"], "name": record["name"], "currency": record["currency"],
        "periodStart": a.to_string(), "periodEndExclusive": b.to_string(),
        "nightCount": (b-a).num_days(), "totalNights": nights,
        "amountMinor": allocated, "totalMinor": total,
        "nightlyMinor": total as f64 / nights as f64
    })))
}

fn record_date(record: &Value) -> Result<Option<NaiveDate>, String> {
    if text(record, "kind") == "transport" {
        if text(record, "departure").is_empty() {
            return Ok(None);
        }
        let instant = chrono::DateTime::parse_from_rfc3339(text(record, "departure"))
            .map_err(|_| "Transport departure must have an explicit UTC offset".to_owned())?;
        let zone = text(record, "departureTimezone");
        if zone.is_empty() {
            return Ok(Some(instant.date_naive()));
        }
        let tz = zone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| "Invalid departure timezone".to_owned())?;
        return Ok(Some(instant.with_timezone(&tz).date_naive()));
    }
    match record["date"].as_str().filter(|s| !s.is_empty()) {
        Some(s) => Ok(Some(date(s)?)),
        None => Ok(None),
    }
}

fn linked_expense<'a>(records: &'a [Value], objective: &Value) -> Vec<&'a Value> {
    let id = text(objective, "id");
    if id.is_empty() {
        return vec![];
    }
    records
        .iter()
        .filter(|r| {
            text(r, "kind") == "expense"
                && committed(r)
                && text(r, "objectiveId") == id
                && matches_occurrence(r, objective)
        })
        .collect()
}

fn matches_occurrence(linked: &Value, objective: &Value) -> bool {
    let occurrence = text(objective, "occurrenceDate");
    occurrence.is_empty()
        || linked["occurrenceDate"]
            .as_str()
            .or_else(|| linked["date"].as_str())
            == Some(occurrence)
}

fn is_duplicate_expense(record: &Value, records: &[Value]) -> bool {
    let payment_id = text(record, "paymentId");
    !payment_id.is_empty()
        && records
            .iter()
            .any(|r| text(r, "kind") == "payment" && text(r, "id") == payment_id)
}

fn selected_budget<'a>(records: &'a [Value], month: &str, currency: &str) -> Option<&'a Value> {
    records
        .iter()
        .filter(|r| {
            text(r, "kind") == "budget"
                && text(r, "month") == month
                && text(r, "currency") == currency
        })
        .max_by_key(|r| text(r, "updatedAt"))
}

fn eligible_day(day: NaiveDate, budget: &Value) -> bool {
    if let Some(excluded) = budget["excludedDates"].as_array() {
        if excluded
            .iter()
            .any(|v| v.as_str() == Some(day.to_string().as_str()))
        {
            return false;
        }
    }
    match budget["eligibleWeekdays"].as_array() {
        Some(days) => days
            .iter()
            .any(|v| v.as_u64() == Some(day.weekday().number_from_monday() as u64)),
        None => true,
    }
}

fn summarize_range(
    records: &[Value],
    _settings: &Value,
    month: &str,
    currency: &str,
    start: NaiveDate,
    end: NaiveDate,
    now: NaiveDate,
) -> Result<Value, String> {
    let (month_start, month_end) = month_bounds(month)?;
    let month_days = (month_end - month_start).num_days();
    let days = (end - start).num_days();
    let null = Value::Null;
    let budget = selected_budget(records, month, currency).unwrap_or(&null);
    let proportion = |total| {
        allocate(
            total,
            month_days,
            (start - month_start).num_days(),
            (end - month_start).num_days(),
        )
    };
    let mut accommodation = 0;
    let mut transport = 0;
    let mut objectives = 0;
    let mut objective_cash_flow = 0;
    let mut expenses = 0;
    let mut flexible_spent = 0;
    let mut flexible_committed = 0;
    let mut booking_cash_flow = 0;
    let mut settled_cash_flow = 0;
    let mut occupied_nights = 0;
    let mut occupied_dates = HashSet::new();
    let mut allocations = Vec::new();
    let mut excluded_currencies = BTreeSet::new();
    let mut warnings = Vec::new();
    let mut unscheduled = Vec::new();
    for record in records {
        let kind = text(record, "kind");
        if !matches!(
            kind,
            "stay" | "transport" | "objective" | "expense" | "payment"
        ) {
            continue;
        }
        if !committed(record) {
            continue;
        }
        if text(record, "currency") != currency {
            if !text(record, "currency").is_empty() {
                excluded_currencies.insert(text(record, "currency").to_owned());
            }
            continue;
        }
        match kind {
            "stay" => {
                if let Some(allocation) = stay_allocation(record, start, end)? {
                    accommodation =
                        checked_sum(accommodation, amount(&allocation, "amountMinor")?)?;
                    occupied_nights += allocation["nightCount"].as_i64().unwrap_or(0);
                    let mut d = date(text(&allocation, "periodStart"))?;
                    let until = date(text(&allocation, "periodEndExclusive"))?;
                    while d < until {
                        occupied_dates.insert(d);
                        d = d.succ_opt().ok_or("Date overflow")?;
                    }
                    allocations.push(allocation);
                }
            }
            "transport" | "objective" => {
                let total = amount(record, "totalMinor")?;
                if total < 0 {
                    return Err(
                        "Booking costs must not be negative; record a linked refund payment".into(),
                    );
                }
                if kind == "objective" && !linked_expense(records, record).is_empty() {
                    continue;
                }
                if let Some(d) = record_date(record)? {
                    if in_range(d, start, end) {
                        if kind == "transport" {
                            transport = checked_sum(transport, total)?;
                        } else {
                            objectives = checked_sum(objectives, total)?;
                            // Explicit payment obligations replace the objective's expected
                            // spending date, while its date-of-use cost stays independent.
                            let has_payments = !text(record, "id").is_empty()
                                && records.iter().any(|p| {
                                    text(p, "kind") == "payment"
                                        && text(p, "bookingId") == text(record, "id")
                                        && committed(p)
                                        && matches_occurrence(p, record)
                                });
                            if !has_payments {
                                objective_cash_flow = checked_sum(objective_cash_flow, total)?;
                            }
                            if record["usesAllowance"].as_bool().unwrap_or(true) {
                                flexible_committed = checked_sum(flexible_committed, total)?;
                            }
                        }
                    }
                } else if total > 0 {
                    unscheduled.push(json!({"id": record["id"],"kind":kind,"amountMinor":total,"reason":"No scheduled date"}));
                }
            }
            "expense" => {
                if is_duplicate_expense(record, records) {
                    continue;
                }
                if let Some(d) = record_date(record)? {
                    if in_range(d, start, end) {
                        let total = amount(record, "amountMinor")?;
                        expenses = checked_sum(expenses, total)?;
                        settled_cash_flow = checked_sum(settled_cash_flow, total)?;
                        if record["usesAllowance"].as_bool().unwrap_or(true) {
                            flexible_spent = checked_sum(flexible_spent, total)?;
                        }
                    }
                }
            }
            "payment" => {
                if let Some(d) = record_date(record)? {
                    if in_range(d, start, end) {
                        let total = amount(record, "amountMinor")?;
                        let ty = text(record, "type");
                        if matches!(ty, "refund" | "deposit_refund") && total > 0 {
                            return Err("Refund amounts must be negative minor units".into());
                        }
                        booking_cash_flow = checked_sum(booking_cash_flow, total)?;
                        if text(record, "status") == "settled" {
                            settled_cash_flow = checked_sum(settled_cash_flow, total)?;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut recurring = 0;
    if let Some(items) = budget["recurringExpenses"].as_array() {
        for item in items {
            recurring = checked_sum(recurring, proportion(amount(item, "amountMinor")?)?)?;
        }
    }
    let allowance = proportion(amount(budget, "allowanceMinor")?)?;
    let allowance_remaining = allowance
        .saturating_sub(flexible_spent)
        .saturating_sub(flexible_committed)
        .max(0);
    let forecast = [
        accommodation,
        transport,
        objectives,
        expenses,
        recurring,
        allowance_remaining,
    ]
    .into_iter()
    .try_fold(0, checked_sum)?;
    let cash_flow = [booking_cash_flow, expenses, recurring, objective_cash_flow]
        .into_iter()
        .try_fold(0, checked_sum)?;
    let income = amount(budget, "incomeMinor")?;
    let opening_balance = amount(budget, "openingBalanceMinor")?;
    let reserve = amount(budget, "reserveMinor")?;
    let available = [
        opening_balance,
        income,
        -reserve,
        -cash_flow,
        -allowance_remaining,
    ]
    .into_iter()
    .try_fold(0, checked_sum)?;
    let mut remaining_days = 0;
    let mut d = start.max(now);
    while d < end {
        if eligible_day(d, budget) {
            remaining_days += 1;
        }
        d = d.succ_opt().ok_or("Date overflow")?;
    }
    let mut payment_statuses = Vec::new();
    for booking in records.iter().filter(|r| {
        matches!(text(r, "kind"), "stay" | "transport") && text(r, "currency") == currency
    }) {
        let booking_id = text(booking, "id");
        let mut settled = 0;
        let mut scheduled = 0;
        let mut refunded = false;
        let mut has_schedule = false;
        let mut deposit_tied_up = 0;
        let mut unconverted_payments = Vec::new();
        for payment in records.iter().filter(|r| {
            text(r, "kind") == "payment"
                && !booking_id.is_empty()
                && text(r, "bookingId") == booking_id
                && committed(r)
        }) {
            has_schedule = true;
            let actual = amount(payment, "amountMinor")?;
            let total = if text(payment, "currency") == currency {
                actual
            } else if text(payment, "bookingCurrency") == currency
                && payment
                    .get("bookingAmountMinor")
                    .is_some_and(|v| !v.is_null())
            {
                let historic = amount(payment, "bookingAmountMinor")?;
                if historic.signum() != actual.signum() {
                    return Err("Historic booking equivalent must have the same sign as the actual settlement".into());
                }
                historic
            } else {
                unconverted_payments.push(json!({
                    "paymentId": payment["id"], "currency": payment["currency"],
                    "amountMinor": actual, "type": payment["type"], "status": payment["status"]
                }));
                continue;
            };
            if matches!(text(payment, "type"), "deposit" | "deposit_refund") {
                if text(payment, "status") == "settled" {
                    deposit_tied_up = checked_sum(deposit_tied_up, total)?;
                }
                continue;
            }
            scheduled = checked_sum(scheduled, total)?;
            if text(payment, "status") == "settled" {
                settled = checked_sum(settled, total)?;
                refunded |= text(payment, "type") == "refund";
            }
        }
        let total = amount(booking, "totalMinor")?;
        let status = if refunded && settled <= 0 {
            "refunded"
        } else if settled >= total {
            "paid"
        } else if settled > 0 {
            "part-paid"
        } else {
            "unpaid"
        };
        payment_statuses.push(json!({"bookingId":booking["id"],"status":status,"currency":currency,
            "settledMinor":settled,"scheduledMinor":scheduled,"outstandingMinor":(total-settled).max(0),
            "depositTiedUpMinor":deposit_tied_up,"hasPaymentSchedule":has_schedule,
            "settlementBasisIncomplete": !unconverted_payments.is_empty(),
            "unconvertedPayments": unconverted_payments}));
        if !unconverted_payments.is_empty() {
            warnings.push(format!(
                "{} has foreign-currency payments without a recorded {} booking equivalent; its payment status is incomplete",
                text(booking,"name"), currency
            ));
        }
        if total > 0 && !has_schedule && committed(booking) {
            warnings.push(format!(
                "{} has no payment schedule; its cash-flow date is unknown",
                text(booking, "name")
            ));
        }
    }
    Ok(json!({
        "month":month,"currency":currency,"daysInMonth":month_days,"daysInPeriod":days,
        "periodStart":start.to_string(),"periodEndExclusive":end.to_string(),
        "accommodationMinor":accommodation,"transportMinor":transport,"objectivesMinor":objectives,
        "expensesMinor":expenses,"recurringMinor":recurring,"forecastMinor":forecast,
        "bookingCashFlowMinor":booking_cash_flow,"cashFlowMinor":cash_flow,
        "settledCashFlowMinor":settled_cash_flow,"incomeMinor":income,"openingBalanceMinor":opening_balance,
        "reserveMinor":reserve,"availableMinor":available,"allowanceMinor":allowance,
        "allowanceSpentMinor":flexible_spent,"allowanceCommittedMinor":flexible_committed,
        "allowanceRemainingMinor":allowance_remaining,"remainingDays":remaining_days,
        "dailyForecastMinor": if days>0 {Some(forecast as f64/days as f64)} else {None},
        "dailyAllowanceMinor": if remaining_days>0 {Some(allowance_remaining as f64/remaining_days as f64)} else {None},
        "occupiedNights":occupied_nights,"uniqueOccupiedDays":occupied_dates.len(),
        "nightlyAccommodationMinor":if occupied_nights>0 {Some(accommodation as f64/occupied_nights as f64)} else {None},
        "excludedCurrencies":excluded_currencies,"paymentStatuses":payment_statuses,"allocations":allocations,
        "warnings":warnings,"unscheduled":unscheduled,"hasBudget":!budget.is_null(),
        "availableBasis":"Opening cash + income − scheduled cash outflows − remaining flexible allowance − reserve. Payments without dates are excluded and flagged."
    }))
}

pub fn summary(records: &[Value], settings: &Value, payload: &Value) -> Result<Value, String> {
    let month = payload["month"]
        .as_str()
        .ok_or("month is required (YYYY-MM)")?;
    let (start, end) = month_bounds(month)?;
    let expanded = crate::recurrence::expand(records, &start.to_string(), &end.to_string())?;
    summarize_range(
        &expanded,
        settings,
        month,
        &currency(settings, payload)?,
        start,
        end,
        today(settings, payload)?,
    )
}

/// Forecast over an explicit end-exclusive period. Monthly allowances and recurring
/// costs use exact calendar-day allocation; stays still use occupied nights.
pub fn period_summary(
    records: &[Value],
    settings: &Value,
    payload: &Value,
) -> Result<Value, String> {
    let start = date(payload["start"].as_str().ok_or("start date is required")?)?;
    let end = date(
        payload["end"]
            .as_str()
            .ok_or("end date (exclusive) is required")?,
    )?;
    if end <= start || (end - start).num_days() > 3660 {
        return Err("Select between 1 and 3660 days".into());
    }
    let c = currency(settings, payload)?;
    let now = today(settings, payload)?;
    let mut cursor = start;
    let mut forecast = 0;
    let mut cash_flow = 0;
    let mut accommodation = 0;
    let mut nights = 0;
    let mut months = Vec::new();
    while cursor < end {
        let month = cursor.format("%Y-%m").to_string();
        let (_, next) = month_bounds(&month)?;
        let expanded =
            crate::recurrence::expand(records, &cursor.to_string(), &next.min(end).to_string())?;
        let result = summarize_range(&expanded, settings, &month, &c, cursor, next.min(end), now)?;
        forecast = checked_sum(forecast, amount(&result, "forecastMinor")?)?;
        cash_flow = checked_sum(cash_flow, amount(&result, "cashFlowMinor")?)?;
        accommodation = checked_sum(accommodation, amount(&result, "accommodationMinor")?)?;
        nights += result["occupiedNights"].as_i64().unwrap_or(0);
        months.push(result);
        cursor = next;
    }
    let days = (end - start).num_days();
    Ok(
        json!({"start":start.to_string(),"endExclusive":end.to_string(),"currency":c,"days":days,
        "forecastMinor":forecast,"cashFlowMinor":cash_flow,"accommodationMinor":accommodation,
        "occupiedNights":nights,"dailyForecastMinor":forecast as f64/days as f64,"months":months}),
    )
}

/// Explicit rational conversion for forecasts/quotes. Returns half-away-from-zero
/// rounding; caller retains the original amount, rate, source and quoted date.
pub fn convert_minor(
    amount_minor: i64,
    from_scale: u32,
    to_scale: u32,
    rate_numerator: i64,
    rate_denominator: i64,
) -> Result<i64, String> {
    if rate_numerator <= 0 || rate_denominator <= 0 || from_scale > 6 || to_scale > 6 {
        return Err("Invalid explicit exchange rate or currency scale".into());
    }
    let numerator = i128::from(amount_minor)
        .checked_mul(i128::from(rate_numerator))
        .and_then(|n| n.checked_mul(10_i128.pow(to_scale)))
        .ok_or("Conversion overflow")?;
    let denominator = i128::from(rate_denominator)
        .checked_mul(10_i128.pow(from_scale))
        .ok_or("Conversion overflow")?;
    let rounded = (numerator.abs() + denominator / 2) / denominator * numerator.signum();
    i64::try_from(rounded)
        .ok()
        .filter(|n| n.unsigned_abs() <= MAX_MONEY as u64)
        .ok_or_else(|| "Converted amount exceeds exact range".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stay() -> Value {
        json!({"id":"s","kind":"stay","name":"Synthetic apartment","currency":"USD","status":"confirmed","checkIn":"2026-12-12","checkOut":"2027-01-11","totalMinor":90000})
    }
    fn get(records: &[Value], month: &str) -> Value {
        summary(
            records,
            &json!({}),
            &json!({"month":month,"today":"2026-12-01"}),
        )
        .unwrap()
    }

    #[test]
    fn prepaid_cross_month_costs_are_separate_from_cash() {
        let records = vec![
            stay(),
            json!({"id":"p","kind":"payment","bookingId":"s","date":"2026-11-20","amountMinor":90000,"currency":"USD","type":"payment","status":"settled"}),
        ];
        let dec = get(&records, "2026-12");
        let jan = get(&records, "2027-01");
        let nov = get(&records, "2026-11");
        assert_eq!(dec["accommodationMinor"], 60000);
        assert_eq!(dec["occupiedNights"], 20);
        assert_eq!(jan["accommodationMinor"], 30000);
        assert_eq!(jan["occupiedNights"], 10);
        assert_eq!(dec["cashFlowMinor"], 0);
        assert_eq!(nov["cashFlowMinor"], 90000);
        assert_eq!(nov["accommodationMinor"], 0);
        assert_eq!(dec["paymentStatuses"][0]["status"], "paid");
    }

    #[test]
    fn exact_allocation_preserves_remainder() {
        assert_eq!(allocate(100, 3, 0, 1).unwrap(), 34);
        assert_eq!(allocate(100, 3, 1, 3).unwrap(), 66);
        for total in 0..1000 {
            assert_eq!(
                (0..31)
                    .map(|n| allocate(total, 31, n, n + 1).unwrap())
                    .sum::<i64>(),
                total
            );
        }
    }

    #[test]
    fn leap_year_and_free_stays() {
        let records = vec![
            json!({"id":"s","kind":"stay","currency":"USD","status":"confirmed","checkIn":"2028-02-01","checkOut":"2028-03-01","totalMinor":0}),
        ];
        let r = get(&records, "2028-02");
        assert_eq!(r["daysInMonth"], 29);
        assert_eq!(r["occupiedNights"], 29);
        assert_eq!(r["accommodationMinor"], 0);
    }

    #[test]
    fn alternatives_and_cancelled_bookings_do_not_commit_costs() {
        let mut alternative = stay();
        alternative["status"] = json!("tentative");
        let mut cancelled = stay();
        cancelled["status"] = json!("cancelled");
        assert_eq!(
            get(&[alternative, cancelled], "2026-12")["forecastMinor"],
            0
        );
    }

    #[test]
    fn deposits_affect_cash_but_not_consumption_or_booking_paid_status() {
        let records = vec![
            stay(),
            json!({"kind":"payment","bookingId":"s","date":"2026-12-01","amountMinor":20000,"currency":"USD","type":"deposit","status":"settled"}),
            json!({"kind":"payment","bookingId":"s","date":"2027-01-20","amountMinor":-20000,"currency":"USD","type":"deposit_refund","status":"settled"}),
        ];
        let r = get(&records, "2026-12");
        assert_eq!(r["cashFlowMinor"], 20000);
        assert_eq!(r["forecastMinor"], 60000);
        assert_eq!(r["paymentStatuses"][0]["status"], "unpaid");
        assert_eq!(get(&records, "2027-01")["cashFlowMinor"], -20000);
    }

    #[test]
    fn actual_objective_expense_consumes_allowance_once() {
        let records = vec![
            json!({"kind":"budget","currency":"USD","month":"2026-12","incomeMinor":200000,"reserveMinor":10000,"allowanceMinor":30000}),
            json!({"id":"o","kind":"objective","date":"2026-12-02","currency":"USD","totalMinor":5000}),
            json!({"kind":"expense","date":"2026-12-02","currency":"USD","amountMinor":4000,"objectiveId":"o"}),
        ];
        let r = get(&records, "2026-12");
        assert_eq!(r["objectivesMinor"], 0);
        assert_eq!(r["expensesMinor"], 4000);
        assert_eq!(r["allowanceRemainingMinor"], 26000);
        assert_eq!(r["forecastMinor"], 30000);
        assert_eq!(r["availableMinor"], 160000);
    }

    #[test]
    fn recurring_objective_receipt_replaces_only_the_matching_occurrence() {
        let records = vec![
            json!({"id":"series","kind":"objective","date":"2026-12-01","currency":"USD","totalMinor":1000,"recurrence":"daily","recurrenceUntil":"2026-12-03"}),
            json!({"kind":"expense","date":"2026-12-02","currency":"USD","amountMinor":800,"objectiveId":"series"}),
        ];
        let r = get(&records, "2026-12");
        assert_eq!(r["objectivesMinor"], 2000);
        assert_eq!(r["expensesMinor"], 800);
        assert_eq!(r["forecastMinor"], 2800);
    }

    #[test]
    fn recurring_prepaid_occurrence_does_not_suppress_other_cash_obligations() {
        let records = vec![
            json!({"id":"series","kind":"objective","date":"2026-12-01","currency":"USD","totalMinor":1000,"recurrence":"weekly","recurrenceUntil":"2026-12-31"}),
            json!({"kind":"payment","bookingId":"series","date":"2026-11-20","occurrenceDate":"2026-12-08","currency":"USD","amountMinor":1000,"type":"payment","status":"settled"}),
        ];
        let r = get(&records, "2026-12");
        assert_eq!(r["objectivesMinor"], 5000);
        assert_eq!(r["cashFlowMinor"], 4000);
        assert_eq!(get(&records, "2026-11")["cashFlowMinor"], 1000);
    }

    #[test]
    fn planned_objective_consumes_allowance_without_double_count() {
        let records = vec![
            json!({"kind":"budget","currency":"USD","month":"2026-12","allowanceMinor":30000}),
            json!({"kind":"objective","date":"2026-12-02","currency":"USD","totalMinor":5000}),
        ];
        let r = get(&records, "2026-12");
        assert_eq!(r["forecastMinor"], 30000);
        assert_eq!(r["allowanceRemainingMinor"], 25000);
    }

    #[test]
    fn objective_payment_schedule_replaces_expected_cash_date_without_double_count() {
        let records = vec![
            json!({"id":"o","kind":"objective","date":"2026-12-02","currency":"USD","totalMinor":5000}),
            json!({"kind":"payment","bookingId":"o","date":"2026-11-20","currency":"USD","amountMinor":5000,"type":"payment","status":"settled"}),
        ];
        assert_eq!(get(&records, "2026-12")["objectivesMinor"], 5000);
        assert_eq!(get(&records, "2026-12")["cashFlowMinor"], 0);
        assert_eq!(get(&records, "2026-11")["cashFlowMinor"], 5000);
    }

    #[test]
    fn original_currencies_never_mix_implicitly() {
        let mut foreign = stay();
        foreign["currency"] = json!("EUR");
        let r = get(&[stay(), foreign], "2026-12");
        assert_eq!(r["accommodationMinor"], 60000);
        assert_eq!(r["excludedCurrencies"], json!(["EUR"]));
    }

    #[test]
    fn historic_cross_currency_settlement_credits_booking_without_rewriting_cash() {
        let records = vec![
            stay(),
            json!({
                "id":"foreign","kind":"payment","bookingId":"s","date":"2026-12-01",
                "currency":"EUR","amountMinor":83000,"bookingCurrency":"USD","bookingAmountMinor":90000,
                "rateDate":"2026-12-01","source":"Synthetic bank settlement","type":"payment","status":"settled"
            }),
        ];
        let usd = get(&records, "2026-12");
        assert_eq!(usd["paymentStatuses"][0]["status"], "paid");
        assert_eq!(usd["paymentStatuses"][0]["settledMinor"], 90000);
        assert_eq!(
            usd["paymentStatuses"][0]["settlementBasisIncomplete"],
            false
        );
        assert_eq!(usd["cashFlowMinor"], 0);
        assert_eq!(usd["accommodationMinor"], 60000);
        let eur = summary(
            &records,
            &json!({}),
            &json!({"month":"2026-12","currency":"EUR"}),
        )
        .unwrap();
        assert_eq!(eur["cashFlowMinor"], 83000);
        assert_eq!(eur["settledCashFlowMinor"], 83000);
        let changed_rate = summary(
            &records,
            &json!({"exchangeRates":{"EUR/USD":2}}),
            &json!({"month":"2026-12"}),
        )
        .unwrap();
        assert_eq!(changed_rate["paymentStatuses"], usd["paymentStatuses"]);
    }

    #[test]
    fn foreign_refund_uses_recorded_equivalent_and_unconverted_payment_is_flagged() {
        let records = vec![
            stay(),
            json!({"kind":"payment","bookingId":"s","date":"2026-11-20","currency":"USD","amountMinor":90000,"type":"payment","status":"settled"}),
            json!({"kind":"payment","bookingId":"s","date":"2026-12-20","currency":"EUR","amountMinor":-8300,"bookingCurrency":"USD","bookingAmountMinor":-9000,"type":"refund","status":"settled"}),
        ];
        let r = get(&records, "2026-12");
        assert_eq!(r["paymentStatuses"][0]["settledMinor"], 81000);
        assert_eq!(r["paymentStatuses"][0]["status"], "part-paid");
        assert_eq!(r["accommodationMinor"], 60000);
        let missing = vec![
            stay(),
            json!({"kind":"payment","bookingId":"s","date":"2026-12-01","currency":"EUR","amountMinor":83000,"type":"payment","status":"settled"}),
        ];
        let r = get(&missing, "2026-12");
        assert_eq!(r["paymentStatuses"][0]["settledMinor"], 0);
        assert_eq!(r["paymentStatuses"][0]["settlementBasisIncomplete"], true);
        assert!(r["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("payment status is incomplete")));
    }

    #[test]
    fn signed_refund_does_not_rewrite_use_costs() {
        let r = get(
            &[
                stay(),
                json!({"kind":"payment","bookingId":"s","date":"2026-12-15","currency":"USD","amountMinor":-9000,"type":"refund","status":"settled"}),
            ],
            "2026-12",
        );
        assert_eq!(r["cashFlowMinor"], -9000);
        assert_eq!(r["accommodationMinor"], 60000);
    }

    #[test]
    fn period_has_explicit_dates_and_calendar_day_denominator() {
        let r = period_summary(
            &[stay()],
            &json!({}),
            &json!({"start":"2026-12-12","end":"2027-01-11"}),
        )
        .unwrap();
        assert_eq!(r["days"], 30);
        assert_eq!(r["forecastMinor"], 90000);
        assert_eq!(r["dailyForecastMinor"], 3000.0);
    }

    #[test]
    fn no_remaining_days_is_null_not_infinite() {
        let r = summary(
            &[],
            &json!({}),
            &json!({"month":"2026-02","today":"2026-03-01"}),
        )
        .unwrap();
        assert_eq!(r["remainingDays"], 0);
        assert!(r["dailyAllowanceMinor"].is_null());
    }

    #[test]
    fn rejects_float_amounts_bad_dates_and_unsafe_totals() {
        let mut s = stay();
        s["totalMinor"] = json!(1.1);
        assert!(summary(&[s], &json!({}), &json!({"month":"2026-12"})).is_err());
        assert!(summary(&[], &json!({}), &json!({"month":"2026-13"})).is_err());
        assert!(allocate(1, 0, 0, 0).is_err());
    }

    #[test]
    fn manual_rational_conversion_handles_currency_scales_and_negative_ties() {
        assert_eq!(convert_minor(10000, 2, 0, 150, 1).unwrap(), 15000);
        assert_eq!(convert_minor(-1, 2, 2, 1, 2).unwrap(), -1);
        assert_eq!(convert_minor(100, 0, 2, 1, 150).unwrap(), 67);
    }
}
