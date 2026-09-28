//! Small Windows integrations: headphone detection, taskbar theme, and window
//! styles for the click-through visualizer overlay.

#[cfg(windows)]
mod imp {
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, PKEY_AudioEndpoint_FormFactor,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
    };
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows::Win32::System::Variant::VT_UI4;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    pub fn init_com() {
        // S_FALSE when COM is already initialised on this thread is fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    }

    /// True when the default output reports itself as headphones or a headset
    /// (wired, USB and Bluetooth devices all set this form factor).
    pub fn default_output_is_headphones() -> bool {
        const HEADPHONES: u32 = 3;
        const HEADSET: u32 = 5;
        let form_factor = || -> windows::core::Result<Option<u32>> {
            unsafe {
                let enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
                let store = device.OpenPropertyStore(STGM_READ)?;
                let value = store.GetValue(&PKEY_AudioEndpoint_FormFactor)?;
                let inner = &value.Anonymous.Anonymous;
                Ok((inner.vt == VT_UI4).then(|| inner.Anonymous.ulVal))
            }
        };
        matches!(form_factor(), Ok(Some(HEADPHONES | HEADSET)))
    }

    /// True when the taskbar uses the light theme, so the tray icon must be dark.
    pub fn taskbar_is_light() -> bool {
        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("SystemUsesLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut value as *mut u32 as *mut _),
                Some(&mut size),
            )
        };
        status.is_ok() && value == 1
    }

    /// Keep the overlay out of Alt+Tab. Must run after the window's last
    /// style change: tao rewrites the extended style whenever its flags change.
    pub fn hide_from_alt_tab(hwnd: HWND) {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & !(WS_EX_APPWINDOW.0 as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW.0 as isize);
        }
    }

    /// Turn off the system show/hide animation so the window's own CSS
    /// motion is the only one.
    pub fn disable_transitions(hwnd: HWND) {
        use windows::core::BOOL;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
        };
        let off = BOOL::from(true);
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &off as *const BOOL as *const _,
                std::mem::size_of::<BOOL>() as u32,
            )
        };
    }

    /// True while a full-screen game, video or presentation owns the screen:
    /// an exclusive full-screen game, presentation mode, or a foreground
    /// window covering its whole monitor. (Windows' own "busy" flag is not
    /// used: background apps such as live wallpapers can keep it on.)
    pub fn fullscreen_app_active() -> bool {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };
        use windows::Win32::UI::Shell::{
            SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetClassNameW, GetForegroundWindow, GetWindowRect, IsZoomed, GWL_STYLE, WS_CAPTION,
        };
        if matches!(
            unsafe { SHQueryUserNotificationState() },
            Ok(state) if state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE
        ) {
            return true;
        }
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return false;
            }
            // The desktop itself covers the monitor but is not an app.
            let mut class = [0u16; 32];
            let len = GetClassNameW(hwnd, &mut class) as usize;
            let class = String::from_utf16_lossy(&class[..len]);
            if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
                return false;
            }
            // A maximized window with a title bar is not full screen, even when
            // an auto-hiding taskbar lets it cover the whole monitor.
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            if IsZoomed(hwnd).as_bool() && style & WS_CAPTION.0 == WS_CAPTION.0 {
                return false;
            }
            let mut window = RECT::default();
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetWindowRect(hwnd, &mut window).is_err()
                || !GetMonitorInfoW(
                    MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                    &mut monitor,
                )
                .as_bool()
            {
                return false;
            }
            let m = monitor.rcMonitor;
            window.left <= m.left
                && window.top <= m.top
                && window.right >= m.right
                && window.bottom >= m.bottom
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn init_com() {}
    pub fn fullscreen_app_active() -> bool {
        false
    }
    pub fn default_output_is_headphones() -> bool {
        false
    }
    pub fn taskbar_is_light() -> bool {
        false
    }
}

pub use imp::*;
