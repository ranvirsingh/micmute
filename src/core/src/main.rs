#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod overlay;

use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{
    Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum BadgePosition {
    N,
    Ne,
    E,
    #[default]
    Se,
    S,
    Sw,
    W,
    Nw,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    shortcut: String,
    border_enabled: bool,
    badge_position: BadgePosition,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            shortcut: "CommandOrControl+Shift+M".into(),
            border_enabled: true,
            badge_position: BadgePosition::Se,
        }
    }
}
#[derive(Clone, Debug, Serialize, PartialEq)]
struct Status {
    microphone: Option<audio::Microphone>,
    error: Option<String>,
    shortcut: String,
    shortcut_error: Option<String>,
    border_enabled: bool,
    badge_position: BadgePosition,
    preview: bool,
}
impl Status {
    fn overlay_visible(&self) -> bool {
        self.preview
            || (self.border_enabled && self.microphone.as_ref().is_some_and(|mic| mic.muted))
    }
}

struct Runtime {
    preferences: Preferences,
    shortcut_error: Option<String>,
    operation_error: Option<String>,
    pressed: bool,
    recording: bool,
    preview_until: Option<Instant>,
    last: Option<Status>,
    screens: Option<Vec<overlay::Screen>>,
}
type AppState = Mutex<Runtime>;

fn prefs_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("settings.json"))
        .map_err(|e| e.to_string())
}
fn write_preferences(app: &tauri::AppHandle, preferences: &Preferences) -> Result<(), String> {
    let path = prefs_path(app)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(preferences).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn snapshot(runtime: &Runtime) -> Status {
    let (microphone, error) = match audio::current() {
        Ok(mic) => (Some(mic), runtime.operation_error.clone()),
        Err(error) => (None, Some(error)),
    };
    Status {
        microphone,
        error,
        shortcut: runtime.preferences.shortcut.clone(),
        shortcut_error: runtime.shortcut_error.clone(),
        border_enabled: runtime.preferences.border_enabled,
        badge_position: runtime.preferences.badge_position,
        preview: runtime
            .preview_until
            .is_some_and(|time| time > Instant::now()),
    }
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn overlay_windows(
    app: &tauri::AppHandle,
    runtime: &mut Runtime,
    visible: bool,
) -> Result<(), String> {
    if !visible {
        for (label, window) in app.webview_windows() {
            if label.starts_with("border-") {
                window.hide().map_err(|e| e.to_string())?;
            }
        }
        return Ok(());
    }
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let screens: Vec<_> = monitors
        .iter()
        .map(|m| {
            (
                m.position().x,
                m.position().y,
                m.size().width,
                m.size().height,
            )
        })
        .collect();
    overlay::sync(&mut runtime.screens, screens, &mut TauriOverlays(app))
}

struct TauriOverlays<'a>(&'a tauri::AppHandle);
impl TauriOverlays<'_> {
    fn window(&self, index: usize) -> Result<tauri::WebviewWindow, String> {
        self.0
            .get_webview_window(&format!("border-{index}"))
            .ok_or_else(|| "The mute indicator window is unavailable.".to_string())
    }
}
impl overlay::Windows for TauriOverlays<'_> {
    fn reset(&mut self) -> Result<(), String> {
        for (label, window) in self.0.webview_windows() {
            if label.starts_with("border-") {
                window.destroy().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    fn exists(&self, index: usize) -> bool {
        self.0
            .get_webview_window(&format!("border-{index}"))
            .is_some()
    }
    fn create(&mut self, index: usize) -> Result<(), String> {
        let app = self.0;
        let label = format!("border-{index}");
        let _window =
            WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html?overlay=1".into()))
                .title("MicMute Indicator")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .visible_on_all_workspaces(true)
                .skip_taskbar(true)
                .resizable(false)
                .focusable(false)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        if let Ok(pointer) = _window.ns_window() {
            // SAFETY: Called on Tauri's main thread with the live NSWindow.
            unsafe {
                let native = &*(pointer as *const objc2::runtime::AnyObject);
                let _: () = objc2::msg_send![native, setLevel: 25_isize];
                // All Spaces, stationary, excluded from cycling, full-screen auxiliary.
                let _: () =
                    objc2::msg_send![native, setCollectionBehavior: (1_usize | 16 | 64 | 256)];
                let _: () = objc2::msg_send![native, setHidesOnDeactivate: false];
            }
        }
        Ok(())
    }
    fn configure(&mut self, index: usize, screen: overlay::Screen) -> Result<(), String> {
        let window = self.window(index)?;
        window
            .set_ignore_cursor_events(true)
            .map_err(|e| e.to_string())?;
        window
            .set_position(PhysicalPosition::new(screen.0, screen.1))
            .map_err(|e| e.to_string())?;
        window
            .set_size(PhysicalSize::new(screen.2, screen.3))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn show(&mut self, index: usize) -> Result<(), String> {
        let window = self.window(index)?;
        if !window.is_visible().map_err(|e| e.to_string())? {
            window.show().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn refresh(app: &tauri::AppHandle) -> Status {
    let state = app.state::<AppState>();
    let mut runtime = state.lock().unwrap();
    let mut status = snapshot(&runtime);
    let visible = status.overlay_visible();
    if let Err(error) = overlay_windows(app, &mut runtime, visible) {
        status.error = Some(format!("Cannot display the mute border: {error}"));
        // A border failure must remain visible in the settings and menu bar.
    }
    if runtime.last.as_ref() != Some(&status) {
        let muted = status.microphone.as_ref().is_some_and(|mic| mic.muted);
        if let Some(tray) = app.tray_by_id("mic") {
            let (icon, template) = tray_image(status.microphone.as_ref().map(|mic| mic.muted));
            let _ = tray.set_icon_with_as_template(Some(icon), template);
            let tooltip = status.error.as_deref().unwrap_or(if muted {
                "System input microphone muted"
            } else {
                "System input microphone unmuted"
            });
            let _ = tray.set_tooltip(Some(tooltip));
        }
        eprintln!("status: {}", serde_json::to_string(&status).unwrap());
        let _ = app.emit("status", &status);
        runtime.last = Some(status.clone());
    }
    status
}

fn tray_image(muted: Option<bool>) -> (tauri::image::Image<'static>, bool) {
    let (bytes, template): (&[u8], bool) = match muted {
        Some(true) => (include_bytes!("../icons/tray-muted.png"), false),
        Some(false) => (include_bytes!("../icons/tray-live.png"), true),
        None => (include_bytes!("../icons/tray-unknown.png"), false),
    };
    (
        tauri::image::Image::from_bytes(bytes).expect("Bundled microphone icon is invalid"),
        template,
    )
}

#[tauri::command]
fn get_status(app: tauri::AppHandle) -> Status {
    refresh(&app)
}

#[tauri::command]
fn toggle_mute(app: tauri::AppHandle) -> Result<Status, String> {
    let result = audio::current().and_then(|mic| audio::set_mute(mic.id, !mic.muted));
    app.state::<AppState>().lock().unwrap().operation_error = result.as_ref().err().cloned();
    let status = refresh(&app);
    result.map(|_| status)
}

fn parse_shortcut(value: &str) -> Result<Shortcut, String> {
    if value.len() > 100 || !value.contains('+') {
        return Err("Use a modifier with a key, such as CommandOrControl+Shift+M.".into());
    }
    let shortcut = value.parse::<Shortcut>().map_err(|_| {
        "That shortcut is not valid. Use a modifier with a letter, number, or function key."
            .to_string()
    })?;
    if !shortcut
        .mods
        .intersects(Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT)
    {
        return Err("Include Command, Control, or Option in the shortcut.".into());
    }
    Ok(shortcut)
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    shortcut: String,
    border_enabled: bool,
    state: State<AppState>,
) -> Result<Status, String> {
    let shortcut = shortcut.trim().to_owned();
    let new_key = parse_shortcut(&shortcut)?;
    let mut runtime = state.lock().unwrap();
    let old = runtime.preferences.clone();
    let old_key = parse_shortcut(&old.shortcut).ok();
    let changed = old_key.as_ref() != Some(&new_key) || runtime.shortcut_error.is_some();
    if changed {
        // Register the replacement before removing the working shortcut.
        app.global_shortcut().register(new_key).map_err(|e| {
            format!("Cannot register that shortcut. Choose another combination. ({e})")
        })?;
    }
    let updated = Preferences {
        shortcut,
        border_enabled,
        badge_position: runtime.preferences.badge_position,
    };
    if let Err(error) = write_preferences(&app, &updated) {
        if changed {
            let _ = app.global_shortcut().unregister(new_key);
        }
        return Err(format!("Cannot save settings: {error}"));
    }
    if changed && runtime.shortcut_error.is_none() {
        if let Some(old_key) = old_key {
            let _ = app.global_shortcut().unregister(old_key);
        }
    }
    runtime.preferences = updated;
    runtime.shortcut_error = None;
    runtime.pressed = false;
    drop(runtime);
    Ok(refresh(&app))
}

#[tauri::command]
fn set_badge_position(
    app: tauri::AppHandle,
    position: BadgePosition,
    state: State<AppState>,
) -> Result<Status, String> {
    let mut runtime = state.lock().unwrap();
    let mut updated = runtime.preferences.clone();
    updated.badge_position = position;
    write_preferences(&app, &updated)
        .map_err(|error| format!("Cannot save label position: {error}"))?;
    runtime.preferences = updated;
    runtime.preview_until = Some(Instant::now() + Duration::from_secs(3));
    drop(runtime);
    Ok(refresh(&app))
}

#[tauri::command]
fn preview_border(app: tauri::AppHandle) -> Status {
    app.state::<AppState>().lock().unwrap().preview_until =
        Some(Instant::now() + Duration::from_secs(3));
    refresh(&app)
}
#[tauri::command]
fn hide_settings(app: tauri::AppHandle) {
    app.state::<AppState>().lock().unwrap().recording = false;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
#[tauri::command]
fn set_recording(recording: bool, state: State<AppState>) {
    state.lock().unwrap().recording = recording;
}
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    if std::env::args().any(|arg| arg == "--test-mic") {
        if let Err(error) = audio::test_round_trip() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--probe") {
        match audio::current() {
            Ok(mic) => println!("{}", serde_json::to_string(&mic).unwrap()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_settings(app)
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    let state = app.state::<AppState>();
                    let mut runtime = state.lock().unwrap();
                    if parse_shortcut(&runtime.preferences.shortcut).ok().as_ref() != Some(shortcut)
                    {
                        return;
                    }
                    if event.state() == ShortcutState::Released {
                        runtime.pressed = false;
                        return;
                    }
                    if runtime.recording {
                        return;
                    }
                    if runtime.pressed {
                        return;
                    }
                    runtime.pressed = true;
                    drop(runtime);
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        let _ = toggle_mute(handle);
                    });
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let preferences: Preferences = prefs_path(app.handle())
                .ok()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            let error = parse_shortcut(&preferences.shortcut)
                .and_then(|shortcut| {
                    app.global_shortcut()
                        .register(shortcut)
                        .map_err(|e| format!("Shortcut unavailable: {e}"))
                })
                .err();
            app.manage(Mutex::new(Runtime {
                preferences,
                shortcut_error: error,
                operation_error: None,
                pressed: false,
                recording: false,
                preview_until: None,
                last: None,
                screens: None,
            }));
            let toggle =
                tauri::menu::MenuItem::with_id(app, "toggle", "Mute / Unmute", true, None::<&str>)?;
            let settings =
                tauri::menu::MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = tauri::menu::MenuItem::with_id(
                app,
                "quit",
                "Quit (keep microphone state)",
                true,
                None::<&str>,
            )?;
            let menu = tauri::menu::Menu::with_items(app, &[&toggle, &settings, &quit])?;
            let (icon, template) = tray_image(None);
            tauri::tray::TrayIconBuilder::with_id("mic")
                .icon(icon)
                .icon_as_template(template)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => {
                        let _ = toggle_mute(app.clone());
                    }
                    "settings" => show_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(300));
                let app = handle.clone();
                if handle
                    .run_on_main_thread(move || {
                        refresh(&app);
                    })
                    .is_err()
                {
                    break;
                }
            });
            refresh(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    window
                        .app_handle()
                        .state::<AppState>()
                        .lock()
                        .unwrap()
                        .recording = false;
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            toggle_mute,
            save_settings,
            set_badge_position,
            preview_border,
            hide_settings,
            set_recording,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("MicMute could not start");
}

#[cfg(test)]
mod tests;
