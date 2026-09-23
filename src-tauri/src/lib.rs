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
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WebviewWindow, WindowEvent};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceVisibility {
    Main,
    Compact,
    TrayOnly,
}

#[derive(Debug, Clone)]
struct PresentationState {
    settings: AppSettings,
    visibility: SurfaceVisibility,
}

impl Default for PresentationState {
    fn default() -> Self {
        Self {
            settings: AppSettings::default(),
            visibility: SurfaceVisibility::Main,
        }
    }
}

impl PresentationState {
    fn select(&mut self, mode: PresentationMode) {
        self.visibility = visibility_for_mode(&mode);
        self.settings.presentation_mode = mode;
    }

    fn close_to_tray(&mut self) {
        // Closing hides the selected surface without changing which mode the
        // next explicit surface action uses.
        self.visibility = SurfaceVisibility::TrayOnly;
    }
}

fn visibility_for_mode(mode: &PresentationMode) -> SurfaceVisibility {
    match mode {
        PresentationMode::Main => SurfaceVisibility::Main,
        PresentationMode::Compact => SurfaceVisibility::Compact,
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
    presentation: Mutex<PresentationState>,
    quitting: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WindowAction {
    Minimize,
    ToggleMaximize,
    Close,
}

fn set_visible_surface(app: &tauri::AppHandle, mode: &PresentationMode) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;
    let compact = app
        .get_webview_window("compact")
        .ok_or_else(|| "compact window is unavailable".to_string())?;

    match mode {
        PresentationMode::Main => {
            compact.hide().map_err(|error| error.to_string())?;
            main.unminimize().map_err(|error| error.to_string())?;
            main.show().map_err(|error| error.to_string())?;
            main.set_focus().map_err(|error| error.to_string())?;
        }
        PresentationMode::Compact => {
            main.hide().map_err(|error| error.to_string())?;
            compact.unminimize().map_err(|error| error.to_string())?;
            compact.show().map_err(|error| error.to_string())?;
            compact.set_focus().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn update_presentation_mode(
    mode: PresentationMode,
    state: &AppState,
    app: &tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut presentation = state
        .presentation
        .lock()
        .map_err(|_| "presentation lock poisoned".to_string())?;
    let target_visibility = visibility_for_mode(&mode);
    if presentation.visibility == target_visibility {
        let label = match &mode {
            PresentationMode::Main => "main",
            PresentationMode::Compact => "compact",
        };
        let window = app
            .get_webview_window(label)
            .ok_or_else(|| format!("{label} window is unavailable"))?;
        if window.is_minimized().map_err(|error| error.to_string())? {
            window.unminimize().map_err(|error| error.to_string())?;
        }
        if !window.is_visible().map_err(|error| error.to_string())? {
            window.show().map_err(|error| error.to_string())?;
        }
        window.set_focus().map_err(|error| error.to_string())?;
    } else {
        set_visible_surface(app, &mode)?;
    }
    presentation.select(mode);
    Ok(presentation.settings.clone())
}

fn mark_tray_only(state: &AppState) -> Result<(), String> {
    let mut presentation = state
        .presentation
        .lock()
        .map_err(|_| "presentation lock poisoned".to_string())?;
    presentation.close_to_tray();
    Ok(())
}

fn initialize_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open S4Quota", true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let compact = MenuItem::with_id(app, "compact", "Compact Mode", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit S4Quota", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &separator_one,
            &compact,
            &refresh,
            &separator_two,
            &quit,
        ],
    )?;
    let icon = tauri::image::Image::from_bytes(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/s4quota-tray.png"
    )))?;

    TrayIconBuilder::with_id("s4quota-tray")
        .icon(icon)
        .tooltip("S4Quota")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let state = app.state::<AppState>();
                let _ = update_presentation_mode(PresentationMode::Main, &state, app);
            }
            "compact" => {
                let state = app.state::<AppState>();
                let _ = update_presentation_mode(PresentationMode::Compact, &state, app);
            }
            "refresh" => {
                let handle = app.state::<AppState>().provider_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = handle.send(ProviderCommand::Refresh).await;
                });
            }
            "quit" => request_quit(app.clone()),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn request_quit(app: tauri::AppHandle) {
    let (handle, provider_state) = {
        let state = app.state::<AppState>();
        if state.quitting.swap(true, Ordering::SeqCst) {
            return;
        }
        (state.provider_handle.clone(), state.provider_state.clone())
    };
    finish_after_provider_shutdown(app, handle, provider_state);
}

impl AppState {
    fn snapshot(&self) -> Result<AppSnapshot, String> {
        let provider = self.provider_state.borrow().clone();
        let settings = self
            .presentation
            .lock()
            .map_err(|_| "presentation lock poisoned".to_string())?
            .settings
            .clone();
        Ok(AppSnapshot::from_canonical_provider(provider, settings))
    }
}

fn finish_after_provider_shutdown(
    app: tauri::AppHandle,
    handle: ProviderManagerHandle,
    mut state: watch::Receiver<ProviderState>,
) {
    tauri::async_runtime::spawn(async move {
        let _ = handle.send(ProviderCommand::Shutdown).await;
        let _ = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                if state.changed().await.is_err() {
                    break;
                }
                if matches!(*state.borrow_and_update(), ProviderState::Stopped) {
                    break;
                }
            }
        })
        .await;
        app.exit(0);
    });
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
        .presentation
        .lock()
        .map(|value| value.settings.clone())
        .map_err(|_| "presentation lock poisoned".into())
}

#[tauri::command]
fn update_settings(
    settings: AppSettings,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut presentation = state
        .presentation
        .lock()
        .map_err(|_| "presentation lock poisoned".to_string())?;
    if settings.presentation_mode != presentation.settings.presentation_mode {
        set_visible_surface(&app, &settings.presentation_mode)?;
        presentation.visibility = visibility_for_mode(&settings.presentation_mode);
    }
    presentation.settings = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn set_presentation_mode(
    mode: PresentationMode,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    update_presentation_mode(mode, &state, &app)
}

#[tauri::command]
fn get_window_label(window: WebviewWindow) -> String {
    window.label().to_string()
}

#[tauri::command]
fn window_action(
    action: WindowAction,
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    match action {
        WindowAction::Minimize => window.minimize().map_err(|error| error.to_string()),
        WindowAction::ToggleMaximize => {
            let maximized = window.is_maximized().map_err(|error| error.to_string())?;
            if maximized {
                window.unmaximize().map_err(|error| error.to_string())
            } else {
                window.maximize().map_err(|error| error.to_string())
            }
        }
        WindowAction::Close => {
            window.hide().map_err(|error| error.to_string())?;
            mark_tray_only(&state)
        }
    }
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

    #[test]
    fn close_hides_surface_without_changing_selected_mode() {
        for (mode, expected) in [
            (PresentationMode::Main, SurfaceVisibility::Main),
            (PresentationMode::Compact, SurfaceVisibility::Compact),
        ] {
            let mut presentation = PresentationState::default();
            presentation.select(mode.clone());
            presentation.close_to_tray();

            assert_eq!(presentation.settings.presentation_mode, mode);
            assert_eq!(presentation.visibility, SurfaceVisibility::TrayOnly);
            assert_ne!(presentation.visibility, expected);
        }
    }

    #[test]
    fn explicit_surface_actions_restore_exactly_one_selected_surface() {
        let mut presentation = PresentationState::default();
        presentation.close_to_tray();

        presentation.select(PresentationMode::Compact);
        assert_eq!(
            presentation.settings.presentation_mode,
            PresentationMode::Compact
        );
        assert_eq!(presentation.visibility, SurfaceVisibility::Compact);

        presentation.close_to_tray();
        presentation.select(PresentationMode::Main);
        assert_eq!(
            presentation.settings.presentation_mode,
            PresentationMode::Main
        );
        assert_eq!(presentation.visibility, SurfaceVisibility::Main);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (provider_handle, provider_runtime, publication) = channel();
    let provider_state = publication.state_rx;
    let event_state = provider_state.clone();
    let startup_handle = provider_handle.clone();
    let event_handle = provider_handle.clone();
    let shutdown_started = Arc::new(AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown_started);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let state = app.state::<AppState>();
            let _ = update_presentation_mode(PresentationMode::Main, &state, app);
        }))
        .manage(AppState {
            provider_handle,
            provider_state,
            presentation: Mutex::new(PresentationState::default()),
            quitting: Arc::clone(&shutdown_started),
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<AppState>();
                if !state.quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = mark_tray_only(&state);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_snapshot,
            refresh_provider,
            get_settings,
            update_settings,
            set_presentation_mode,
            get_window_label,
            window_action
        ])
        .setup(move |app| {
            initialize_tray(app.handle())?;
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

    app.run(move |_app_handle, event| match event {
        tauri::RunEvent::Resumed => {
            let _ = event_handle.try_send(ProviderCommand::Refresh);
        }
        tauri::RunEvent::ExitRequested { code: None, .. }
            if !shutdown_flag.load(Ordering::SeqCst) =>
        {
            // Do not prevent operating-system session shutdown/logoff. Windows
            // Job Object kill-on-close remains the final orphan-process guard.
            let _ = event_handle.try_send(ProviderCommand::Shutdown);
        }
        _ => {}
    });
}
