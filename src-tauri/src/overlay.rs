//! The visualizer: a click-through, always-on-top window that never takes
//! focus. It comes in three styles, can be dragged anywhere, steps aside
//! for full-screen apps, and stays out of screenshots and screen shares.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::keyboard::KeyPulse;
use crate::settings::{Placement, Style, Visualizer};
use crate::tray::{self, Frame};
use crate::{lock, system, AppState};

const LABEL: &str = "overlay";
/// How long the tray icon stays squished after the last key press.
const TRAY_PRESS: Duration = Duration::from_millis(110);
/// Random placement hops at most this often.
const HOP_EVERY: Duration = Duration::from_millis(180);

/// True while a full-screen app has the overlay hidden.
static SUPPRESSED: AtomicBool = AtomicBool::new(false);

/// Logical window size for a style. The wave's width follows the monitor.
fn logical_size(v: &Visualizer) -> (f64, f64) {
    let scale = v.size.scale();
    match v.style {
        Style::Keyboard => (380.0 * scale, 170.0 * scale),
        Style::Pill => (220.0 * scale, 96.0 * scale),
        Style::Wave => (0.0, 72.0),
    }
}

/// Create, update or close the overlay to match the settings.
pub fn sync(app: &AppHandle, v: &Visualizer) {
    let existing = app.get_webview_window(LABEL);
    if !v.enabled {
        if let Some(window) = existing {
            let _ = window.destroy();
        }
        return;
    }
    let window = match existing {
        Some(window) => window,
        None => match create(app, v) {
            Ok(window) => window,
            Err(e) => {
                eprintln!("could not open the visualizer: {e}");
                return;
            }
        },
    };
    let _ = window.set_content_protected(v.hide_from_capture);
    place(&window, v);
}

fn create(app: &AppHandle, v: &Visualizer) -> tauri::Result<WebviewWindow> {
    let (width, height) = logical_size(v);
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("TubbyKeys visualizer")
        .inner_size(width.max(1.0), height)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .content_protected(v.hide_from_capture)
        .visible(false)
        .build()?;
    window.set_ignore_cursor_events(true)?;
    if !SUPPRESSED.load(Relaxed) {
        // Built with focused(false), so tao shows it without taking focus.
        window.show()?;
    }
    finish_styles(app, &window);
    Ok(window)
}

/// tao rewrites the extended window style whenever one of its flags changes,
/// so reapply ours afterwards. Queued behind any pending window calls.
fn finish_styles(app: &AppHandle, window: &WebviewWindow) {
    #[cfg(windows)]
    {
        let window = window.clone();
        let _ = app.run_on_main_thread(move || {
            if let Ok(hwnd) = window.hwnd() {
                system::hide_from_alt_tab(hwnd);
                system::disable_transitions(hwnd);
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (app, window);
}

fn place(window: &WebviewWindow, v: &Visualizer) {
    // A dragged-to spot restores on whichever monitor holds it.
    let saved = match (v.placement, v.position) {
        (Placement::Custom, Some([x, y])) => Some((x as i32, y as i32)),
        _ => None,
    };
    let holding = saved.and_then(|(x, y)| {
        window.available_monitors().ok()?.into_iter().find(|m| {
            let a = m.work_area();
            (a.position.x..a.position.x + a.size.width as i32).contains(&x)
                && (a.position.y..a.position.y + a.size.height as i32).contains(&y)
        })
    });
    let custom = holding.is_some();
    let Some(monitor) = holding.or_else(|| window.primary_monitor().ok().flatten()) else {
        return;
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let (width, height) = logical_size(v);
    let size = if v.style == Style::Wave {
        PhysicalSize::new(area.size.width, (height * scale) as u32)
    } else {
        PhysicalSize::new((width * scale) as u32, (height * scale) as u32)
    };
    let _ = window.set_size(size);
    let free_x = area.size.width.saturating_sub(size.width) as i32;
    let free_y = area.size.height.saturating_sub(size.height) as i32;
    let margin = if v.style == Style::Wave {
        0
    } else {
        (12.0 * scale) as i32
    };
    let (x, y) = match (v.placement, saved, custom) {
        (Placement::Custom, Some((x, y)), true) => (
            (x - area.position.x).clamp(0, free_x),
            (y - area.position.y).clamp(0, free_y),
        ),
        (Placement::Top, _, _) => (free_x / 2, margin),
        (Placement::Random, _, _) => {
            let r = random();
            (
                (r % free_x.max(1) as u32) as i32,
                ((r >> 12) % free_y.max(1) as u32) as i32,
            )
        }
        _ => (free_x / 2, free_y - margin),
    };
    let _ = window.set_position(PhysicalPosition::new(
        area.position.x + x,
        area.position.y + y,
    ));
}

/// Start or finish drag-to-move. Finishing returns the screen position the
/// overlay was dropped at.
pub fn arrange(app: &AppHandle, active: bool) -> Result<Option<[f64; 2]>, String> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or("Turn on the visualizer first.")?;
    let err = |e: tauri::Error| e.to_string();
    let dropped = if active {
        None
    } else {
        let pos = window.outer_position().map_err(err)?;
        Some([pos.x as f64, pos.y as f64])
    };
    window.set_ignore_cursor_events(!active).map_err(err)?;
    finish_styles(app, &window);
    let _ = app.emit("overlay-arrange", active);
    Ok(dropped)
}

/// Hide the overlay while a full-screen app runs, show it again afterwards.
pub fn set_suppressed(app: &AppHandle, suppressed: bool) {
    if SUPPRESSED.swap(suppressed, Relaxed) == suppressed {
        return;
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        if suppressed {
            let _ = window.hide();
        } else {
            let _ = window.show();
            finish_styles(app, &window);
        }
    }
}

fn random() -> u32 {
    static STATE: AtomicU32 = AtomicU32::new(0x9E37_79B9);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut x = (STATE.load(Relaxed) ^ nanos) | 1;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    STATE.store(x, Relaxed);
    x
}

/// Deliver key pulses to the overlay and squish the tray icon. Random
/// placement hops around the screen as you type.
pub fn run_pulse_forwarder(app: AppHandle, pulses: Receiver<KeyPulse>) {
    std::thread::Builder::new()
        .name("tubbykeys-visualizer".into())
        .spawn(move || {
            let mut last_hop = Instant::now();
            let mut release_at: Option<Instant> = None;
            loop {
                let wait = release_at.map_or(Duration::from_secs(3600), |t| {
                    t.saturating_duration_since(Instant::now())
                });
                let pulse = match pulses.recv_timeout(wait) {
                    Ok(pulse) => pulse,
                    Err(RecvTimeoutError::Timeout) => {
                        release_at = None;
                        let enabled = app
                            .try_state::<AppState>()
                            .is_some_and(|s| lock(&s.settings).enabled);
                        tray::show_frame(&app, if enabled { Frame::On } else { Frame::Off });
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                let Some(state) = app.try_state::<AppState>() else {
                    continue;
                };
                let (v, tray_pulse) = {
                    let s = lock(&state.settings);
                    (s.visualizer.clone(), s.tray_pulse && s.enabled)
                };
                if tray_pulse {
                    if release_at.is_none() {
                        tray::show_frame(&app, Frame::Pressed);
                    }
                    release_at = Some(Instant::now() + TRAY_PRESS);
                }
                if !v.enabled || SUPPRESSED.load(Relaxed) {
                    continue;
                }
                let Some(window) = app.get_webview_window(LABEL) else {
                    continue;
                };
                let _ = app.emit_to(LABEL, "key-pulse", pulse);
                if v.placement == Placement::Random && last_hop.elapsed() >= HOP_EVERY {
                    last_hop = Instant::now();
                    place(&window, &v);
                }
            }
        })
        .expect("failed to start the visualizer thread");
}
