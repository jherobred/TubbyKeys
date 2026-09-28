mod audio;
mod keyboard;
mod keymap;
mod marketplace;
mod overlay;
mod packs;
mod settings;
mod system;
mod tray;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::menu::CheckMenuItem;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_opener::OpenerExt;

use audio::{AudioHandle, Command, OutputStatus, Params};
use keyboard::HookFlags;
use packs::{PackInfo, PackStore, SoundBank, DEFAULT_PACK};
use settings::{Placement, Settings};

const REPO_URL: &str = "https://github.com/jherobred/TubbyKeys";
const BANK_CACHE: usize = 4;

pub(crate) struct AppState {
    settings: Mutex<Settings>,
    store: PackStore,
    audio: AudioHandle,
    hook: Arc<HookFlags>,
    output: Arc<Mutex<OutputStatus>>,
    banks: Mutex<Vec<Arc<SoundBank>>>,
    market: Mutex<Option<marketplace::Index>>,
    tray_toggle: Mutex<Option<CheckMenuItem<Wry>>>,
    flyout_hidden_at: Mutex<Option<Instant>>,
    /// The flyout was just created and should appear once its page has loaded.
    flyout_pending: AtomicBool,
    saver: Mutex<mpsc::Sender<Settings>>,
    settings_path: PathBuf,
}

/// Lock that survives a panicked holder instead of taking the app down.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    settings: Settings,
    packs: Vec<PackInfo>,
    output: OutputStatus,
    version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketItem {
    id: String,
    name: String,
    version: String,
    author: String,
    license: String,
    #[serde(rename = "type")]
    kind: String,
    description: String,
    size: u64,
    installed_version: Option<String>,
}

// ---------------------------------------------------------------- commands

#[tauri::command]
fn get_state(state: State<'_, AppState>) -> Snapshot {
    Snapshot {
        settings: lock(&state.settings).clone(),
        packs: state.store.list(),
        output: lock(&state.output).clone(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

// Commands that can open windows are async: creating a window from a sync
// command runs on the main thread and can deadlock WebView2 on Windows.
#[tauri::command]
async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    apply_settings(&app, &state, settings)
}

#[tauri::command]
fn preview_pack(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let bank = load_bank(&state, &id)?;
    state.audio.send(Command::Preview(bank));
    Ok(())
}

#[tauri::command]
async fn market_list(app: AppHandle) -> Result<Vec<MarketItem>, String> {
    let index = tauri::async_runtime::spawn_blocking(|| {
        marketplace::fetch_index(&marketplace::Https::new())
    })
    .await
    .map_err(|e| e.to_string())??;
    let state = app.state::<AppState>();
    let installed: HashMap<String, String> = state
        .store
        .installed_manifests()
        .into_iter()
        .map(|m| (m.id, m.version))
        .collect();
    let items = index
        .packs
        .iter()
        .map(|e| MarketItem {
            id: e.id.clone(),
            name: e.name.clone(),
            version: e.version.clone(),
            author: e.author.clone(),
            license: e.license.clone(),
            kind: e.kind.clone(),
            description: e.description.clone(),
            size: e.size(),
            installed_version: installed.get(&e.id).cloned(),
        })
        .collect();
    *lock(&state.market) = Some(index);
    Ok(items)
}

#[tauri::command]
async fn market_install(app: AppHandle, id: String) -> Result<Vec<PackInfo>, String> {
    let (entry, dir) = {
        let state = app.state::<AppState>();
        let entry = lock(&state.market)
            .as_ref()
            .and_then(|index| index.packs.iter().find(|e| e.id == id).cloned())
            .ok_or("Refresh the marketplace and try again.")?;
        (entry, state.store.dir().to_path_buf())
    };
    tauri::async_runtime::spawn_blocking(move || {
        marketplace::install(&marketplace::Https::new(), &entry, &dir)
    })
    .await
    .map_err(|e| e.to_string())??;
    let state = app.state::<AppState>();
    lock(&state.banks).retain(|b| b.id != id);
    if lock(&state.settings).pack_id == id {
        // An update to the active pack: reload it.
        let bank = load_bank(&state, &id)?;
        state.audio.send(Command::SetBank(bank));
    }
    Ok(packs_changed(&app, &state))
}

#[tauri::command]
async fn remove_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<PackInfo>, String> {
    marketplace::remove(state.store.dir(), &id)?;
    lock(&state.banks).retain(|b| b.id != id);
    let current = lock(&state.settings).clone();
    if current.pack_id == id {
        apply_settings(
            &app,
            &state,
            Settings {
                pack_id: DEFAULT_PACK.into(),
                ..current
            },
        )?;
    }
    Ok(packs_changed(&app, &state))
}

#[tauri::command]
fn open_packs_folder(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.store.dir();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Only known links can be opened.
#[tauri::command]
fn open_link(app: AppHandle, link: String) -> Result<(), String> {
    let url = match link.as_str() {
        "repo" => REPO_URL.to_string(),
        "submit-pack" => format!("{REPO_URL}/blob/main/marketplace/README.md"),
        "issues" => format!("{REPO_URL}/issues"),
        _ => return Err("unknown link".into()),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn show_settings(app: AppHandle) {
    open_settings_window(&app);
}

/// Drag-to-move for the visualizer: start, or finish and remember the spot.
#[tauri::command]
async fn arrange_overlay(
    app: AppHandle,
    state: State<'_, AppState>,
    active: bool,
) -> Result<Settings, String> {
    let dropped = overlay::arrange(&app, active)?;
    let mut next = lock(&state.settings).clone();
    if let Some(position) = dropped {
        next.visualizer.placement = Placement::Custom;
        next.visualizer.position = Some(position);
    }
    apply_settings(&app, &state, next)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------- state changes

/// Validate and apply new settings. Fallible steps (loading a pack, taking a
/// hotkey) run first so a failure leaves the old settings fully in place.
fn apply_settings(app: &AppHandle, state: &AppState, new: Settings) -> Result<Settings, String> {
    let new = new.sanitized();
    let old = lock(&state.settings).clone();
    let bank = if new.pack_id != old.pack_id {
        Some(load_bank(state, &new.pack_id)?)
    } else {
        None
    };
    if new.hotkey != old.hotkey {
        swap_hotkey(app, &old.hotkey, &new.hotkey)?;
    }
    if let Some(bank) = bank {
        state.audio.send(Command::SetBank(bank));
    }
    *lock(&state.settings) = new.clone();
    apply_params(state, &new);
    if new.visualizer != old.visualizer {
        overlay::sync(app, &new.visualizer);
    }
    if new.enabled != old.enabled {
        tray::refresh(app, state);
    }
    let _ = lock(&state.saver).send(new.clone());
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

fn apply_params(state: &AppState, s: &Settings) {
    let p = &state.audio.params;
    p.enabled.store(s.enabled, Relaxed);
    p.volume.set(s.volume);
    p.tone.set(s.tone);
    p.pitch.set(s.pitch);
    p.randomize.store(s.randomize_pitch, Relaxed);
    p.spatial.store(s.spatial, Relaxed);
    p.width
        .set(s.effective_width(lock(&state.output).headphones));
    p.balance.set(s.balance);
    state.hook.visualizer.store(s.visualizer.enabled, Relaxed);
    state.hook.tray.store(s.tray_pulse && s.enabled, Relaxed);
}

pub(crate) fn toggle_sounds(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut next = lock(&state.settings).clone();
    next.enabled = !next.enabled;
    if let Err(e) = apply_settings(app, &state, next) {
        eprintln!("toggle failed: {e}");
    }
}

fn packs_changed(app: &AppHandle, state: &AppState) -> Vec<PackInfo> {
    let packs = state.store.list();
    let _ = app.emit("packs-changed", &packs);
    packs
}

/// Decoded packs are cached so hover previews and switching stay instant.
fn load_bank(state: &AppState, id: &str) -> Result<Arc<SoundBank>, String> {
    {
        let mut cache = lock(&state.banks);
        if let Some(i) = cache.iter().position(|b| b.id == id) {
            let bank = cache.remove(i);
            cache.push(Arc::clone(&bank));
            return Ok(bank);
        }
    }
    let bank = Arc::new(state.store.load_bank(id)?);
    let mut cache = lock(&state.banks);
    cache.push(Arc::clone(&bank));
    if cache.len() > BANK_CACHE {
        cache.remove(0);
    }
    Ok(bank)
}

fn swap_hotkey(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    if !new.is_empty() {
        new.parse::<Shortcut>()
            .map_err(|e| format!("Invalid shortcut: {e}"))?;
    }
    let shortcuts = app.global_shortcut();
    if !old.is_empty() {
        let _ = shortcuts.unregister(old);
    }
    if new.is_empty() {
        return Ok(());
    }
    if let Err(e) = shortcuts.register(new) {
        if !old.is_empty() {
            let _ = shortcuts.register(old);
        }
        return Err(format!("{new} is not available: {e}"));
    }
    Ok(())
}

fn on_output_status(app: &AppHandle, output: &Mutex<OutputStatus>, status: OutputStatus) {
    *lock(output) = status.clone();
    if let Some(state) = app.try_state::<AppState>() {
        let settings = lock(&state.settings).clone();
        state
            .audio
            .params
            .width
            .set(settings.effective_width(status.headphones));
    }
    let _ = app.emit("output-changed", &status);
}

// ---------------------------------------------------------------- windows

pub(crate) fn open_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("TubbyKeys")
        .inner_size(940.0, 680.0)
        .min_inner_size(780.0, 560.0)
        .center()
        .build();
    if let Err(e) = built {
        eprintln!("could not open settings: {e}");
    }
}

/// Poll what Windows does not push to us: the taskbar theme (for the tray
/// icon) and full-screen apps (the overlay steps aside for them).
fn watch_system(app: AppHandle) {
    std::thread::Builder::new()
        .name("tubbykeys-system".into())
        .spawn(move || {
            let mut tick = 0u64;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                tick += 1;
                let Some(state) = app.try_state::<AppState>() else {
                    continue;
                };
                if tick % 3 == 0 && tray::update_theme() {
                    tray::refresh(&app, &state);
                }
                let hide = {
                    let v = &lock(&state.settings).visualizer;
                    v.enabled && v.hide_in_fullscreen
                };
                overlay::set_suppressed(&app, hide && system::fullscreen_app_active());
            }
        })
        .expect("failed to start the system watcher");
}

/// Coalesce rapid changes (slider drags) into one write.
fn run_saver(path: PathBuf) -> mpsc::Sender<Settings> {
    let (tx, rx) = mpsc::channel::<Settings>();
    std::thread::Builder::new()
        .name("tubbykeys-saver".into())
        .spawn(move || {
            while let Ok(mut latest) = rx.recv() {
                while let Ok(newer) = rx.recv_timeout(Duration::from_millis(400)) {
                    latest = newer;
                }
                if let Err(e) = settings::save(&path, &latest) {
                    eprintln!("could not save settings: {e}");
                }
            }
        })
        .expect("failed to start the settings thread");
    tx
}

// ---------------------------------------------------------------- startup

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            open_settings_window(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_sounds(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let store = PackStore::new(app.path().app_local_data_dir()?.join("packs"));
            let (mut settings, first_run) = settings::load(&settings_path);
            if !store.exists(&settings.pack_id) {
                settings.pack_id = DEFAULT_PACK.into();
            }

            let params = Arc::new(Params::default());
            let output = Arc::new(Mutex::new(OutputStatus::default()));
            let (status_app, status_out) = (handle.clone(), Arc::clone(&output));
            let (keys, audio) = audio::start(Arc::clone(&params), move |status| {
                on_output_status(&status_app, &status_out, status)
            });
            let hook = Arc::new(HookFlags::default());
            let (pulse_tx, pulse_rx) = mpsc::sync_channel(64);
            keyboard::start(keys, params, audio.waker(), Arc::clone(&hook), pulse_tx);

            let state = AppState {
                settings: Mutex::new(settings.clone()),
                store,
                audio,
                hook,
                output,
                banks: Mutex::new(Vec::new()),
                market: Mutex::new(None),
                tray_toggle: Mutex::new(None),
                flyout_hidden_at: Mutex::new(None),
                flyout_pending: AtomicBool::new(false),
                saver: Mutex::new(run_saver(settings_path.clone())),
                settings_path,
            };
            match load_bank(&state, &settings.pack_id) {
                Ok(bank) => state.audio.send(Command::SetBank(bank)),
                Err(e) => eprintln!("could not load {}: {e}", settings.pack_id),
            }
            apply_params(&state, &settings);
            app.manage(state);

            let toggle = tray::build(&handle, settings.enabled)?;
            *lock(&handle.state::<AppState>().tray_toggle) = Some(toggle);
            if !settings.hotkey.is_empty() {
                if let Err(e) = handle.global_shortcut().register(settings.hotkey.as_str()) {
                    eprintln!("hotkey {} unavailable: {e}", settings.hotkey);
                }
            }
            overlay::run_pulse_forwarder(handle.clone(), pulse_rx);
            watch_system(handle.clone());
            overlay::sync(&handle, &settings.visualizer);
            if first_run {
                let _ = lock(&handle.state::<AppState>().saver).send(settings);
                open_settings_window(&handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            update_settings,
            preview_pack,
            market_list,
            market_install,
            remove_pack,
            open_packs_folder,
            open_link,
            show_settings,
            arrange_overlay,
            quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build TubbyKeys")
        .run(|app, event| match event {
            // Closing the last window keeps TubbyKeys running in the tray.
            RunEvent::ExitRequested {
                api, code: None, ..
            } => api.prevent_exit(),
            RunEvent::Exit => {
                // Flush any change still waiting in the saver.
                let state = app.state::<AppState>();
                let settings = lock(&state.settings).clone();
                let _ = settings::save(&state.settings_path, &settings);
            }
            _ => {}
        });
}
