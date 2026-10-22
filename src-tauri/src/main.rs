#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use atlas_core::Atlas;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
mod updates;
use updates::{update_install, update_status, UpdateState};
#[cfg(feature = "update-smoke")]
mod update_smoke;

mod companion;
use companion::CompanionState;

#[derive(Clone)]
struct AtlasState(Arc<Mutex<Atlas>>);

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
            #[cfg(feature = "update-smoke")]
            update_smoke::start(app.handle().clone());
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
