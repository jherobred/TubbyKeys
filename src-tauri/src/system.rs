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
}

#[cfg(not(windows))]
mod imp {
    pub fn init_com() {}
    pub fn default_output_is_headphones() -> bool {
        false
    }
    pub fn taskbar_is_light() -> bool {
        false
    }
}

pub use imp::*;
