use crate::config::Config;
use crate::error::NativeError;
use std::ffi::c_void;
use winapi::shared::windef::HWND;
use winapi::um::winuser::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetWindowLongA, GetWindowTextA, GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST, IsWindow, IsWindowVisible,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongA, SetWindowPos, WS_CLIPCHILDREN, WS_EX_TOPMOST,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
pub use windows_sys::Win32::{Foundation::*, Graphics::Dwm::*, System::LibraryLoader::*};

#[allow(dead_code)]
type Color = (u8, u8, u8, u8);

pub struct WindowManager;

impl WindowManager {
    pub fn new(_config: Config) -> Self {
        Self
    }

    #[allow(dead_code)]
    pub fn set_acrylic(&self, hwnd: i64, enable: bool, color: Option<Color>) -> Result<(), NativeError> {
        if hwnd == 0 {
            return Err(NativeError::InvalidWindowHandle);
        }

        unsafe {
            let state = if enable {
                AccentState::AccentEnableAcrylicblurbehind
            } else {
                AccentState::AccentDisabled
            };
            let success = set_window_composition_attribute(hwnd as HWND, state, color);
            if !success {
                return Err(NativeError::WindowEffectFailed("Failed to apply acrylic effect".into()));
            }
        }
        Ok(())
    }

    pub fn restore_native_frame(&self, hwnd: i64) -> Result<(), NativeError> {
        if hwnd == 0 {
            return Err(NativeError::InvalidWindowHandle);
        }

        if !unsafe { restore_native_frame(hwnd as HWND) } {
            return Err(NativeError::WindowEffectFailed("Failed to restore native window frame".into()));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn disable_rounds(&self, hwnd: i64) -> Result<(), NativeError> {
        if hwnd == 0 {
            return Err(NativeError::InvalidWindowHandle);
        }

        if is_at_least_build(22523) {
            unsafe {
                let hr = DwmSetWindowAttribute(hwnd as isize, DWMWA_WINDOW_CORNER_PREFERENCE as _, &DWMWCP_DONOTROUND as *const _ as _, 4);
                if hr < 0 {
                    return Err(NativeError::WindowEffectFailed("Failed to disable window corner rounding".into()));
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_pip_always_on_top_mode(&self, on_top: bool) -> Result<bool, NativeError> {
        unsafe {
            let Some(hwnd) = find_pip_window_by_process() else {
                return Ok(false);
            };

            if IsWindow(hwnd) == 0 {
                return Err(NativeError::InvalidWindowHandle);
            }

            let ex_style = GetWindowLongA(hwnd, GWL_EXSTYLE);
            if ex_style == 0 {
                let err = winapi::um::errhandlingapi::GetLastError();
                if err != 0 {
                    return Err(NativeError::WindowEffectFailed("Failed to get window style".into()));
                }
            }

            let is_already_on_top = (ex_style & WS_EX_TOPMOST as i32) != 0;
            if on_top == is_already_on_top {
                return Ok(true);
            }

            let new_ex_style = ex_style | if on_top { WS_EX_TOPMOST } else { !WS_EX_TOPMOST } as i32;
            let hwnd_insert = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };

            SetWindowLongA(hwnd, GWL_EXSTYLE, new_ex_style);
            let success = SetWindowPos(hwnd, hwnd_insert, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);

            if success == 0 {
                return Err(NativeError::WindowEffectFailed("Failed to set window position".into()));
            }

            Ok(true)
        }
    }
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut process_id) };

    let current_process_id = unsafe { GetCurrentProcessId() };
    if process_id == current_process_id && unsafe { IsWindowVisible(hwnd) } != 0 {
        let mut title_buffer = vec![0u8; 256];
        unsafe { GetWindowTextA(hwnd, title_buffer.as_mut_ptr() as *mut i8, title_buffer.len() as i32) };
        let title = String::from_utf8_lossy(&title_buffer);
        let style = unsafe { GetWindowLongA(hwnd, GWL_STYLE) };
        if (style & WS_CLIPCHILDREN as i32) != 0 && !title.contains("StreamQ") {
            let result_ptr = lparam as *mut Option<HWND>;
            if !result_ptr.is_null() {
                unsafe { *result_ptr = Some(hwnd) };
                return 0;
            }
        }
    }
    1
}

fn find_pip_window_by_process() -> Option<HWND> {
    let mut result: Option<HWND> = None;
    unsafe {
        EnumWindows(Some(enum_windows_proc), &mut result as *mut _ as LPARAM);
    }
    result
}

fn get_function_impl(library: &str, function: &str) -> Option<FARPROC> {
    assert_eq!(library.chars().last(), Some('\0'));
    assert_eq!(function.chars().last(), Some('\0'));

    let module = unsafe { LoadLibraryA(library.as_ptr()) };
    if module == 0 {
        return None;
    }
    Some(unsafe { GetProcAddress(module, function.as_ptr()) })
}

macro_rules! get_function {
    ($lib:expr, $func:ident) => {
        get_function_impl(concat!($lib, '\0'), concat!(stringify!($func), '\0'))
            .map(|f| unsafe { std::mem::transmute::<::windows_sys::Win32::Foundation::FARPROC, $func>(f) })
    };
}

#[allow(unused)]
#[repr(C)]
enum DwmSystembackdropType {
    DwmsbtDisable = 1,         // None
    DwmsbtMainwindow = 2,      // Mica
    DwmsbtTransientwindow = 3, // Acrylic
    DwmsbtTabbedwindow = 4,    // Tabbed
}

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

type WINDOWCOMPOSITIONATTRIB = u32;

#[repr(C)]
struct WINDOWCOMPOSITIONATTRIBDATA {
    attrib: WINDOWCOMPOSITIONATTRIB,
    pv_data: *mut c_void,
    cb_data: usize,
}

#[derive(PartialEq)]
#[repr(C)]
enum AccentState {
    AccentDisabled = 0,
    AccentEnableAcrylicblurbehind = 4,
}

unsafe fn set_window_composition_attribute(hwnd: HWND, accent_state: AccentState, color: Option<Color>) -> bool {
    type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut WINDOWCOMPOSITIONATTRIBDATA) -> BOOL;

    let Some(set_window_composition_attribute) = get_function!("user32.dll", SetWindowCompositionAttribute) else {
        return false;
    };

    let mut color = color.unwrap_or_default();
    let is_acrylic = accent_state == AccentState::AccentEnableAcrylicblurbehind;
    if is_acrylic && color.3 == 0 {
        color.3 = 1;
    }
    let mut policy = AccentPolicy {
        accent_state: accent_state as _,
        accent_flags: if is_acrylic { 0 } else { 2 },
        gradient_color: (color.0 as u32) | (color.1 as u32) << 8 | (color.2 as u32) << 16 | (color.3 as u32) << 24,
        animation_id: 0,
    };
    let mut data = WINDOWCOMPOSITIONATTRIBDATA {
        attrib: 0x13,
        pv_data: &mut policy as *mut _ as _,
        cb_data: std::mem::size_of_val(&policy),
    };

    (unsafe { set_window_composition_attribute(hwnd, &mut data as *mut _ as _) }) != 0
}

unsafe fn restore_native_frame(hwnd: HWND) -> bool {
    type GetWindowLongW = unsafe extern "system" fn(HWND, i32) -> i32;
    type SetWindowLongW = unsafe extern "system" fn(HWND, i32, i32) -> i32;
    type SetWindowPos = unsafe extern "system" fn(HWND, HWND, i32, i32, i32, i32, u32) -> BOOL;

    let Some(get_window_long) = get_function!("user32.dll", GetWindowLongW) else {
        return false;
    };

    let Some(set_window_long) = get_function!("user32.dll", SetWindowLongW) else {
        return false;
    };

    let Some(set_window_pos) = get_function!("user32.dll", SetWindowPos) else {
        return false;
    };

    const GWL_STYLE: i32 = -16;

    const WS_THICKFRAME: u32 = 0x0004_0000;
    const WS_CAPTION: u32 = 0x00C0_0000;

    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_NOOWNERZORDER: u32 = 0x0200;

    let style = unsafe { get_window_long(hwnd, GWL_STYLE) } as u32;
    let new_style = style | WS_THICKFRAME | WS_CAPTION;

    unsafe { set_window_long(hwnd, GWL_STYLE, new_style as i32) };

    let applied_style = unsafe { get_window_long(hwnd, GWL_STYLE) } as u32;
    if applied_style & (WS_THICKFRAME | WS_CAPTION) != (WS_THICKFRAME | WS_CAPTION) {
        return false;
    }

    (unsafe {
        set_window_pos(
            hwnd,
            0 as HWND,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_NOOWNERZORDER,
        )
    }) != 0
}

fn is_at_least_build(build: u32) -> bool {
    let v = windows_version::OsVersion::current();
    v.build >= build
}
