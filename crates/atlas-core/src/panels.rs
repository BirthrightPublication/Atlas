//! Durable, local assistant sessions and notification state.
use crate::{error, hash, id, journey, money, now, planning, schedule, string as text, Atlas};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

pub(crate) const MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS conversations(id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS chat_messages(sequence INTEGER PRIMARY KEY AUTOINCREMENT, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, request_id TEXT NOT NULL, role TEXT NOT NULL, body TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(conversation_id, request_id, role));
CREATE INDEX IF NOT EXISTS chat_activity ON conversations(updated_at);
CREATE INDEX IF NOT EXISTS chat_conversation ON chat_messages(conversation_id,sequence);
CREATE TABLE IF NOT EXISTS notifications(id TEXT PRIMARY KEY, body TEXT NOT NULL, created_at TEXT NOT NULL, read_at TEXT, dismissed_at TEXT);
CREATE INDEX IF NOT EXISTS notification_activity ON notifications(created_at);
";

impl Atlas {
    /// Native updater only: informational state cannot authorize installation.
    /// Deliberately absent from the public RPC and companion method tables.
    pub fn record_update_notice(&mut self, version: &str) -> Result<(), String> {
        let version = semver::Version::parse(version)
            .map_err(|_| "Invalid update version")?
            .to_string();
        let timestamp = now();
        let id = format!("update:{version}");
        let row = json!({"id":id,"type":"update","title":format!("Atlas {version} is available"),"description":"The trusted updater found this version. Open Updates to check current compatibility and installation status.","priority":"normal","createdAt":timestamp,"target":{"action":"updates"}});
        self.db
            .execute(
                "INSERT OR IGNORE INTO notifications(id,body,created_at) VALUES(?,?,?)",
                params![id, row.to_string(), timestamp],
            )
            .map_err(error)?;
        Ok(())
    }
    pub(crate) fn chat(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        let cid = text(p, "id");
        match method {
            "chat.list" => {
                let offset = p["offset"].as_i64().unwrap_or(0).max(0);
                let mut stmt=self.db.prepare("SELECT id,title,created_at,updated_at FROM conversations ORDER BY updated_at DESC LIMIT 50 OFFSET ?").map_err(error)?;
                let rows=stmt.query_map([offset],|r|Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?,"updatedAt":r.get::<_,String>(3)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
                Ok(json!(rows))
            }
            "chat.create" => {
                let cid = id();
                let timestamp = now();
                self.db
                    .execute(
                        "INSERT INTO conversations VALUES(?,?,?,?)",
                        params![cid, "New conversation", timestamp, timestamp],
                    )
                    .map_err(error)?;
                self.chat("chat.get", &json!({"id":cid}))
            }
            "chat.get" => {
                let mut session=self.db.query_row("SELECT id,title,created_at,updated_at FROM conversations WHERE id=?",[cid],|r|Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?,"updatedAt":r.get::<_,String>(3)?}))).optional().map_err(error)?.ok_or("Conversation no longer exists")?;
                let before = p["beforeSequence"].as_i64().unwrap_or(i64::MAX);
                let mut stmt=self.db.prepare("SELECT sequence,body FROM chat_messages WHERE conversation_id=? AND sequence<? ORDER BY sequence DESC LIMIT 100").map_err(error)?;
                let rows = stmt
                    .query_map(params![cid, before], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                    })
                    .map_err(error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(error)?;
                let mut messages = vec![];
                for (sequence, body) in rows.into_iter().rev() {
                    let mut v: Value = serde_json::from_str(&body).map_err(error)?;
                    v["sequence"] = json!(sequence);
                    messages.push(v);
                }
                session["messages"] = json!(messages);
                Ok(session)
            }
            "chat.delete" => {
                self.db
                    .execute("DELETE FROM conversations WHERE id=?", [cid])
                    .map_err(error)?;
                Ok(json!({"deleted":true}))
            }
            "chat.rename" => {
                let title = text(p, "title").trim();
                if title.is_empty() || title.chars().count() > 120 {
                    return Err("Use a title of 1–120 characters".into());
                }
                if self
                    .db
                    .execute(
                        "UPDATE conversations SET title=? WHERE id=?",
                        params![title, cid],
                    )
                    .map_err(error)?
                    != 1
                {
                    return Err("Conversation no longer exists".into());
                }
                self.chat("chat.get", p)
            }
            "chat.send" => {
                let message = text(p, "text").trim();
                let request = text(p, "requestId");
                if message.is_empty()
                    || message.chars().count() > 8000
                    || request.len() > 80
                    || request.is_empty()
                {
                    return Err(
                        "A message (up to 8000 characters) and request ID are required".into(),
                    );
                }
                let session = self.chat("chat.get", p)?;
                let existing:Option<String>=self.db.query_row("SELECT body FROM chat_messages WHERE conversation_id=? AND request_id=? AND role='assistant'",params![cid,request],|r|r.get(0)).optional().map_err(error)?;
                if existing.is_some() {
                    return self.chat("chat.get", p);
                }
                let context = &p["context"];
                let answer = self.contextual_answer(message, context)?;
                let timestamp = now();
                let user = json!({"id":id(),"role":"user","text":message,"createdAt":timestamp,"context":context});
                let response = json!({"id":id(),"role":"assistant","text":answer["message"],"result":answer,"createdAt":timestamp,"context":context});
                let tx = self.db.transaction().map_err(error)?;
                tx.execute("INSERT INTO chat_messages(conversation_id,request_id,role,body,created_at) VALUES(?,?,'user',?,?)",params![cid,request,user.to_string(),timestamp]).map_err(error)?;
                tx.execute("INSERT INTO chat_messages(conversation_id,request_id,role,body,created_at) VALUES(?,?,'assistant',?,?)",params![cid,request,response.to_string(),timestamp]).map_err(error)?;
                let title = if text(&session, "title") == "New conversation" {
                    message.chars().take(72).collect::<String>()
                } else {
                    text(&session, "title").into()
                };
                tx.execute(
                    "UPDATE conversations SET title=?,updated_at=? WHERE id=?",
                    params![title, timestamp, cid],
                )
                .map_err(error)?;
                tx.commit().map_err(error)?;
                self.chat("chat.get", p)
            }
            "chat.undo" => {
                let seq = p["sequence"].as_i64().ok_or("Select a message")?;
                let body:String=self.db.query_row("SELECT body FROM chat_messages WHERE sequence=? AND conversation_id=? AND role='assistant'",params![seq,cid],|r|r.get(0)).map_err(error)?;
                let mut body: Value = serde_json::from_str(&body).map_err(error)?;
                let undo = &body["result"]["undo"];
                if text(undo, "method") != "records.delete" {
                    return Err("This response has no reversible action".into());
                }
                let r = self.get_required(text(&undo["payload"], "id"))?;
                if text(&r, "kind") != "reminder" {
                    return Err("Only the explicitly created reminder can be undone".into());
                }
                self.delete_record(text(&r, "id"), undo["payload"]["expectedVersion"].as_i64())?;
                body["result"]["undo"] = Value::Null;
                body["result"]["records"] = json!([]);
                body["text"] = json!(format!("{}\nReminder undone.", text(&body, "text")));
                self.db
                    .execute(
                        "UPDATE chat_messages SET body=? WHERE sequence=?",
                        params![body.to_string(), seq],
                    )
                    .map_err(error)?;
                self.chat("chat.get", p)
            }
            _ => Err("Unknown conversation operation".into()),
        }
    }

    pub(crate) fn contextual_answer(
        &mut self,
        message: &str,
        context: &Value,
    ) -> Result<Value, String> {
        if context.to_string().len() > 4000 {
            return Err("Context references are too large".into());
        }
        if message.trim().to_lowercase().starts_with("remind me:") {
            return self.assistant(message);
        }
        let q = message.to_lowercase();
        let records = self.records(None)?;
        let settings = self.settings()?;
        let date = context["date"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
        crate::parse_date(&date)?;
        let month = context["month"].as_str().unwrap_or(&date[..7]);
        if q.contains("gap") || q.contains("uncovered") {
            let start = crate::parse_date(&format!("{month}-01"))?;
            let end = (start + Duration::days(32)).format("%Y-%m-01").to_string();
            let analysis = journey::gaps(&records, &json!({"start":start.to_string(),"end":end}))?;
            let ranges = analysis["ranges"].as_array().unwrap();
            let lines = ranges
                .iter()
                .map(|r| {
                    format!(
                        "{} → {}: {} night(s), {}.",
                        text(r, "start"),
                        text(r, "end"),
                        r["nights"],
                        text(r, "status")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(
                json!({"message":format!("Accommodation review for {month}: {} range(s) need attention.\n{lines}\nConfirmed stays and intentional overnight travel cover nights; tentative options remain unconfirmed.",ranges.len()),"analysis":analysis,"targets":ranges.iter().map(|r|json!({"label":format!("Review {}",text(r,"start")),"target":r["target"]})).collect::<Vec<_>>() }),
            );
        }
        if q.contains("check") && (q.contains("out") || q.contains("deadline")) {
            let mut stays: Vec<_> = records
                .iter()
                .filter(|r| {
                    text(r, "kind") == "stay"
                        && text(r, "status") != "cancelled"
                        && text(r, "checkOut") >= date.as_str()
                })
                .cloned()
                .collect();
            stays.sort_by(|a, b| text(a, "checkOut").cmp(text(b, "checkOut")));
            stays.truncate(15);
            return Ok(
                json!({"message":"Upcoming checkout dates from your saved stays. Times and cancellation conditions are only those recorded.","records":stays}),
            );
        }
        if q.contains("store my data") || q.contains("privacy") {
            return Ok(
                json!({"message":"Atlas stores records, assistant conversations and notification state in local SQLite. Files and backups use your OS application-data directory. Core planning works offline; no telemetry or cloud AI transmission is enabled. Protect the local files with OS account security and disk encryption.","targets":[{"label":"Privacy & Data","target":{"module":"Settings","settingsCategory":"Privacy & Data"}}]}),
            );
        }
        if q.contains("import") || q.contains("proton") {
            return Ok(
                json!({"message":"Paid Proton Mail uses Bridge on your own computer. Authorize the read-only connector in Email & Integrations using Bridge-generated credentials and its trusted certificate. Atlas reviews imported booking drafts before acceptance. Manual text, EML, ICS and text-bearing PDF imports remain available offline.","targets":[{"label":"Email & Integrations","target":{"module":"Settings","settingsCategory":"Email & Integrations"}},{"label":"Review imports","target":{"action":"imports"}}]}),
            );
        }
        if q.contains("compare") || (q.contains("options") && q.contains("budget")) {
            return self.assistant(message);
        }
        if q.contains("refrigerator") || q.contains("fridge") {
            let candidates = records
                .iter()
                .filter(|r| {
                    ["stay", "quote"].contains(&text(r, "kind"))
                        && text(r, "status") != "cancelled"
                        && r["facilities"]["refrigerator"] == true
                })
                .map(|r| json!({"record":r,"suitability":journey::workspace(&records,r)}))
                .collect::<Vec<_>>();
            return Ok(
                json!({"message":"These saved accommodations explicitly record a refrigerator. Workspace suitability is assessed separately below; unknown conditions do not establish suitability. No live availability was checked.","workspaces":candidates}),
            );
        }
        if q.contains("work") || q.contains("grocer") {
            let work = schedule::window(
                &records,
                &settings,
                &json!({"date":date,"dateBasis":"destination"}),
            )?;
            let candidates = records
                .iter()
                .filter(|r| ["stay", "place", "quote"].contains(&text(r, "kind")))
                .map(|r| json!({"record":r,"suitability":journey::workspace(&records,r)}))
                .collect::<Vec<_>>();
            return Ok(
                json!({"message":format!("Work review for {date}: {} conflict(s). The schedule below uses your employer timezone and destination-local time. Unknown internet, venue hours and commute stay unverified. Suggested windows are proposals; no protected work or booking was moved.",work["conflicts"].as_array().map_or(0,Vec::len)),"work":work,"workspaces":candidates,"targets":[{"label":"View workday","target":{"module":"Calendar","date":date,"action":"work"}},{"label":"Work preferences","target":{"module":"Settings","settingsCategory":"Work Schedule"}}]}),
            );
        }
        if q.contains("near") || q.contains("saved places") || q.contains("destinations") {
            let places: Vec<_> = records
                .iter()
                .filter(|r| ["place", "objective"].contains(&text(r, "kind")))
                .cloned()
                .collect();
            if q.contains("preferences") || q.contains("scenic") || q.contains("nature") {
                let ranking = planning::rank(&records, &settings, &json!({}))?;
                return Ok(
                    json!({"message":"Saved accommodation options ranked using your current editable interests, scenic and surroundings preferences. Open Discover to review the recorded evidence and unknowns. Nearby distances and live inventory are unverified.","records":ranking["ranked"].as_array().unwrap().iter().map(|r|r["record"].clone()).collect::<Vec<_>>(),"comparison":ranking,"targets":[{"label":"Review saved options","target":{"module":"Discover"}}]}),
                );
            }
            return Ok(
                json!({"message":"These are saved places and objectives. Use their recorded addresses and map links; proximity, live routes and venue hours are unknown unless recorded.","records":places}),
            );
        }
        if q.contains("extend") {
            if let Some(stay) = records
                .iter()
                .find(|r| text(r, "id") == text(context, "recordId") && text(r, "kind") == "stay")
            {
                let scenario = planning::what_if(
                    &records,
                    &settings,
                    &json!({"stayId":stay["id"],"additionalNights":7}),
                )?;
                return Ok(
                    json!({"message":format!("Seven additional nights at {}: {} estimated additional cost. The historical nightly average is an estimate; availability, fees and any new discount require a fresh quote. No records changed.",text(stay,"name"),crate::display_money(scenario["scenario"]["estimatedAdditionalMinor"].as_f64().unwrap_or(0.0),text(stay,"currency"))),"scenario":scenario,"records":[stay]}),
                );
            }
        }
        if (q.contains("cost") || q.contains("allowance") || q.contains("budget"))
            && !q.contains("compare")
            && !q.contains("extend")
        {
            let summary = money::summary(&records, &settings, &json!({"month":month}))?;
            let currency = text(&summary, "currency");
            let amount =
                |key: &str| crate::display_money(summary[key].as_f64().unwrap_or(0.0), currency);
            return Ok(
                json!({"message":format!("For {month}: {} planned per calendar day across {} days; {} per occupied accommodation night. Flexible allowance remaining: {} across {} eligible days. Forecast: {}. Month-of-use costs and payment-date cash flow remain separate; tentative options and other currencies are excluded.",amount("dailyForecastMinor"),summary["daysInMonth"],amount("nightlyAccommodationMinor"),amount("allowanceRemainingMinor"),summary["remainingDays"],amount("forecastMinor")),"summary":summary,"targets":[{"label":"Open budget","target":{"module":"Budget","month":month}}]}),
            );
        }
        self.assistant(message)
    }

    pub(crate) fn notifications(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        match method {
            "notifications.read" | "notifications.dismiss" => {
                let field = if method.ends_with("dismiss") {
                    "dismissed_at"
                } else {
                    "read_at"
                };
                if self
                    .db
                    .execute(
                        &format!("UPDATE notifications SET {field}=? WHERE id=?"),
                        params![now(), text(p, "id")],
                    )
                    .map_err(error)?
                    == 0
                {
                    return Err("Notification no longer exists".into());
                }
                return Ok(json!({"saved":true}));
            }
            "notifications.read_all" => {
                self.db
                    .execute(
                        "UPDATE notifications SET read_at=? WHERE read_at IS NULL",
                        [now()],
                    )
                    .map_err(error)?;
                return Ok(json!({"saved":true}));
            }
            "notifications.list" => {}
            _ => return Err("Unknown notification operation".into()),
        }
        let timestamp = p["now"].as_str().map(str::to_owned).unwrap_or_else(now);
        let instant =
            DateTime::parse_from_rfc3339(&timestamp).map_err(|_| "Invalid notification time")?;
        let today = instant.date_naive();
        let horizon = (today + Duration::days(7)).to_string();
        let start = today.to_string();
        // Preserve the existing reminder scheduler and its catch-up policy.
        self.due_reminders(Some(&timestamp))?;
        let records = self.records(None)?;
        let settings = self.settings()?;
        let mut generated: Vec<Value> = vec![];
        let mut add = |kind: &str,
                       r: &Value,
                       when: &str,
                       title: String,
                       description: String,
                       target: Value,
                       priority: &str| {
            generated.push(json!({"id":hash(format!("{kind}:{}:{when}",text(r,"id")).as_bytes()),"type":kind,"title":title,"description":description,"priority":priority,"target":target}));
        };
        for r in &records {
            if ["cancelled", "dismissed", "accepted"].contains(&text(r, "status")) {
                continue;
            }
            let name = text(r, "name");
            let rid = text(r, "id");
            if text(r, "kind") == "stay" && text(r, "status") == "confirmed" {
                for (field, label) in [
                    ("checkIn", "Check-in"),
                    ("checkOut", "Checkout"),
                    ("cancellationAt", "Cancellation deadline"),
                ] {
                    let when = text(r, field);
                    let day = when.get(..10).unwrap_or("");
                    if day >= start.as_str() && day <= horizon.as_str() {
                        add(
                            field,
                            r,
                            when,
                            format!("{label}: {name}"),
                            when.into(),
                            json!({"module":"Calendar","date":day,"recordId":rid}),
                            "normal",
                        );
                    }
                }
            }
            if text(r, "kind") == "transport" && text(r, "status") == "confirmed" {
                let when = text(r, "departure");
                if let Ok(at) = DateTime::parse_from_rfc3339(when) {
                    if at >= instant && at <= instant + Duration::days(7) {
                        let zone = text(r, "departureTimezone")
                            .parse::<chrono_tz::Tz>()
                            .map_err(|_| "Invalid transport timezone")?;
                        add(
                            "departure",
                            r,
                            when,
                            format!("Departure: {name}"),
                            when.into(),
                            json!({"module":"Calendar","date":at.with_timezone(&zone).date_naive().to_string(),"recordId":rid}),
                            "normal",
                        );
                    }
                }
            }
            if text(r, "kind") == "draft" && text(r, "status") == "pending" {
                add(
                    "import",
                    r,
                    "pending",
                    "Booking import needs review".into(),
                    name.into(),
                    json!({"action":"imports","recordId":rid}),
                    "normal",
                );
            }
            if text(r, "kind") == "payment"
                && text(r, "status") == "planned"
                && text(r, "date") >= start.as_str()
                && text(r, "date") <= horizon.as_str()
            {
                add(
                    "payment",
                    r,
                    text(r, "date"),
                    format!("Payment due: {name}"),
                    text(r, "date").into(),
                    json!({"module":"Budget","month":&text(r,"date")[..7],"recordId":rid}),
                    "normal",
                );
            }
            if text(r, "kind") == "reminder" && text(r, "state") == "delivered" {
                let booking = records
                    .iter()
                    .find(|b| text(b, "id") == text(r, "bookingId"));
                let day = text(r, "dueAt").get(..10).unwrap_or(&start);
                add(
                    "reminder",
                    r,
                    text(r, "dueAt"),
                    name.into(),
                    "Saved reminder is due.".into(),
                    json!({"module":"Calendar","date":day,"recordId":booking.unwrap_or(r)["id"]}),
                    "high",
                );
            }
        }
        let stays: Vec<_> = records
            .iter()
            .filter(|r| text(r, "kind") == "stay" && text(r, "status") != "cancelled")
            .collect();
        if let (Some(first), Some(last)) = (
            stays.iter().map(|r| text(r, "checkIn")).min(),
            stays.iter().map(|r| text(r, "checkOut")).max(),
        ) {
            let begin = first.max(start.as_str());
            let end = last.min(horizon.as_str());
            if begin < end {
                let analysis = journey::gaps(&records, &json!({"start":begin,"end":end}))?;
                for r in analysis["ranges"].as_array().unwrap() {
                    add(
                        "gap",
                        &json!({}),
                        text(r, "start"),
                        format!(
                            "Accommodation {}: {} nights",
                            text(r, "status"),
                            r["nights"]
                        ),
                        format!("{} → {}", text(r, "start"), text(r, "end")),
                        r["target"].clone(),
                        "normal",
                    );
                }
            }
        }
        let work = schedule::window(
            &records,
            &settings,
            &json!({"date":start,"dateBasis":"destination"}),
        )?;
        for conflict in work["conflicts"].as_array().into_iter().flatten() {
            add(
                "work",
                &json!({"id":format!("{}:{}",text(conflict,"recordId"),text(conflict,"workBlockId"))}),
                &start,
                "Work schedule conflict".into(),
                text(conflict, "message").into(),
                json!({"module":"Calendar","date":start,"action":"work"}),
                "high",
            );
        }
        let summary = money::summary(
            &records,
            &settings,
            &json!({"month":today.format("%Y-%m").to_string()}),
        )?;
        if summary["hasBudget"] == true && summary["availableMinor"].as_i64().is_some_and(|v| v < 0)
        {
            add(
                "budget",
                &json!({}),
                &start[..7],
                "Projected funds below reserve".into(),
                "Review known commitments, expenses and the selected reserve.".into(),
                json!({"module":"Budget","month":&start[..7]}),
                "high",
            );
        }
        for mut row in generated {
            row["createdAt"] = json!(timestamp);
            self.db
                .execute(
                    "INSERT OR IGNORE INTO notifications(id,body,created_at) VALUES(?,?,?)",
                    params![text(&row, "id"), row.to_string(), timestamp],
                )
                .map_err(error)?;
        }
        let offset = p["offset"].as_i64().unwrap_or(0).max(0);
        let mut stmt=self.db.prepare("SELECT body,read_at FROM notifications WHERE dismissed_at IS NULL ORDER BY created_at DESC,id LIMIT 100 OFFSET ?").map_err(error)?;
        let rows = stmt
            .query_map([offset], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        let mut items = vec![];
        for (body, read) in rows {
            let mut v: Value = serde_json::from_str(&body).map_err(error)?;
            v["readAt"] = json!(read);
            items.push(v);
        }
        let unread: i64 = self
            .db
            .query_row(
                "SELECT count(*) FROM notifications WHERE read_at IS NULL AND dismissed_at IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(error)?;
        Ok(json!({"items":items,"unread":unread}))
    }
}
