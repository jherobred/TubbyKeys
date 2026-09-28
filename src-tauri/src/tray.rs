//! Tray icon and its pop-up. The icon follows the taskbar theme and
//! squishes with each key press. Left click opens the pop-up panel with a
//! spring animation.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent, Wry,
};

use crate::{lock, open_settings_window, system, toggle_sounds, AppState};

/// Pop-up window size. The panel inside is smaller; the transparent margin
/// holds its shadow and leaves room for the slide-in.
const FLYOUT_SIZE: (f64, f64) = (360.0, 600.0);
/// Length of the pop-up's closing animation before the window hides.
const FLYOUT_CLOSE: Duration = Duration::from_millis(170);

static LIGHT_TASKBAR: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    On,
    Off,
    Pressed,
}

fn icon(frame: Frame) -> Image<'static> {
    let bytes: &'static [u8] = match (LIGHT_TASKBAR.load(Relaxed), frame) {
        (false, Frame::On) => include_bytes!("../icons/tray/dark-on.png"),
        (false, Frame::Off) => include_bytes!("../icons/tray/dark-off.png"),
        (false, Frame::Pressed) => include_bytes!("../icons/tray/dark-press.png"),
        (true, Frame::On) => include_bytes!("../icons/tray/light-on.png"),
        (true, Frame::Off) => include_bytes!("../icons/tray/light-off.png"),
        (true, Frame::Pressed) => include_bytes!("../icons/tray/light-press.png"),
    };
    Image::from_bytes(bytes).expect("tray icons are valid PNGs")
}

pub fn show_frame(app: &AppHandle, frame: Frame) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(icon(frame)));
    }
}

pub fn build(app: &AppHandle, enabled: bool) -> tauri::Result<CheckMenuItem<Wry>> {
    LIGHT_TASKBAR.store(system::taskbar_is_light(), Relaxed);
    let toggle = CheckMenuItem::with_id(app, "toggle", "Sounds on", true, enabled, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TubbyKeys", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &separator, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(icon(if enabled { Frame::On } else { Frame::Off }))
        .tooltip("TubbyKeys")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_sounds(app),
            "settings" => open_settings_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_flyout(tray.app_handle(), position);
            }
        })
        .build(app)?;
    Ok(toggle)
}

/// Sync icon, tooltip and menu check with the current settings.
pub fn refresh(app: &AppHandle, state: &AppState) {
    let enabled = lock(&state.settings).enabled;
    show_frame(app, if enabled { Frame::On } else { Frame::Off });
    if let Some(tray) = app.tray_by_id("main") {
        let tip = if enabled {
            "TubbyKeys"
        } else {
            "TubbyKeys (muted)"
        };
        let _ = tray.set_tooltip(Some(tip));
    }
    if let Some(toggle) = lock(&state.tray_toggle).as_ref() {
        let _ = toggle.set_checked(enabled);
    }
}

/// Re-read the taskbar theme. Returns true when it changed.
pub fn update_theme() -> bool {
    let light = system::taskbar_is_light();
    LIGHT_TASKBAR.swap(light, Relaxed) != light
}

// ---------------------------------------------------------------- pop-up

fn toggle_flyout(app: &AppHandle, click: PhysicalPosition<f64>) {
    let state = app.state::<AppState>();
    // Clicking the icon while the pop-up is open first blurs (closes) it.
    // Don't reopen it on that same click.
    if lock(&state.flyout_hidden_at).is_some_and(|t| t.elapsed() < Duration::from_millis(350)) {
        return;
    }
    let Some(window) = app.get_webview_window("flyout") else {
        // First open: show it once the page has loaded. Shown any earlier, it
        // loses focus to WebView2 start-up and the blur handler closes it.
        match create_flyout(app) {
            Ok(window) => {
                place_near(app, &window, click);
                state.flyout_pending.store(true, Relaxed);
            }
            Err(e) => eprintln!("could not open the tray menu: {e}"),
        }
        return;
    };
    if window.is_visible().unwrap_or(false) {
        close_flyout(app, &window);
        return;
    }
    place_near(app, &window, click);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to("flyout", "flyout-open", ());
}

/// Play the closing animation, then hide.
fn close_flyout(app: &AppHandle, window: &WebviewWindow) {
    *lock(&app.state::<AppState>().flyout_hidden_at) = Some(Instant::now());
    let _ = app.emit_to("flyout", "flyout-close", ());
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FLYOUT_CLOSE);
        if !window.is_focused().unwrap_or(false) {
            let _ = window.hide();
        }
    });
}

fn create_flyout(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, "flyout", WebviewUrl::App("index.html".into()))
        .title("TubbyKeys")
        .inner_size(FLYOUT_SIZE.0, FLYOUT_SIZE.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished
                && window
                    .state::<AppState>()
                    .flyout_pending
                    .swap(false, Relaxed)
            {
                let _ = window.show();
                let _ = window.set_focus();
            }
        })
        .build()?;
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        system::disable_transitions(hwnd);
    }
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if let Some(flyout) = handle.get_webview_window("flyout") {
                close_flyout(&handle, &flyout);
            }
        }
    });
    Ok(window)
}

/// Rest the pop-up on the taskbar edge nearest the click, inside the work area.
fn place_near(app: &AppHandle, window: &WebviewWindow, click: PhysicalPosition<f64>) {
    let monitor = app
        .monitor_from_point(click.x, click.y)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else {
        return;
    };
    let area = monitor.work_area();
    let (w, h) = (size.width as i32, size.height as i32);
    let (left, top) = (area.position.x, area.position.y);
    let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);
    let x = (click.x as i32 - w / 2).clamp(left, (right - w).max(left));
    let y = if click.y as i32 > (top + bottom) / 2 {
        bottom - h
    } else {
        top
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
}
