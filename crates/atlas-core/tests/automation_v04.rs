use atlas_core::Atlas;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const CONFIRMATION:&str="Booking confirmation\nProvider: Synthetic Stays\nReservation ID: V04-TEST\nProperty: River apartment\nCheck-in: 2028-12-12\nCheck-out: 2029-01-11\nCheck-in time: 15:00\nCheck-out time: 11:00\nTimezone: Europe/Lisbon\nTotal: USD 900.00\nRefrigerator: yes\nWorkspace: yes\nCancellation deadline: 2028-12-10 18:00\nPayment due: 2028-12-01 12:00\nStatus: confirmed";
fn call(a: &mut Atlas, m: &str, p: Value) -> Value {
    a.call(m, p).unwrap_or_else(|e| panic!("{m}: {e}"))
}
fn save(a: &mut Atlas, r: Value) -> Value {
    call(a, "records.save", json!({"record":r}))
}
fn parse(a: &mut Atlas, t: &str) -> Value {
    call(a, "imports.parse", json!({"text":t}))[0].clone()
}
fn accepted(a: &mut Atlas) -> Value {
    let d = parse(a, CONFIRMATION);
    call(a, "imports.accept", json!({"id":d["id"]}))
}
#[test]
fn selective_revision_keeps_confirmation_evidence_and_recalculates_reminders() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let stay = accepted(&mut a);
    let suggestions = call(
        &mut a,
        "reminders.suggestions",
        json!({"now":"2028-01-01T00:00:00Z"}),
    );
    let checkout = suggestions
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["rule"] == "checkout")
        .unwrap();
    let reminder = call(
        &mut a,
        "reminders.adopt",
        json!({"suggestionKey":checkout["suggestionKey"]}),
    );
    assert_eq!(
        call(
            &mut a,
            "reminders.adopt",
            json!({"suggestionKey":checkout["suggestionKey"]})
        )["id"],
        reminder["id"]
    );
    let d=parse(&mut a,"Revised reservation\nProvider: Synthetic Stays\nReservation ID: V04-TEST\nCheck-out: 2029-01-12\nTotal: USD 950.00");
    assert!(d["record"]["status"].is_null());
    let preview = call(&mut a, "imports.preview", json!({"id":d["id"]}));
    assert_eq!(preview["before"]["id"], stay["id"]);
    let saved = call(
        &mut a,
        "imports.accept",
        json!({"id":d["id"],"fields":["checkOut"],"expectedVersion":stay["version"]}),
    );
    assert_eq!(saved["status"], "confirmed");
    assert_eq!(saved["totalMinor"], 90000);
    assert_eq!(saved["checkOut"], "2029-01-12");
    assert_eq!(saved["facilities"]["refrigerator"], true);
    let reminders = call(&mut a, "records.list", json!({"kind":"reminder"}));
    assert_eq!(reminders[0]["dueAt"], "2029-01-11T11:00:00+00:00");
    assert_eq!(reminders[0]["id"], reminder["id"]);
    let history = call(&mut a, "records.history", json!({"id":stay["id"]}));
    assert!(history.as_array().unwrap().len() >= 2);
    let before = call(&mut a, "snapshot", json!({}));
    drop(a);
    let mut a = Atlas::open(dir.path()).unwrap();
    assert_eq!(before, call(&mut a, "snapshot", json!({})));
}
#[test]
fn cancellations_and_partial_refunds_preserve_original_cash_and_require_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let stay = accepted(&mut a);
    let payment = save(
        &mut a,
        json!({"kind":"payment","bookingId":stay["id"],"amountMinor":90000,"currency":"USD","date":"2028-11-01","type":"payment","status":"settled"}),
    );
    let d=parse(&mut a,"Reservation cancelled\nProvider: Synthetic Stays\nReservation ID: V04-TEST\nRefund expected\nRefund amount: USD 900.00\nRefund expected date: 2028-12-02");
    let cancelled = call(&mut a, "imports.accept", json!({"id":d["id"]}));
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["id"], stay["id"]);
    let refund = call(&mut a, "records.list", json!({"kind":"refund"}))[0].clone();
    assert_eq!(refund["state"], "expected");
    assert_eq!(
        call(&mut a, "records.list", json!({"kind":"payment"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let request = json!({"id":refund["id"],"expectedVersion":refund["version"],"paymentId":payment["id"],"amountMinor":40000,"currency":"USD","date":"2028-12-02","requestId":uuid::Uuid::new_v4().to_string(),"confirmReceived":false});
    assert!(a.call("refunds.receive", request.clone()).is_err());
    let mut request = request;
    request["confirmReceived"] = json!(true);
    let first = call(&mut a, "refunds.receive", request.clone());
    assert_eq!(first["amountMinor"], -40000);
    assert_eq!(call(&mut a, "refunds.receive", request)["id"], first["id"]);
    let partial = call(&mut a, "records.list", json!({"kind":"refund"}))[0].clone();
    assert_eq!(partial["state"], "expected");
    assert_eq!(partial["receivedMinor"], 40000);
    call(
        &mut a,
        "refunds.receive",
        json!({"id":refund["id"],"expectedVersion":partial["version"],"paymentId":payment["id"],"amountMinor":50000,"currency":"USD","date":"2028-12-03","requestId":uuid::Uuid::new_v4().to_string(),"confirmReceived":true}),
    );
    let refunds = call(&mut a, "records.list", json!({"kind":"refund"}));
    assert_eq!(refunds[0]["state"], "received");
    let payments = call(&mut a, "records.list", json!({"kind":"payment"}));
    assert!(payments.as_array().unwrap().contains(&payment));
    let budget = call(
        &mut a,
        "budget.summary",
        json!({"month":"2028-12","currency":"USD"}),
    );
    assert_eq!(budget["accommodationMinor"], 0);
    assert_eq!(budget["bookingCashFlowMinor"], -90000);
}
#[test]
fn cancellation_field_can_be_kept_and_stale_reviews_fail_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let stay = accepted(&mut a);
    let d = parse(
        &mut a,
        "Reservation cancelled\nProvider: Synthetic Stays\nReservation ID: V04-TEST",
    );
    let kept = call(&mut a, "imports.accept", json!({"id":d["id"],"fields":[]}));
    assert_eq!(kept["status"], "confirmed");
    let d=parse(&mut a,"Revised reservation\nProvider: Synthetic Stays\nReservation ID: V04-TEST\nCheck-out: 2029-01-13");
    let mut edit = kept;
    edit["name"] = json!("Owner edit");
    save(&mut a, edit);
    let snapshot = call(&mut a, "snapshot", json!({}));
    assert!(a.call("imports.accept", json!({"id":d["id"]})).is_err());
    assert_eq!(snapshot, call(&mut a, "snapshot", json!({})));
    assert_eq!(stay["version"], 1);
}
#[test]
fn readiness_unknowns_and_snooze_survive_revision_without_duplicate_rules() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    let mut stay = accepted(&mut a);
    let readiness = call(
        &mut a,
        "readiness.summary",
        json!({"start":"2028-12-01","end":"2029-01-15"}),
    );
    assert_eq!(readiness["ready"], false);
    assert!(readiness["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["group"] == "unknown" && i["kind"] == "workspace"));
    let suggestions = call(&mut a, "reminders.suggestions", json!({}));
    let checkout = suggestions
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["rule"] == "checkout")
        .unwrap();
    let mut reminder = call(
        &mut a,
        "reminders.adopt",
        json!({"suggestionKey":checkout["suggestionKey"]}),
    );
    reminder["state"] = json!("snoozed");
    reminder["dueAt"] = json!("2029-01-10T20:00:00Z");
    save(&mut a, reminder);
    stay["checkOut"] = json!("2029-01-12");
    let mut stay = save(&mut a, stay);
    let reminders = call(&mut a, "records.list", json!({"kind":"reminder"}));
    assert_eq!(reminders[0]["dueAt"], "2029-01-10T20:00:00Z");
    assert_eq!(reminders[0]["baseDueAt"], "2029-01-11T11:00:00+00:00");
    stay["status"] = json!("cancelled");
    save(&mut a, stay);
    assert!(call(
        &mut a,
        "reminders.due",
        json!({"now":"2030-01-01T00:00:00Z"})
    )
    .as_array()
    .unwrap()
    .is_empty());
}
#[test]
fn offer_comparison_scenario_and_watchlist_are_exact_and_noncommitting() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    call(
        &mut a,
        "settings.save",
        json!({"preferences":{"currency":"USD","refrigeratorRequired":true,"workspaceRequired":true}}),
    );
    let one = save(
        &mut a,
        json!({"kind":"quote","name":"Verified apartment","checkIn":"2028-12-12","checkOut":"2029-01-11","totalMinor":90000,"currency":"USD","feesComplete":true,"facilities":{"workspace":true,"refrigerator":true},"checkedAt":"2028-11-01","timezone":"Europe/Lisbon"}),
    );
    let two = save(
        &mut a,
        json!({"kind":"quote","name":"Unknown apartment","checkIn":"2028-12-12","checkOut":"2029-01-11","totalMinor":80000,"currency":"USD","feesComplete":false,"facilities":{}}),
    );
    let before = call(&mut a, "snapshot", json!({}));
    let ranking = call(
        &mut a,
        "discovery.compare",
        json!({"quoteIds":[one["id"],two["id"]],"now":"2028-11-02T00:00:00Z"}),
    );
    assert_eq!(ranking["ranked"][0]["id"], one["id"]);
    assert_eq!(ranking["ranked"][1]["requirementsVerified"], false);
    let scenario = call(&mut a, "discovery.scenario", json!({"quoteId":one["id"]}));
    assert_eq!(scenario["after"]["accommodationMinor"], 90000);
    assert_eq!(scenario["after"]["months"][0]["accommodationMinor"], 60000);
    assert_eq!(scenario["after"]["months"][1]["accommodationMinor"], 30000);
    assert_eq!(before, call(&mut a, "snapshot", json!({})));
    let snapshot = call(
        &mut a,
        "watchlist.snapshot",
        json!({"id":one["id"],"expectedVersion":one["version"],"snapshot":{"priceMinor":92000,"currency":"USD","availability":"unknown","evidence":"Synthetic manual check"}}),
    );
    assert_eq!(snapshot["totalMinor"], 90000);
    assert_eq!(snapshot["lastSnapshot"]["priceMinor"], 92000);
    assert!(a.call("watchlist.snapshot",json!({"id":snapshot["id"],"expectedVersion":snapshot["version"],"snapshot":{"sourceUrl":"javascript:alert(1)"}})).is_err());
}
#[test]
fn preceding_transport_and_assistant_readiness_use_saved_references() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    accepted(&mut a);
    let flight = save(
        &mut a,
        json!({"kind":"transport","name":"Synthetic onward flight","origin":"Lisbon","destination":"Rome","departure":"2029-01-11T12:00:00Z","arrival":"2029-01-11T15:00:00Z","departureTimezone":"Europe/Lisbon","arrivalTimezone":"Europe/Rome","totalMinor":10000,"currency":"USD","status":"confirmed"}),
    );
    let origin = call(&mut a, "planning.origin", json!({"date":"2029-01-12"}));
    assert_eq!(origin["location"]["recordId"], flight["id"]);
    let chat = call(&mut a, "chat.create", json!({}));
    let answer = call(
        &mut a,
        "chat.send",
        json!({"id":chat["id"],"requestId":uuid::Uuid::new_v4().to_string(),"text":"Am I prepared for my next trip?","context":{"date":"2028-12-01"}}),
    );
    assert!(answer.to_string().contains("readiness"));
}
#[test]
fn schema_four_restore_preserves_chat_notifications_documents_and_pairing() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    accepted(&mut a);
    let chat = call(&mut a, "chat.create", json!({}));
    call(
        &mut a,
        "chat.send",
        json!({"id":chat["id"],"requestId":uuid::Uuid::new_v4().to_string(),"text":"Show imports","context":{"date":"2028-12-01"}}),
    );
    call(
        &mut a,
        "notifications.list",
        json!({"now":"2028-12-01T00:00:00Z"}),
    );
    call(
        &mut a,
        "documents.attach",
        json!({"filename":"synthetic-v03.txt","contentBase64":STANDARD.encode(b"Synthetic travel document")}),
    );
    let before = call(&mut a, "snapshot", json!({}));
    let chats = call(&mut a, "chat.get", json!({"id":chat["id"]}));
    let backup = call(&mut a, "backup.export", json!({}));
    drop(a);
    std::fs::write(
        dir.path().join("companion-devices.json"),
        b"synthetic-pairing-preservation",
    )
    .unwrap();
    rusqlite::Connection::open(dir.path().join("atlas.sqlite3"))
        .unwrap()
        .execute_batch("PRAGMA user_version=4")
        .unwrap();
    let mut a = Atlas::open(dir.path()).unwrap();
    assert_eq!(before, call(&mut a, "snapshot", json!({})));
    assert_eq!(chats, call(&mut a, "chat.get", json!({"id":chat["id"]})));
    assert_eq!(
        std::fs::read(dir.path().join("companion-devices.json")).unwrap(),
        b"synthetic-pairing-preservation"
    );
    assert!(std::fs::read_dir(dir.path().join("backups"))
        .unwrap()
        .flatten()
        .any(|e| e
            .file_name()
            .to_string_lossy()
            .starts_with("pre-migration-v4")));
    let folder = dir
        .path()
        .join("backups")
        .join(backup["filename"].as_str().unwrap());
    rusqlite::Connection::open(folder.join("atlas.sqlite3"))
        .unwrap()
        .execute_batch("PRAGMA user_version=4")
        .unwrap();
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    manifest["schemaVersion"] = json!(4);
    manifest["databaseSha256"] = json!(format!(
        "{:x}",
        Sha256::digest(std::fs::read(folder.join("atlas.sqlite3")).unwrap())
    ));
    std::fs::write(
        folder.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let mut changed = before["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "stay")
        .unwrap()
        .clone();
    changed["name"] = json!("After backup edit");
    save(&mut a, changed);
    call(
        &mut a,
        "backup.restore",
        json!({"filename":backup["filename"]}),
    );
    assert_eq!(before, call(&mut a, "snapshot", json!({})));
    assert_eq!(chats, call(&mut a, "chat.get", json!({"id":chat["id"]})));
}
