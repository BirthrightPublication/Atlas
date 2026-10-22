#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use atlas_core::Atlas;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

mod companion;
use companion::CompanionState;

#[derive(Clone)]
struct AtlasState(Arc<Mutex<Atlas>>);
#[derive(Default)]
struct UpdateState(Mutex<Option<Update>>);

#[tauri::command]
async fn atlas_call(
    app: tauri::AppHandle,
    state: State<'_, AtlasState>,
    companion_state: State<'_, CompanionState>,
    method: String,
    payload: Option<Value>,
) -> Result<Value, String> {
    if method == "ai.cancel" {
        return Ok(atlas_core::ai::cancel());
    }
    if method.starts_with("companion.") {
        let companion = companion_state.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            companion::call(
                &app,
                &companion,
                &method,
                &payload.unwrap_or_else(|| json!({})),
            )
        })
        .await
        .map_err(|_| "Companion worker interrupted".to_string())?;
    }
    let atlas = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let payload = payload.unwrap_or_else(|| json!({}));
        if method == "ai.extract" {
            let settings = atlas
                .lock()
                .map_err(|_| "Application service unavailable")?
                .call("settings.get", json!({}))?;
            return atlas_core::ai::extract(
                payload.get("text").and_then(Value::as_str).unwrap_or(""),
                &settings,
            );
        }
        let mut service = atlas
            .lock()
            .map_err(|_| "Application service unavailable")?;
        service.call(&method, payload)
    })
    .await
    .map_err(|_| "Application worker interrupted".to_string())?
}

#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|_| "Invalid link")?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Only HTTP(S) links without embedded credentials can be opened".into());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|_| "Could not open the system browser".into())
}

#[tauri::command]
async fn update_status(
    app: tauri::AppHandle,
    state: State<'_, UpdateState>,
) -> Result<Value, String> {
    let current = app.package_info().version.to_string();
    let (Some(key), Some(endpoint)) = (
        option_env!("ATLAS_UPDATE_PUBLIC_KEY"),
        option_env!("ATLAS_UPDATE_ENDPOINT"),
    ) else {
        return Ok(
            json!({"configured": false, "available": false, "currentVersion":current,
            "reason":"Signed updates are not configured for this build. Install a verified release manually."}),
        );
    };
    let parsed = url::Url::parse(endpoint).map_err(|_| "Invalid compiled update endpoint")?;
    if parsed.scheme() != "https" || key.trim().is_empty() {
        return Err("Updates require an HTTPS feed and a verification public key".into());
    }
    let update = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![parsed])
        .map_err(|_| "Invalid update configuration")?
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "Could not initialize signed updater")?
        .check()
        .await
        .map_err(|_| "Could not check for updates; saved data remains available offline")?;
    let response = match &update {
        Some(update) => json!({"configured":true,"available":true,"currentVersion":current,
            "version":update.version,"notes":update.body}),
        None => json!({"configured":true,"available":false,"currentVersion":current}),
    };
    *state.0.lock().map_err(|_| "Updater unavailable")? = update;
    Ok(response)
}

#[tauri::command]
async fn update_install(
    app: tauri::AppHandle,
    atlas: State<'_, AtlasState>,
    updates: State<'_, UpdateState>,
) -> Result<(), String> {
    let update = updates
        .0
        .lock()
        .map_err(|_| "Updater unavailable")?
        .clone()
        .ok_or("Check for a signed update before installing")?;
    let service = atlas.0.clone();
    // UI asks the owner to save the open form before invoking this command.
    tauri::async_runtime::spawn_blocking(move || {
        service
            .lock()
            .map_err(|_| "Application service unavailable")?
            .call("backup.export", json!({}))
    })
    .await
    .map_err(|_| "Backup worker interrupted")??;
    let progress = app.clone();
    update.download_and_install(
        move |chunk, total| { let _ = progress.emit("atlas:update-progress", json!({"chunk":chunk,"total":total})); },
        || {},
    ).await.map_err(|_| "Update download, signature verification, or installation failed; no unsigned fallback was used")?;
    app.restart();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(UpdateState::default())
        .manage(CompanionState::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let atlas = Atlas::open(&data_dir).map_err(std::io::Error::other)?;
            let state = AtlasState(Arc::new(Mutex::new(atlas)));
            app.manage(state.clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticks: u32 = 0;
                loop {
                    let service = state.0.clone();
                    let delivered = tauri::async_runtime::spawn_blocking(move || {
                        service
                            .lock()
                            .map_err(|_| "Application service unavailable".to_string())?
                            .call("reminders.due", json!({}))
                    })
                    .await;
                    if let Ok(Ok(Value::Array(reminders))) = delivered {
                        for reminder in reminders {
                            let name = reminder
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("A saved reminder is due");
                            // The persistent inbox remains the delivery record even when the OS declines a notification.
                            let _ = handle
                                .notification()
                                .builder()
                                .title("Birthright: Atlas")
                                .body(name)
                                .show();
                            let _ = handle.emit("atlas:reminder", &reminder);
                        }
                    }
                    if ticks % 2 == 0 {
                        let service = state.0.clone();
                        let synced = tauri::async_runtime::spawn_blocking(move || {
                            let mut service = service
                                .lock()
                                .map_err(|_| "Application service unavailable".to_string())?;
                            service.call("email.auto_sync", json!({}))
                        })
                        .await;
                        if let Ok(Ok(result)) = synced {
                            let _ = handle.emit("atlas:email-synced", result);
                        }
                    }
                    ticks = ticks.wrapping_add(1);
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            atlas_call,
            open_external,
            update_status,
            update_install
        ])
        .build(tauri::generate_context!())
        .expect("Birthright: Atlas could not start its native window")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                atlas_core::ai::cancel();
                if let Ok(mut companion) = app.state::<CompanionState>().0.lock() {
                    companion.stop();
                }
            }
        });
}
