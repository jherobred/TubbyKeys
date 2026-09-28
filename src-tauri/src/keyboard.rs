//! Global key listener. A low-level keyboard hook on its own thread reads only
//! the physical scan code and whether the key went down or up. Characters are
//! never translated, stored, or logged. Every event is passed on to Windows
//! untouched.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use serde::Serialize;

use crate::audio::{KeyEvent, Params, Waker};
use crate::keymap::{self, KeyClass};

/// Flags the hook reads on every event.
#[derive(Default)]
pub struct HookFlags {
    /// Forward key positions to the visualizer overlay.
    pub visualizer: AtomicBool,
}

/// What the overlay receives: a position on the board, never a key identity.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct KeyPulse {
    pub row: u8,
    pub x: f32,
    pub wide: bool,
}

struct Context {
    keys: rtrb::Producer<KeyEvent>,
    params: Arc<Params>,
    waker: Waker,
    flags: Arc<HookFlags>,
    pulses: SyncSender<KeyPulse>,
    held: [bool; 512],
}

impl Context {
    fn handle(&mut self, scan: u16, down: bool) {
        let slot = &mut self.held[scan as usize & 511];
        if down == *slot {
            return; // auto-repeat, or a release we never saw pressed
        }
        *slot = down;
        let pos = keymap::lookup(scan);
        // Sounds off: nothing is queued, so the audio device stays asleep.
        if self.params.enabled.load(Relaxed) && self.keys.push(KeyEvent { pos, down }).is_ok() {
            self.waker.wake();
        }
        if down && self.flags.visualizer.load(Relaxed) {
            let wide = matches!(
                pos.class,
                KeyClass::Space | KeyClass::Enter | KeyClass::Backspace
            );
            let _ = self.pulses.try_send(KeyPulse {
                row: pos.row,
                x: pos.x,
                wide,
            });
        }
    }
}

#[cfg(windows)]
pub fn start(
    keys: rtrb::Producer<KeyEvent>,
    params: Arc<Params>,
    waker: Waker,
    flags: Arc<HookFlags>,
    pulses: SyncSender<KeyPulse>,
) {
    use std::cell::RefCell;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetTimer, SetWindowsHookExW,
        UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, MSG, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
    };

    thread_local! {
        static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
    }

    unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let message = wparam.0 as u32;
            let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
            let up = message == WM_KEYUP || message == WM_SYSKEYUP;
            if down || up {
                // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
                let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
                let mut scan = info.scanCode;
                if scan == 0 {
                    // Some injected keys carry only a virtual-key code.
                    scan = unsafe { MapVirtualKeyW(info.vkCode, MAPVK_VK_TO_VSC) };
                }
                let extended = info.flags.0 & LLKHF_EXTENDED.0 != 0;
                let id = keymap::scan_id(scan, extended);
                CONTEXT.with(|cell| {
                    if let Ok(mut guard) = cell.try_borrow_mut() {
                        if let Some(context) = guard.as_mut() {
                            context.handle(id, down);
                        }
                    }
                });
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    std::thread::Builder::new()
        .name("tubbykeys-keyboard".into())
        .spawn(move || {
            CONTEXT.with(|cell| {
                *cell.borrow_mut() = Some(Context {
                    keys,
                    params,
                    waker,
                    flags,
                    pulses,
                    held: [false; 512],
                })
            });
            unsafe {
                // Windows silently removes a low-level hook whose callback ever misses
                // the system timeout, for example during a CPU spike. So this thread
                // runs at top priority and reinstalls the hook every few seconds. The
                // new hook goes in before the old one comes out; `held` drops the
                // duplicate events of that brief overlap.
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
                let module = GetModuleHandleW(None).ok().map(Into::into);
                let install = || SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), module, 0);
                let mut current = match install() {
                    Ok(hook) => hook,
                    Err(e) => {
                        eprintln!("keyboard hook failed: {e}");
                        return;
                    }
                };
                SetTimer(None, 0, 5_000, None);
                // The hook runs inside this message loop.
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_TIMER {
                        if let Ok(fresh) = install() {
                            let _ = UnhookWindowsHookEx(current);
                            current = fresh;
                        }
                        continue;
                    }
                    DispatchMessageW(&msg);
                }
            }
        })
        .expect("failed to start the keyboard thread");
}

#[cfg(not(windows))]
pub fn start(
    _keys: rtrb::Producer<KeyEvent>,
    _params: Arc<Params>,
    _waker: Waker,
    _flags: Arc<HookFlags>,
    _pulses: SyncSender<KeyPulse>,
) {
    eprintln!("the global key listener is only implemented for Windows");
}
