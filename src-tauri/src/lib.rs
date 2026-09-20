mod codex_provider;
mod domain;
mod json_rpc;
#[allow(dead_code)]
mod mock_provider;
mod provider_manager;
mod sanitize;
#[cfg(windows)]
mod windows_job;

use codex_provider::CodexProvider;
use domain::{ProviderState, QuotaSnapshot};
use provider_manager::{channel, ProviderCommand, ProviderManagerConfig, ProviderManagerHandle};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PresentationMode {
    Main,
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub presentation_mode: PresentationMode,
    pub always_on_top_compact: bool,
    pub autostart: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            presentation_mode: PresentationMode::Main,
            always_on_top_compact: true,
            autostart: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub provider: ProviderState,
    pub quota: Option<QuotaSnapshot>,
    pub settings: AppSettings,
}

impl AppSnapshot {
    fn from_canonical_provider(provider: ProviderState, settings: AppSettings) -> Self {
        let quota = provider.snapshot().cloned();
        Self {
            provider,
            quota,
            settings,
        }
    }
}

pub struct AppState {
    provider_handle: ProviderManagerHandle,
    provider_state: watch::Receiver<ProviderState>,
    settings: Mutex<AppSettings>,
}

impl AppState {
    fn snapshot(&self) -> Result<AppSnapshot, String> {
        let provider = self.provider_state.borrow().clone();
        let settings = self
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?
            .clone();
        Ok(AppSnapshot::from_canonical_provider(provider, settings))
    }
}

#[tauri::command]
fn get_app_snapshot(state: tauri::State<'_, AppState>) -> Result<AppSnapshot, String> {
    state.snapshot()
}

#[tauri::command]
async fn refresh_provider(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .provider_handle
        .send(ProviderCommand::Refresh)
        .await
        .map_err(|_| "provider manager is unavailable".into())
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    state
        .settings
        .lock()
        .map(|value| value.clone())
        .map_err(|_| "settings lock poisoned".into())
}

#[tauri::command]
fn update_settings(
    settings: AppSettings,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, String> {
    let mut value = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?;
    *value = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn set_presentation_mode(
    mode: PresentationMode,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, String> {
    let mut value = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?;
    value.presentation_mode = mode;
    Ok(value.clone())
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::mock_provider::MockProvider;

    fn app_snapshot() -> AppSnapshot {
        AppSnapshot::from_canonical_provider(
            ProviderState::Ready {
                snapshot: MockProvider::snapshot(),
            },
            AppSettings::default(),
        )
    }

    #[test]
    fn app_snapshot_serializes_with_stable_camel_case_contract() {
        let json = serde_json::to_value(app_snapshot()).unwrap();
        assert_eq!(json["provider"]["status"], "ready");
        assert_eq!(json["provider"]["snapshot"]["providerId"], "mock");
        assert_eq!(json["settings"]["presentationMode"], "main");
        assert!(json["quota"]["windows"][0]["durationMinutes"].is_number());
    }

    #[test]
    fn presentation_settings_round_trip() {
        let settings = AppSettings {
            presentation_mode: PresentationMode::Compact,
            always_on_top_compact: true,
            autostart: false,
        };
        let encoded = serde_json::to_string(&settings).unwrap();
        let decoded: AppSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, settings);
        assert!(encoded.contains("presentationMode"));
    }

    #[test]
    fn quota_is_derived_from_canonical_provider_state() {
        let app = app_snapshot();
        assert_eq!(app.quota, app.provider.snapshot().cloned());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (provider_handle, provider_runtime, publication) = channel();
    let provider_state = publication.state_rx;
    let event_state = provider_state.clone();
    let shutdown_state = provider_state.clone();
    let startup_handle = provider_handle.clone();
    let event_handle = provider_handle.clone();
    let shutdown_started = Arc::new(AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown_started);

    let app = tauri::Builder::default()
        .manage(AppState {
            provider_handle,
            provider_state,
            settings: Mutex::new(AppSettings::default()),
        })
        .invoke_handler(tauri::generate_handler![
            get_app_snapshot,
            refresh_provider,
            get_settings,
            update_settings,
            set_presentation_mode
        ])
        .setup(move |app| {
            tauri::async_runtime::spawn(
                provider_runtime.run(CodexProvider::default(), ProviderManagerConfig::default()),
            );
            startup_handle
                .try_send(ProviderCommand::Start)
                .map_err(|_| std::io::Error::other("failed to start provider manager"))?;

            let app_handle = app.handle().clone();
            let mut state = event_state;
            tauri::async_runtime::spawn(async move {
                while state.changed().await.is_ok() {
                    let canonical = state.borrow().clone();
                    let _ = app_handle.emit("provider-state", canonical);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |app_handle, event| match event {
        tauri::RunEvent::Resumed => {
            let _ = event_handle.try_send(ProviderCommand::Refresh);
        }
        tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } if !shutdown_flag.swap(true, Ordering::SeqCst) => {
            api.prevent_exit();
            let handle = event_handle.clone();
            let app_handle = app_handle.clone();
            let mut state = shutdown_state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = handle.send(ProviderCommand::Shutdown).await;
                let _ = tokio::time::timeout(Duration::from_secs(4), async {
                    loop {
                        if matches!(*state.borrow(), ProviderState::Stopped) {
                            break;
                        }
                        if state.changed().await.is_err() {
                            break;
                        }
                    }
                })
                .await;
                app_handle.exit(0);
            });
        }
        _ => {}
    });
}
