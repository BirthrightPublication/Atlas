use atlas_core::Atlas;
use serde_json::{json, Value};
use std::sync::{Arc, Barrier};

fn stay() -> Value {
    json!({"kind":"stay","name":"Synthetic concurrency stay","checkIn":"2026-12-12","checkOut":"2027-01-11","timezone":"Europe/Lisbon","totalMinor":90000,"currency":"USD","status":"confirmed","provider":"Synthetic","reservationId":"TEST-BOUNDARY-1"})
}

#[test]
fn separate_clients_cannot_overwrite_the_same_version() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let record = app.call("records.save", json!({"record":stay()})).unwrap();
    drop(app);
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|n| {
            let mut app = Atlas::open(directory.path()).unwrap();
            let mut record = record.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                record["name"] = json!(format!("Edit {n}"));
                barrier.wait();
                app.call("records.save", json!({"record":record}))
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .all(|e| e.contains("Conflict")));
    let mut app = Atlas::open(directory.path()).unwrap();
    let saved = app.call("records.list", json!({"kind":"stay"})).unwrap();
    assert_eq!(saved[0]["version"], 2);
}

#[test]
fn reservations_are_unique_within_kind_and_provider() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    app.call("records.save", json!({"record":stay()})).unwrap();
    assert!(app.call("records.save", json!({"record":stay()})).is_err());
    let trip = json!({"kind":"transport","name":"Synthetic bundled train","provider":"Synthetic","reservationId":"TEST-BOUNDARY-1","departure":"2026-12-12T08:00:00Z","arrival":"2026-12-12T11:00:00Z","departureTimezone":"Europe/Lisbon","arrivalTimezone":"Europe/Lisbon","currency":"USD","totalMinor":1000,"status":"confirmed"});
    assert!(app.call("records.save", json!({"record":trip})).is_ok());
}

#[test]
fn historic_settlement_basis_is_validated_and_fx_does_not_mutate_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let stay = app.call("records.save", json!({"record":stay()})).unwrap();
    let mut payment = json!({"kind":"payment","bookingId":stay["id"],"date":"2026-11-20","currency":"EUR","amountMinor":85000,"bookingCurrency":"USD","bookingAmountMinor":90000,"rateDate":"2026-11-20","rateSource":"Synthetic bank settlement","status":"settled","type":"payment"});
    let paid = app.call("records.save", json!({"record":payment})).unwrap();
    assert_eq!(
        app.call(
            "budget.summary",
            json!({"month":"2026-12","currency":"USD"})
        )
        .unwrap()["paymentStatuses"][0]["status"],
        "paid"
    );
    let result=app.call("fx.convert",json!({"amountMinor":90000,"fromCurrency":"USD","toCurrency":"EUR","rateNumerator":95,"rateDenominator":100,"rateDate":"2026-12-01"})).unwrap();
    assert_eq!(result["convertedMinor"], 85500);
    assert_eq!(
        app.call("records.list", json!({"kind":"payment"})).unwrap()[0],
        paid
    );
    payment["bookingCurrency"] = json!("JPY");
    assert!(app.call("records.save", json!({"record":payment})).is_err());
}

#[test]
fn invalid_schedules_and_nested_money_do_not_poison_saved_data() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    assert!(app.call("records.save",json!({"record":{"kind":"workBlock","name":"Invalid","start":"nonsense","end":"2026-01-01T12:00:00Z"}})).is_err());
    assert!(app.call("records.save",json!({"record":{"kind":"budget","month":"2026-12","currency":"USD","recurringExpenses":[{"name":"Invalid","amountMinor":2.5}]}})).is_err());
    assert_eq!(app.call("records.list", json!({})).unwrap(), json!([]));
}

#[test]
fn pasted_links_are_reviewable_places_and_unknown_revision_fields_preserve_facts() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let drafts = app
        .call(
            "imports.parse",
            json!({"text":"https://example.com/synthetic-place"}),
        )
        .unwrap();
    assert_eq!(
        app.call("imports.accept", json!({"id":drafts[0]["id"]}))
            .unwrap()["kind"],
        "place"
    );
    let mut record = stay();
    record["contact"] = json!("Synthetic host contact");
    record["facilities"] = json!({"refrigerator":true});
    let saved = app.call("records.save", json!({"record":record})).unwrap();
    let text="Booking confirmation\nProvider: Synthetic\nReservation ID: TEST-BOUNDARY-1\nProperty: Synthetic concurrency stay\nCheck-in: 2026-12-12\nCheck-out: 2027-01-11\nTimezone: Europe/Lisbon\nTotal: USD 930.00\nRevised confirmation";
    let drafts = app.call("imports.parse", json!({"text":text})).unwrap();
    let updated = app
        .call("imports.accept", json!({"id":drafts[0]["id"]}))
        .unwrap();
    assert_eq!(updated["id"], saved["id"]);
    assert_eq!(updated["contact"], saved["contact"]);
    assert_eq!(updated["facilities"]["refrigerator"], true);
}

#[test]
fn explicit_assistant_reminder_has_a_real_persistent_record_and_undo() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Atlas::open(directory.path()).unwrap();
    let reply = app
        .call(
            "assistant.ask",
            json!({"text":"Remind me: Synthetic checkout at 2026-12-10T09:00:00+01:00"}),
        )
        .unwrap();
    let reminders = app
        .call("records.list", json!({"kind":"reminder"}))
        .unwrap();
    assert_eq!(reminders.as_array().unwrap().len(), 1);
    assert_eq!(reminders[0]["name"], "Synthetic checkout");
    let listing = app
        .call(
            "assistant.ask",
            json!({"text":"Show my upcoming reminders"}),
        )
        .unwrap();
    assert_eq!(listing["records"][0]["id"], reminders[0]["id"]);
    app.call(
        reply["undo"]["method"].as_str().unwrap(),
        reply["undo"]["payload"].clone(),
    )
    .unwrap();
    assert!(app
        .call("records.list", json!({"kind":"reminder"}))
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    app.call(
        "assistant.ask",
        json!({"text":"Should I create a reminder before checkout?"}),
    )
    .unwrap();
    assert!(app
        .call("records.list", json!({"kind":"reminder"}))
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}
