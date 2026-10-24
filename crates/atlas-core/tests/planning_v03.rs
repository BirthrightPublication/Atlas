use atlas_core::Atlas;
use serde_json::{json, Value};
use tempfile::tempdir;

fn save(app: &mut Atlas, r: Value) -> Value {
    app.call("records.save", json!({"record":r})).unwrap()
}
fn stay(name: &str, a: &str, b: &str) -> Value {
    json!({"kind":"stay","name":name,"type":"apartment","checkIn":a,"checkOut":b,"timezone":"Europe/Lisbon","status":"confirmed","currency":"USD","totalMinor":90000})
}

#[test]
fn conversation_survives_restart_retry_reopen_rename_and_delete() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let chat = app.call("chat.create", json!({})).unwrap();
    let cid = chat["id"].clone();
    let request = json!({"id":cid,"text":"Remind me: Synthetic checkout at 2027-02-01T09:00:00Z","requestId":"retry-stable","context":{"module":"Calendar","date":"2027-01-01","month":"2027-01"}});
    let sent = app.call("chat.send", request.clone()).unwrap();
    assert_eq!(sent["messages"].as_array().unwrap().len(), 2);
    let retry = app.call("chat.send", request).unwrap();
    assert_eq!(retry, sent);
    assert_eq!(
        app.call("records.list", json!({"kind":"reminder"}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    app.call(
        "chat.rename",
        json!({"id":cid,"title":"Synthetic retained chat"}),
    )
    .unwrap();
    drop(app);
    let mut app = Atlas::open(dir.path()).unwrap();
    let reopened = app.call("chat.get", json!({"id":cid})).unwrap();
    assert_eq!(reopened["title"], "Synthetic retained chat");
    assert_eq!(reopened["messages"], sent["messages"]);
    let second = app.call("chat.create", json!({})).unwrap();
    assert_ne!(second["id"], cid);
    assert!(second["messages"].as_array().unwrap().is_empty());
    let undone = app
        .call(
            "chat.undo",
            json!({"id":cid,"sequence":sent["messages"][1]["sequence"]}),
        )
        .unwrap();
    assert!(undone["messages"][1]["result"]["undo"].is_null());
    assert!(app
        .call("records.list", json!({"kind":"reminder"}))
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    app.call("chat.delete", json!({"id":cid})).unwrap();
    assert!(app.call("chat.get", json!({"id":cid})).is_err());
    assert_eq!(
        app.call("chat.list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn context_budget_uses_selected_month_and_exact_existing_accounting() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    save(
        &mut app,
        stay("Synthetic cross-month", "2026-12-12", "2027-01-11"),
    );
    let answer=app.call("assistant.ask",json!({"text":"What is my daily cost?","context":{"date":"2026-12-14","month":"2026-12"}})).unwrap();
    let budget = app
        .call("budget.summary", json!({"month":"2026-12"}))
        .unwrap();
    assert_eq!(answer["summary"], budget);
    assert_eq!(answer["summary"]["accommodationMinor"], 60000);
    assert_eq!(answer["targets"][0]["target"]["month"], "2026-12");
}

#[test]
fn gap_ranges_distinguish_tentative_vehicle_overnight_and_overlap() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    save(&mut app, stay("First", "2027-01-01", "2027-01-03"));
    let mut tentative = stay("Maybe", "2027-01-03", "2027-01-04");
    tentative["status"] = json!("tentative");
    save(&mut app, tentative);
    save(
        &mut app,
        json!({"kind":"transport","name":"Overnight train","mode":"train","departure":"2027-01-04T22:00:00Z","arrival":"2027-01-05T07:00:00Z","departureTimezone":"Europe/Lisbon","arrivalTimezone":"Europe/Madrid","overnight":true,"status":"confirmed","currency":"USD","totalMinor":1000}),
    );
    let mut vehicle = stay("Vehicle", "2027-01-06", "2027-01-08");
    vehicle["type"] = json!("vehicle");
    vehicle["totalMinor"] = json!(0);
    save(&mut app, vehicle);
    save(&mut app, stay("Overlap", "2027-01-07", "2027-01-09"));
    let result = app
        .call(
            "planning.gaps",
            json!({"start":"2027-01-01","end":"2027-01-09"}),
        )
        .unwrap();
    let rows = result["nights"].as_array().unwrap();
    assert_eq!(rows[2]["status"], "tentative");
    assert_eq!(rows[3]["status"], "overnight_transport");
    assert_eq!(rows[4]["status"], "gap");
    assert_eq!(rows[5]["vehicle"], true);
    assert_eq!(rows[5]["noCost"], true);
    assert_eq!(rows[6]["status"], "overlap");
    assert_eq!(result["ranges"][1]["previousStay"]["name"], "Maybe");
    assert_eq!(result["ranges"][1]["nextStay"]["name"], "Vehicle");
}

#[test]
fn dateline_overnight_and_duration_use_actual_instants() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let flight = json!({"kind":"transport","name":"Synthetic date-line flight","mode":"flight","departure":"2027-01-04T23:00:00+13:00","arrival":"2027-01-04T13:00:00-08:00","departureTimezone":"Pacific/Auckland","arrivalTimezone":"America/Los_Angeles","overnight":true,"status":"confirmed","currency":"USD","totalMinor":1000});
    save(&mut app, flight.clone());
    let preview = app
        .call("planning.preview", json!({"record":flight}))
        .unwrap();
    assert_eq!(preview["durationMinutes"], 660);
    let result = app
        .call(
            "planning.gaps",
            json!({"start":"2027-01-04","end":"2027-01-06"}),
        )
        .unwrap();
    assert_eq!(result["nights"][0]["status"], "overnight_transport");
    assert_eq!(result["nights"][1]["status"], "gap");
}

#[test]
fn preview_reports_unknown_workspace_and_protected_work_without_writing() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    save(
        &mut app,
        json!({"kind":"workBlock","name":"Protected","date":"2027-01-04","start":"2027-01-04T17:00:00Z","end":"2027-01-05T01:00:00Z","timezone":"UTC"}),
    );
    let before = app.call("snapshot", json!({})).unwrap();
    let draft = json!({"kind":"transport","name":"Proposed train","mode":"train","departure":"2027-01-04T18:00:00Z","arrival":"2027-01-04T20:00:00Z","departureTimezone":"UTC","arrivalTimezone":"UTC","status":"tentative"});
    let preview = app
        .call("planning.preview", json!({"record":draft}))
        .unwrap();
    assert!(!preview["work"]["conflicts"].as_array().unwrap().is_empty());
    assert_eq!(preview["recordsChanged"], false);
    assert_eq!(app.call("snapshot", json!({})).unwrap(), before);
    let unknown = app
        .call(
            "planning.preview",
            json!({"record":stay("Unknown workspace","2027-01-04","2027-01-06")}),
        )
        .unwrap();
    assert_eq!(unknown["workspace"]["status"], "potential");
    assert!(unknown["workspace"]["unknown"].as_array().unwrap().len() >= 4);
}

#[test]
fn notification_dedup_read_dismiss_and_deleted_target_survive_restart() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let booking = save(
        &mut app,
        stay("Synthetic upcoming", "2027-01-03", "2027-01-05"),
    );
    let p = json!({"now":"2027-01-01T12:00:00Z"});
    let first = app.call("notifications.list", p.clone()).unwrap();
    let second = app.call("notifications.list", p.clone()).unwrap();
    assert_eq!(first, second);
    let checkin = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["type"] == "checkIn")
        .unwrap();
    let nid = checkin["id"].clone();
    assert_eq!(checkin["target"]["recordId"], booking["id"]);
    app.call("notifications.read", json!({"id":nid})).unwrap();
    app.call(
        "records.delete",
        json!({"id":booking["id"],"expectedVersion":booking["version"]}),
    )
    .unwrap();
    drop(app);
    let mut app = Atlas::open(dir.path()).unwrap();
    let reopened = app.call("notifications.list", p.clone()).unwrap();
    assert!(reopened["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == nid)
        .unwrap()["readAt"]
        .is_string());
    app.call("notifications.dismiss", json!({"id":nid}))
        .unwrap();
    let dismissed = app.call("notifications.list", p.clone()).unwrap();
    assert!(!dismissed["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["id"] == nid));
    app.call("notifications.read_all", json!({})).unwrap();
    assert_eq!(app.call("notifications.list", p).unwrap()["unread"], 0);
}

#[test]
fn schema_three_migration_backs_up_and_retains_existing_data() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let saved = save(&mut app, stay("Retained", "2027-01-01", "2027-01-03"));
    drop(app);
    let db = rusqlite::Connection::open(dir.path().join("atlas.sqlite3")).unwrap();
    db.execute_batch("DROP TABLE chat_messages; DROP TABLE conversations; DROP TABLE notifications; PRAGMA user_version=3;").unwrap();
    drop(db);
    let mut app = Atlas::open(dir.path()).unwrap();
    assert_eq!(
        app.call("records.list", json!({"kind":"stay"})).unwrap()[0],
        saved
    );
    assert!(std::fs::read_dir(dir.path().join("backups"))
        .unwrap()
        .any(|f| f
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("pre-migration-v3-")));
    let chat = app.call("chat.create", json!({})).unwrap();
    app.call(
        "chat.send",
        json!({"id":chat["id"],"requestId":"backup-chat","text":"How does Atlas store my data?"}),
    )
    .unwrap();
    let backup = app.call("backup.export", json!({})).unwrap();
    app.call("chat.delete", json!({"id":chat["id"]})).unwrap();
    app.call("backup.restore", json!({"filename":backup["filename"]}))
        .unwrap();
    assert_eq!(
        app.call("chat.get", json!({"id":chat["id"]})).unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn transport_scenario_is_exact_currency_specific_and_does_not_reprice_history() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let transport = save(
        &mut app,
        json!({"kind":"transport","name":"Synthetic train scenario","mode":"train","departure":"2027-02-01T10:00:00Z","arrival":"2027-02-01T12:00:00Z","departureTimezone":"UTC","arrivalTimezone":"UTC","currency":"EUR","totalMinor":2800,"status":"tentative"}),
    );
    let before = app.call("snapshot", json!({})).unwrap();
    let scenario = app
        .call(
            "planning.what_if",
            json!({"transportId":transport["id"],"totalMinor":5600,"month":"2027-02"}),
        )
        .unwrap();
    assert_eq!(scenario["currency"], "EUR");
    assert_eq!(scenario["before"]["forecastMinor"], 0);
    assert_eq!(scenario["after"]["forecastMinor"], 5600);
    assert_eq!(scenario["after"]["dailyForecastMinor"], 200.0);
    assert_eq!(app.call("snapshot", json!({})).unwrap(), before);
    assert!(app
        .call(
            "planning.what_if",
            json!({"transportId":transport["id"],"totalMinor":-1,"month":"2027-02"})
        )
        .is_err());
}

#[test]
fn transport_checks_arrival_work_deadline_and_checkout_without_a_destination_stay() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let origin = save(
        &mut app,
        stay("Synthetic origin", "2027-01-01", "2027-01-04"),
    );
    save(
        &mut app,
        json!({"kind":"workBlock","name":"Arrival day protected","date":"2027-01-05","start":"2027-01-05T17:00:00Z","end":"2027-01-06T01:00:00Z","timezone":"UTC"}),
    );
    let proposed = json!({"kind":"transport","name":"Long proposed journey","mode":"train","departure":"2027-01-04T18:00:00Z","arrival":"2027-01-05T18:00:00Z","departureTimezone":"UTC","arrivalTimezone":"UTC","originStayId":origin["id"],"arrivalRequiredAt":"2027-01-04T16:00:00Z","status":"tentative"});
    let result = app
        .call("planning.preview", json!({"record":proposed}))
        .unwrap();
    assert_eq!(result["durationMinutes"], 1440);
    assert_eq!(result["workChecks"].as_array().unwrap().len(), 2);
    assert!(result["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["type"] == "late_arrival"));
    assert!(result["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["type"] == "checkout_departure" && v["minutes"] == 420));
    assert!(result["workChecks"][1]["result"]["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["type"] == "transit"));
}

#[test]
fn alternate_workspace_uses_recorded_venue_hours_and_commute() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    let venue = save(
        &mut app,
        json!({"kind":"place","name":"Synthetic library","timezone":"UTC","workspaceHours":{"start":"18:00","end":"23:00","weekdays":[1,2,3,4,5]}}),
    );
    let mut booking = stay("Synthetic work stay", "2027-01-01", "2027-01-10");
    booking["alternateWorkspaceId"] = venue["id"].clone();
    booking["workspaceCommuteMinutes"] = json!(30);
    save(&mut app, booking);
    save(
        &mut app,
        json!({"kind":"workBlock","name":"Protected","date":"2027-01-04","start":"2027-01-04T17:00:00Z","end":"2027-01-05T01:00:00Z","timezone":"UTC"}),
    );
    save(
        &mut app,
        json!({"kind":"transport","name":"Arriving before work","mode":"train","departure":"2027-01-04T16:00:00Z","arrival":"2027-01-04T16:45:00Z","departureTimezone":"UTC","arrivalTimezone":"UTC","status":"confirmed","currency":"USD","totalMinor":100}),
    );
    let result = app
        .call("work.window", json!({"date":"2027-01-04","timezone":"UTC"}))
        .unwrap();
    assert!(result["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["type"] == "workspace_closed"));
    assert!(result["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["type"] == "workspace_commute"));
    assert!(app
        .call(
            "records.save",
            json!({"record":{"kind":"place","name":"Bad hours","workspaceHours":{"start":"09:00"}}})
        )
        .is_err());
}

#[test]
fn native_update_notifications_deduplicate_and_keep_read_state() {
    let dir = tempdir().unwrap();
    let mut app = Atlas::open(dir.path()).unwrap();
    app.record_update_notice("0.3.1").unwrap();
    app.record_update_notice("0.3.1").unwrap();
    let result = app.call("notifications.list", json!({})).unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["target"]["action"], "updates");
    app.call("notifications.read_all", json!({})).unwrap();
    drop(app);
    let mut app = Atlas::open(dir.path()).unwrap();
    app.record_update_notice("0.3.1").unwrap();
    assert_eq!(
        app.call("notifications.list", json!({})).unwrap()["unread"],
        0
    );
    assert!(app
        .call("notifications.update", json!({"version":"9.0.0"}))
        .is_err());
}
