//! Review, readiness and offer scenarios share saved evidence and exact accounting.
use crate::{
    error, journey, merge_known, money, now, parse_date, planning, schedule, string as text, Atlas,
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde_json::{json, Value};

impl Atlas {
    pub(crate) fn automation(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "imports.preview" => self.import_preview(p),
            "refunds.receive" => self.receive_refund(p),
            "readiness.summary" => readiness(&self.records(None)?, &self.settings()?, p),
            "reminders.suggestions" => Ok(json!(suggestions(&self.records(None)?, p)?)),
            "reminders.adopt" => {
                let list = suggestions(&self.records(None)?, p)?;
                let suggestion = list
                    .into_iter()
                    .find(|r| r["suggestionKey"] == p["suggestionKey"])
                    .ok_or("This suggestion changed. Refresh before adopting it.")?;
                if let Some(old) = self
                    .records(Some("reminder"))?
                    .into_iter()
                    .find(|r| r["suggestionKey"] == suggestion["suggestionKey"])
                {
                    return Ok(old);
                }
                self.save_record(suggestion, None)
            }
            "discovery.compare" => compare(&self.records(None)?, &self.settings()?, p),
            "discovery.scenario" => scenario(&self.records(None)?, &self.settings()?, p),
            "watchlist.snapshot" => {
                let mut record = self.get_required(text(p, "id"))?;
                if !["watchlist", "quote", "place"].contains(&text(&record, "kind")) {
                    return Err("Choose a saved offer or watchlist entry.".into());
                }
                if record["version"] != p["expectedVersion"] {
                    return Err("Conflict: refresh this saved item.".into());
                }
                let patch = p["snapshot"]
                    .as_object()
                    .ok_or("A manual snapshot is required")?;
                let mut snapshot = json!({"checkedAt":now(),"method":"manual"});
                for key in [
                    "priceMinor",
                    "currency",
                    "availability",
                    "evidence",
                    "sourceUrl",
                ] {
                    if let Some(value) = patch.get(key) {
                        snapshot[key] = value.clone();
                    }
                }
                if snapshot["priceMinor"].as_i64().is_some_and(|n| n < 0)
                    || (!snapshot["priceMinor"].is_null()
                        && snapshot["priceMinor"].as_i64().is_none())
                {
                    return Err("Price must use nonnegative integer minor units.".into());
                }
                if let Some(amount) = snapshot["priceMinor"].as_i64() {
                    crate::require_currency_amount(
                        &json!({"currency":snapshot["currency"],"amountMinor":amount}),
                        "amountMinor",
                    )?;
                }
                let mut checked = json!({"kind":"watchlist","name":"Manual snapshot"});
                for key in ["currency", "sourceUrl"] {
                    if !snapshot[key].is_null() {
                        checked[key] = snapshot[key].clone();
                    }
                }
                self.validate_record(&mut checked)?;
                let mut history = record["snapshots"].as_array().cloned().unwrap_or_default();
                history.push(snapshot.clone());
                if history.len() > 100 {
                    history.remove(0);
                }
                record["snapshots"] = json!(history);
                record["lastSnapshot"] = snapshot;
                record["lastCheckedAt"] = json!(now());
                self.save_record(record, p["expectedVersion"].as_i64())
            }
            "watchlist.remind" => {
                let record = self.get_required(text(p, "id"))?;
                if !["watchlist", "quote", "place"].contains(&text(&record, "kind")) {
                    return Err("Choose a watchlist entry or offer.".into());
                }
                let key = format!("watch:{}:{}", text(&record, "id"), text(p, "dueAt"));
                if let Some(old) = self
                    .records(Some("reminder"))?
                    .into_iter()
                    .find(|r| text(r, "suggestionKey") == key)
                {
                    return Ok(old);
                }
                self.save_record(json!({"kind":"reminder","name":format!("Recheck {}",text(&record,"name")),"bookingId":record["id"],"dueAt":p["dueAt"],"suggestionKey":key,"state":"pending","catchUp":"deliver"}),None)
            }
            _ => Err("Unknown automation service".into()),
        }
    }
    fn import_preview(&self, p: &Value) -> Result<Value, String> {
        let draft = self.get_required(text(p, "id"))?;
        if text(&draft, "kind") != "draft" {
            return Err("Choose an import draft.".into());
        }
        let before = self.get(text(&draft, "existingRecordId"))?;
        let mut candidate = before.clone().unwrap_or_else(|| json!({}));
        let mut patch = draft["record"].clone();
        if let Some(fields) = p["fields"].as_array() {
            if before.is_some() {
                patch
                    .as_object_mut()
                    .ok_or("Draft fields missing")?
                    .retain(|key, _| key == "kind" || fields.contains(&json!(key)));
            }
        }
        merge_known(&mut candidate, &patch);
        let mut changes = vec![];
        for (field, after) in draft["record"]
            .as_object()
            .ok_or("Draft fields are missing")?
        {
            if ["id", "version", "kind", "sourceIds"].contains(&field.as_str()) || after.is_null() {
                continue;
            }
            let old = before
                .as_ref()
                .and_then(|b| b.get(field))
                .cloned()
                .unwrap_or(Value::Null);
            if old != *after {
                changes.push(json!({"field":field,"before":old,"after":after,"evidence":draft["evidence"][field],"state":draft["fieldStates"][field]}));
            }
        }
        let all = self.records(None)?;
        let settings = self.settings()?;
        let mut hypothetical: Vec<_> = all
            .iter()
            .filter(|r| before.as_ref().is_none_or(|b| r["id"] != b["id"]))
            .cloned()
            .collect();
        hypothetical.push(candidate.clone());
        let mut effects = if text(&candidate, "status") == "cancelled" {
            json!({"warnings":[{"message":"This booking will leave itinerary forecasts. Original payments and audit history remain."}]})
        } else {
            journey::preview(&hypothetical, &settings, &json!({"record":candidate}))
                .unwrap_or_else(|e| json!({"needsReview":e}))
        };
        let variants: Vec<_> = before.iter().chain(std::iter::once(&candidate)).collect();
        let starts: Vec<_> = variants
            .iter()
            .filter_map(|r| {
                if text(r, "kind") == "stay" {
                    r["checkIn"].as_str()
                } else {
                    text(r, "departure").get(..10)
                }
            })
            .collect();
        let ends: Vec<_> = variants
            .iter()
            .filter_map(|r| {
                if text(r, "kind") == "stay" {
                    r["checkOut"].as_str().map(str::to_owned)
                } else {
                    text(r, "arrival")
                        .get(..10)
                        .and_then(|d| parse_date(d).ok())
                        .map(|d| (d + Duration::days(1)).to_string())
                }
            })
            .collect();
        let mut gaps = Value::Null;
        if let (Some(day), Some(end)) = (starts.iter().min(), ends.iter().max()) {
            let request = json!({"start":day,"end":end});
            gaps = journey::gaps(&hypothetical, &request).unwrap_or(Value::Null);
            let mut currencies: Vec<_> = variants
                .iter()
                .filter_map(|r| r["currency"].as_str())
                .collect();
            currencies.sort();
            currencies.dedup();
            let mut budgets = vec![];
            for currency in currencies {
                let request = json!({"start":day,"end":end,"currency":currency});
                if let (Ok(before), Ok(after)) = (
                    money::period_summary(&all, &settings, &request),
                    money::period_summary(&hypothetical, &settings, &request),
                ) {
                    budgets.push(json!({"currency":currency,"before":before,"after":after}));
                }
            }
            effects["budgetChanges"] = json!(budgets);
        }
        effects["linkedRecords"] = json!(all
            .iter()
            .filter(|r| before.as_ref().is_some_and(|old| [
                "stayId",
                "originStayId",
                "destinationStayId",
                "bookingId"
            ]
            .iter()
            .any(|key| r[*key] == old["id"])))
            .map(|r| json!({"id":r["id"],"name":r["name"],"kind":r["kind"]}))
            .collect::<Vec<_>>());
        Ok(
            json!({"draft":draft,"before":before,"candidate":candidate,"changes":changes,"effects":effects,"gaps":gaps,"history":before.as_ref().map(|b|self.history(text(b,"id"))).transpose()?,"notice":"Only selected known fields replace saved facts. Review timing, work, gaps, objectives, money and linked reminders before approval."}),
        )
    }
    pub(crate) fn save_import_refund(
        &mut self,
        draft: &mut Value,
        booking: &Value,
    ) -> Result<(), String> {
        if !draft["refund"].is_object() {
            return Ok(());
        }
        let notice = &draft["refund"];
        let pending: Vec<_> = self
            .records(Some("refund"))?
            .into_iter()
            .filter(|r| {
                r["bookingId"] == booking["id"]
                    && text(r, "state") != "received"
                    && r["amountMinor"] == notice["amountMinor"]
                    && r["currency"] == notice["currency"]
            })
            .collect();
        let mut refund = if pending.len() == 1 {
            pending[0].clone()
        } else {
            json!({"kind":"refund","bookingId":booking["id"],"sourceIds":[]})
        };
        refund["reportedState"] = notice["state"].clone();
        refund["state"] = if notice["state"] == "received" {
            json!("expected")
        } else {
            notice["state"].clone()
        };
        for key in ["amountMinor", "currency", "date"] {
            if !notice[key].is_null() {
                refund[key] = notice[key].clone();
            }
        }
        let mut sources = refund["sourceIds"].as_array().cloned().unwrap_or_default();
        if !sources.contains(&draft["sourceId"]) {
            sources.push(draft["sourceId"].clone());
        }
        refund["sourceIds"] = json!(sources);
        refund["name"] = json!(format!("Refund · {}", text(booking, "name")));
        refund["outstandingMinor"] = refund["amountMinor"]
            .as_i64()
            .map(|n| json!((n - refund["receivedMinor"].as_i64().unwrap_or(0)).max(0)))
            .unwrap_or(Value::Null);
        let saved = self.save_record(refund, None)?;
        draft["refundId"] = saved["id"].clone();
        Ok(())
    }
    fn receive_refund(&mut self, p: &Value) -> Result<Value, String> {
        let mut refund = self.get_required(text(p, "id"))?;
        if text(&refund, "kind") != "refund" {
            return Err("Choose a refund notice.".into());
        }
        let request_id = text(p, "requestId");
        uuid::Uuid::parse_str(request_id).map_err(|_| "A stable receipt request ID is required")?;
        if let Some(receipt) = refund["receipts"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| text(r, "requestId") == request_id)
        {
            return self.get_required(text(receipt, "paymentId"));
        }
        if refund["version"] != p["expectedVersion"] {
            return Err("Conflict: refund notice changed. Review it again.".into());
        }
        if p["confirmReceived"] != true {
            return Err("Confirm that the refund actually reached your account.".into());
        }
        let original = self.get_required(text(p, "paymentId"))?;
        if text(&original, "kind") != "payment"
            || original["bookingId"] != refund["bookingId"]
            || text(&original, "status") != "settled"
            || !["payment", "deposit"].contains(&text(&original, "type"))
        {
            return Err("Link the original settled booking payment or deposit.".into());
        }
        let amount = p["amountMinor"]
            .as_i64()
            .filter(|n| *n > 0)
            .ok_or("Enter the positive amount actually received.")?;
        if p["currency"] != original["currency"] {
            return Err("Record the receipt in the original payment currency; cross-currency reconciliation needs the payment editor.".into());
        }
        parse_date(text(p, "date"))?;
        let returned: i64 = self
            .records(Some("payment"))?
            .iter()
            .filter(|r| r["paymentId"] == original["id"] && text(r, "status") == "settled")
            .map(|r| -r["amountMinor"].as_i64().unwrap_or(0))
            .sum();
        if returned
            .checked_add(amount)
            .is_none_or(|n| n > original["amountMinor"].as_i64().unwrap_or(0))
        {
            return Err("Refund receipts exceed the original payment. Review the amounts.".into());
        }
        self.db
            .execute_batch("SAVEPOINT refund_receipt")
            .map_err(error)?;
        let result = (|| {
            let payment=self.save_record(json!({"kind":"payment","name":refund["name"],"bookingId":refund["bookingId"],"paymentId":original["id"],"date":p["date"],"amountMinor":-amount,"currency":p["currency"],"type":if text(&original,"type")=="deposit"{"deposit_refund"}else{"refund"},"status":"settled","sourceIds":refund["sourceIds"]}),None)?;
            let total = refund["receivedMinor"]
                .as_i64()
                .unwrap_or(0)
                .checked_add(amount)
                .ok_or("Refund total exceeds supported range")?;
            if refund["receivedCurrency"]
                .as_str()
                .is_some_and(|c| c != text(p, "currency"))
            {
                return Err(
                    "Refund receipts in different currencies must be reviewed separately.".into(),
                );
            }
            refund["state"] = json!(if refund["amountMinor"]
                .as_i64()
                .is_none_or(|expected| expected <= total)
                && (refund["currency"].is_null() || refund["currency"] == p["currency"])
            {
                "received"
            } else {
                "expected"
            });
            refund["receivedMinor"] = json!(total);
            refund["outstandingMinor"] = if refund["currency"] == p["currency"] {
                refund["amountMinor"]
                    .as_i64()
                    .map(|n| json!((n - total).max(0)))
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            let mut receipts = refund["receipts"].as_array().cloned().unwrap_or_default();
            receipts.push(json!({"requestId":request_id,"paymentId":payment["id"],"amountMinor":amount,"date":p["date"]}));
            refund["receipts"] = json!(receipts);
            refund["receivedCurrency"] = p["currency"].clone();
            refund["receivedDate"] = p["date"].clone();
            refund["paymentId"] = original["id"].clone();
            refund["settledPaymentId"] = payment["id"].clone();
            self.save_record(refund, None)?;
            Ok(payment)
        })();
        match result {
            Ok(v) => {
                self.db
                    .execute_batch("RELEASE refund_receipt")
                    .map_err(error)?;
                Ok(v)
            }
            Err(e) => {
                let _ = self
                    .db
                    .execute_batch("ROLLBACK TO refund_receipt; RELEASE refund_receipt");
                Err(e)
            }
        }
    }
    pub(crate) fn import_notice(&self, draft: &Value, booking: &Value) -> Result<(), String> {
        let timestamp = now();
        let id = format!("accepted:{}", text(draft, "id"));
        let row = json!({"id":id,"type":"booking_import","title":format!("Booking {}: {}",text(draft,"action"),text(booking,"name")),"description":"Reviewed import saved. Original payments and source evidence are retained.","priority":"normal","createdAt":timestamp,"target":{"module":"Calendar","recordId":booking["id"]}});
        self.db
            .execute(
                "INSERT OR IGNORE INTO notifications(id,body,created_at) VALUES(?,?,?)",
                params![id, row.to_string(), timestamp],
            )
            .map_err(error)?;
        Ok(())
    }
    pub(crate) fn recalculate_booking_reminders(&mut self, booking: &Value) -> Result<(), String> {
        let records = self.records(None)?;
        let proposed = suggestions(&[booking.clone()], &json!({}))?;
        let auto = self.settings()?["bookingReminderRules"].clone();
        for mut reminder in records
            .iter()
            .filter(|r| {
                text(r, "kind") == "reminder"
                    && r["bookingId"] == booking["id"]
                    && !text(r, "rule").is_empty()
            })
            .cloned()
        {
            if !["pending", "snoozed"].contains(&text(&reminder, "state")) {
                continue;
            }
            let replacement = proposed
                .iter()
                .find(|r| r["suggestionKey"] == reminder["suggestionKey"]);
            if text(booking, "status") == "cancelled" {
                reminder["state"] = json!("dismissed");
                reminder["deliveryNote"] = json!("Linked booking was cancelled.");
            } else if let Some(next) = replacement {
                if text(&reminder, "state") == "pending" {
                    reminder["dueAt"] = next["dueAt"].clone();
                }
                reminder["baseDueAt"] = next["dueAt"].clone();
            } else {
                reminder["state"] = json!("dismissed");
                reminder["deliveryNote"] =
                    json!("Booking revision removed or passed this reminder time.");
            }
            if records
                .iter()
                .find(|r| r["id"] == reminder["id"])
                .is_some_and(|old| *old != reminder)
            {
                self.save_record(reminder, None)?;
            }
        }
        for suggestion in proposed {
            if auto[text(&suggestion, "rule")] == true
                && !records
                    .iter()
                    .any(|r| r["suggestionKey"] == suggestion["suggestionKey"])
            {
                self.save_record(suggestion, None)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn suggestions(records: &[Value], p: &Value) -> Result<Vec<Value>, String> {
    let at = p["now"]
        .as_str()
        .map(DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(error)?
        .map(|v| v.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let mut out = vec![];
    for r in records.iter().filter(|r| {
        ["stay", "transport"].contains(&text(r, "kind")) && text(r, "status") != "cancelled"
    }) {
        let mut dates: Vec<(&str, &str, DateTime<Utc>, i64)> = vec![];
        if text(r, "kind") == "stay" {
            for (rule, label, day, time, lead) in [
                (
                    "checkin",
                    "Check in tomorrow",
                    "checkIn",
                    "checkInTime",
                    1440,
                ),
                (
                    "checkout",
                    "Checkout approaching",
                    "checkOut",
                    "checkOutTime",
                    1440,
                ),
            ] {
                if let Ok(instant) =
                    schedule::resolve_local(text(r, day), text(r, time), text(r, "timezone"))
                {
                    dates.push((rule, label, instant, lead));
                }
            }
        }
        for (rule, label, field, lead) in [
            ("departure", "Departure approaching", "departure", 120),
            (
                "cancellation",
                "Cancellation deadline",
                "cancellationAt",
                1440,
            ),
            ("payment", "Payment due", "paymentDueAt", 1440),
        ] {
            if let Ok(instant) = DateTime::parse_from_rfc3339(text(r, field)) {
                dates.push((rule, label, instant.with_timezone(&Utc), lead));
            }
        }
        for (rule, label, instant, lead) in dates {
            let due = instant - Duration::minutes(lead);
            if due <= at {
                continue;
            }
            let key = format!("booking:{}:{rule}", text(r, "id"));
            let existing = records.iter().find(|other| other["suggestionKey"] == key);
            out.push(json!({"kind":"reminder","name":format!("{label} · {}",text(r,"name")),"bookingId":r["id"],"rule":rule,"suggestionKey":key,"dueAt":due.to_rfc3339(),"baseDueAt":due.to_rfc3339(),"eventAt":instant.to_rfc3339(),"state":"pending","catchUp":"deliver","adopted":existing.is_some(),"existingState":existing.map(|v|&v["state"])}));
        }
    }
    out.sort_by(|a, b| text(a, "dueAt").cmp(text(b, "dueAt")));
    Ok(out)
}

pub(crate) fn readiness(records: &[Value], settings: &Value, p: &Value) -> Result<Value, String> {
    let day = p["start"]
        .as_str()
        .map(parse_date)
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());
    let end = p["end"]
        .as_str()
        .map(parse_date)
        .transpose()?
        .unwrap_or(day + Duration::days(30));
    if end <= day || (end - day).num_days() > 90 {
        return Err("Readiness covers between 1 and 90 days.".into());
    }
    let start = day.to_string();
    let stop = end.to_string();
    let mut items = vec![];
    let mut add = |group: &str, kind: &str, title: String, detail: String, target: Value| {
        items
            .push(json!({"group":group,"kind":kind,"title":title,"detail":detail,"target":target}));
    };
    let bookings: Vec<_> = records
        .iter()
        .filter(|r| {
            text(r, "status") != "cancelled"
                && match text(r, "kind") {
                    "stay" => {
                        text(r, "checkOut") > start.as_str() && text(r, "checkIn") < stop.as_str()
                    }
                    "transport" => text(r, "departure")
                        .get(..10)
                        .is_some_and(|d| d >= start.as_str() && d < stop.as_str()),
                    _ => false,
                }
        })
        .collect();
    for r in &bookings {
        let target = json!({"module":"Calendar","recordId":r["id"]});
        add(
            if text(r, "status") == "confirmed" {
                "complete"
            } else {
                "attention"
            },
            "confirmation",
            text(r, "name").into(),
            format!("Booking {}", text(r, "status")),
            target.clone(),
        );
        if text(r, "kind") == "stay" {
            let workspace = journey::workspace(records, r);
            if text(&workspace, "status") != "suitable" {
                add(
                    if text(&workspace, "status") == "unsuitable" {
                        "attention"
                    } else {
                        "unknown"
                    },
                    "workspace",
                    format!("Workspace · {}", text(r, "name")),
                    format!(
                        "Verify: {}",
                        workspace["unknown"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    target.clone(),
                );
            }
        }
        let docs: Vec<_> = records
            .iter()
            .filter(|d| {
                text(d, "kind") == "document"
                    && (d["bookingId"] == r["id"]
                        || r["documentIds"]
                            .as_array()
                            .is_some_and(|ids| ids.contains(&d["id"])))
            })
            .collect();
        if docs.is_empty() {
            add(
                "unknown",
                "document",
                format!("Documents · {}", text(r, "name")),
                "No linked travel document is saved. Verify what this booking requires.".into(),
                target.clone(),
            );
        } else {
            for doc in docs {
                add(
                    "complete",
                    "document",
                    text(doc, "filename").into(),
                    "Linked document saved locally.".into(),
                    json!({"module":"Calendar","recordId":doc["id"]}),
                );
            }
        }
        for (key, label) in [
            ("cancellationAt", "Cancellation deadline"),
            ("paymentDueAt", "Payment due"),
        ] {
            if let Ok(t) = DateTime::parse_from_rfc3339(text(r, key)) {
                if t.date_naive() >= day && t.date_naive() < end {
                    add(
                        "upcoming",
                        key,
                        format!("{label} · {}", text(r, "name")),
                        t.to_rfc3339(),
                        if key == "paymentDueAt" {
                            json!({"module":"Budget","recordId":r["id"]})
                        } else {
                            target.clone()
                        },
                    );
                }
            }
        }
    }
    if bookings.is_empty() {
        add(
            "unknown",
            "itinerary",
            "No upcoming booking in this period".into(),
            "Select travel dates or add your next stay.".into(),
            json!({"module":"Calendar","date":start}),
        );
    }
    // Only examine nights within the recorded journey; a month selection alone is not a trip.
    let starts: Vec<_> = bookings
        .iter()
        .filter_map(|r| {
            if text(r, "kind") == "stay" {
                Some(text(r, "checkIn"))
            } else {
                text(r, "departure").get(..10)
            }
        })
        .collect();
    let ends: Vec<_> = bookings
        .iter()
        .filter_map(|r| {
            if text(r, "kind") == "stay" {
                Some(text(r, "checkOut").to_owned())
            } else {
                text(r, "arrival").get(..10).map(str::to_owned)
            }
        })
        .collect();
    if let (Some(a), Some(b)) = (starts.iter().min(), ends.iter().max()) {
        let a = (*a).max(&start);
        let b = b.as_str().min(&stop);
        if b > a {
            for gap in journey::gaps(records, &json!({"start":a,"end":b}))?["ranges"]
                .as_array()
                .into_iter()
                .flatten()
            {
                add(
                    "attention",
                    "gap",
                    format!("{} accommodation nights need review", gap["nights"]),
                    format!(
                        "{} → {} · {}",
                        text(gap, "start"),
                        text(gap, "end"),
                        text(gap, "status")
                    ),
                    json!({"module":"Calendar","date":gap["start"]}),
                );
            }
        }
    }
    for r in records {
        match text(r, "kind") {
            "draft" if text(r, "status") == "pending" => add(
                "attention",
                "import",
                format!(
                    "Review {}",
                    r["record"]["name"].as_str().unwrap_or("booking import")
                ),
                format!("{} · source evidence awaits approval", text(r, "action")),
                json!({"action":"imports"}),
            ),
            "payment"
                if text(r, "status") == "planned"
                    && text(r, "date") >= start.as_str()
                    && text(r, "date") < stop.as_str() =>
            {
                add(
                    "upcoming",
                    "payment",
                    "Scheduled payment".into(),
                    format!(
                        "{} · {} minor units {}",
                        text(r, "date"),
                        r["amountMinor"],
                        text(r, "currency")
                    ),
                    json!({"module":"Budget","recordId":r["id"]}),
                )
            }
            "refund" if text(r, "state") != "received" => add(
                "attention",
                "refund",
                text(r, "name").into(),
                "Expected refund is not settled cash. Verify the actual receipt.".into(),
                json!({"action":"imports"}),
            ),
            "reminder"
                if ["pending", "snoozed"].contains(&text(r, "state"))
                    && text(r, "dueAt")
                        .get(..10)
                        .is_some_and(|d| d >= start.as_str() && d < stop.as_str()) =>
            {
                add(
                    "upcoming",
                    "reminder",
                    text(r, "name").into(),
                    text(r, "dueAt").into(),
                    json!({"module":"Calendar","recordId":r["id"]}),
                )
            }
            _ => {}
        }
    }
    let mut cursor = day;
    while cursor < end {
        let d = cursor.to_string();
        if bookings.iter().any(|r| {
            (text(r, "kind") == "stay"
                && text(r, "checkIn") <= d.as_str()
                && text(r, "checkOut") > d.as_str())
                || (text(r, "kind") == "transport" && text(r, "departure").starts_with(&d))
        }) {
            if let Ok(work) = schedule::window(
                records,
                settings,
                &json!({"date":d,"dateBasis":"destination"}),
            ) {
                for conflict in work["conflicts"].as_array().into_iter().flatten() {
                    add(
                        "attention",
                        "work",
                        format!("Work conflict · {d}"),
                        conflict["message"]
                            .as_str()
                            .unwrap_or("Review the saved travel and work windows.")
                            .into(),
                        json!({"module":"Calendar","date":d,"action":"work"}),
                    );
                }
            }
        }
        cursor += Duration::days(1);
    }
    let ready = !bookings.is_empty()
        && !items
            .iter()
            .any(|i| ["attention", "unknown"].contains(&text(i, "group")));
    let next_stay = bookings
        .iter()
        .filter(|r| text(r, "kind") == "stay" && text(r, "status") == "confirmed")
        .min_by_key(|r| text(r, "checkIn"));
    let next_transport = bookings
        .iter()
        .filter(|r| text(r, "kind") == "transport")
        .min_by_key(|r| text(r, "departure"));
    Ok(
        json!({"start":start,"end":stop,"ready":ready,"items":items,"nextStay":next_stay,"nextTransport":next_transport,"suggestions":suggestions(records,p)?,"basis":"Current saved records. Unknown requirements prevent a ready status; live provider checks are not implied."}),
    )
}

fn compare(records: &[Value], settings: &Value, p: &Value) -> Result<Value, String> {
    let mut ranked = planning::rank(records, settings, p)?;
    let ids = p["quoteIds"].as_array();
    let mut transport = vec![];
    for offer in records.iter().filter(|r| {
        text(r, "kind") == "quote"
            && text(r, "offerType") == "transport"
            && ids.is_none_or(|ids| ids.contains(&r["id"]))
    }) {
        let mut unknown = vec![];
        let mut known = offer["totalMinor"]
            .as_i64()
            .ok_or("Transport price requires minor units.")?;
        for field in ["baggageMinor", "transferMinor", "overnightMinor"] {
            match offer[field].as_i64() {
                Some(v) if v >= 0 => {
                    known = known
                        .checked_add(v)
                        .ok_or("Journey cost exceeds supported range")?
                }
                _ => unknown.push(field),
            }
        }
        let analysis=journey::preview(records,settings,&json!({"record":{ "kind":"transport", "id":offer["id"],"departure":offer["departure"],"arrival":offer["arrival"],"departureTimezone":offer["departureTimezone"],"arrivalTimezone":offer["arrivalTimezone"],"totalMinor":known,"currency":offer["currency"],"origin":offer["origin"],"destination":offer["destination"]}})).unwrap_or_else(|e|json!({"needsReview":e}));
        transport.push(json!({"record":offer,"knownJourneyMinor":known,"unknownComponents":unknown,"analysis":analysis,"availability":offer["availability"],"source":offer["url"],"lastCheckedAt":offer["lastCheckedAt"]}));
    }
    transport.sort_by(|a, b| {
        text(&a["record"], "currency")
            .cmp(text(&b["record"], "currency"))
            .then(
                a["knownJourneyMinor"]
                    .as_i64()
                    .cmp(&b["knownJourneyMinor"].as_i64()),
            )
    });
    ranked["transport"] = json!(transport);
    Ok(ranked)
}
fn scenario(records: &[Value], settings: &Value, p: &Value) -> Result<Value, String> {
    let offer = records
        .iter()
        .find(|r| r["id"] == p["quoteId"] && text(r, "kind") == "quote")
        .ok_or("Choose a saved offer.")?;
    if let Some(replace) = p["replaceBookingId"].as_str() {
        if !records
            .iter()
            .any(|r| text(r, "id") == replace && ["stay", "transport"].contains(&text(r, "kind")))
        {
            return Err("Select an existing booking to replace in this scenario.".into());
        }
    }
    let mut candidate = offer.clone();
    candidate["kind"] = json!(if text(offer, "offerType") == "transport" {
        "transport"
    } else {
        "stay"
    });
    candidate["status"] = json!("confirmed");
    let start = p["start"].as_str().unwrap_or(text(offer, "checkIn"));
    let end = p["end"].as_str().unwrap_or(text(offer, "checkOut"));
    let request = json!({"start":start,"end":end,"currency":offer["currency"],"today":p["today"].as_str().unwrap_or(start)});
    let ranked = planning::rank(records, settings, &json!({"quoteIds":[offer["id"]]}))?;
    if let Some(known) = ranked["ranked"]
        .as_array()
        .and_then(|v| v.first())
        .and_then(|v| v["knownPayableMinor"].as_i64())
    {
        candidate["totalMinor"] = json!(known);
    }
    let before = money::period_summary(records, settings, &request)?;
    let mut hypothetical: Vec<_> = records
        .iter()
        .filter(|r| p["replaceBookingId"].is_null() || r["id"] != p["replaceBookingId"])
        .cloned()
        .collect();
    hypothetical.push(candidate);
    let after = money::period_summary(&hypothetical, settings, &request)?;
    Ok(
        json!({"scenarioOnly":true,"recordsChanged":false,"before":before,"after":after,"forecastDeltaMinor":after["forecastMinor"].as_i64().unwrap()-before["forecastMinor"].as_i64().unwrap(),"basis":"Saved price and known fees for the quoted dates. Unknown transport, availability, discounts and fees remain unverified."}),
    )
}
