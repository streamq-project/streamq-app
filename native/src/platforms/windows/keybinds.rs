use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::thread;
use tracing::{info, instrument, warn};
use winapi::shared::windef::HHOOK;
use winapi::um::winuser::{
    CallNextHookEx, DispatchMessageA, GetMessageA, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, SetWindowsHookExA, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEWHEEL,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

use crate::Config;
use crate::config::Keybind;
use crate::input_codes::parse_js_input;

struct HookHandle(HHOOK);
unsafe impl Send for HookHandle {}
unsafe impl Sync for HookHandle {}

static KEYBOARD_HOOK_HANDLE: LazyLock<Mutex<Option<HookHandle>>> = LazyLock::new(|| Mutex::new(None));
static MOUSE_HOOK_HANDLE: LazyLock<Mutex<Option<HookHandle>>> = LazyLock::new(|| Mutex::new(None));
static HOOK_EVENT_EMITTER: LazyLock<Mutex<Option<std::sync::Arc<crate::event_emitter::EventEmitter>>>> = LazyLock::new(|| Mutex::new(None));

pub static NOW_PRESSED: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub static KEYBINDS: LazyLock<Mutex<Vec<Keybind>>> = LazyLock::new(|| Mutex::new(Vec::new()));

pub struct KeybindsManager {
    #[allow(dead_code)]
    config: Config,
}

impl KeybindsManager {
    pub fn new(config: Config, event_emitter: std::sync::Arc<crate::event_emitter::EventEmitter>) -> Self {
        *HOOK_EVENT_EMITTER.lock().unwrap() = Some(event_emitter);
        let manager = Self { config };

        manager.initialize();
        manager
    }

    #[instrument(skip(self))]
    fn initialize(&self) {
        info!(keybinds_count = self.config.keybinds.len(), "Windows keybinds initializing");
        *KEYBINDS.lock().unwrap() = self.config.keybinds.clone();

        thread::spawn(move || {
            unsafe {
                let keyboard_hook_id = SetWindowsHookExA(WH_KEYBOARD_LL, Some(keyboard_hook_callback), std::ptr::null_mut(), 0);
                let mouse_hook_id = SetWindowsHookExA(WH_MOUSE_LL, Some(mouse_hook_callback), std::ptr::null_mut(), 0);

                *KEYBOARD_HOOK_HANDLE.lock().unwrap() = Some(HookHandle(keyboard_hook_id));
                *MOUSE_HOOK_HANDLE.lock().unwrap() = Some(HookHandle(mouse_hook_id));

                let mut msg: MSG = std::mem::zeroed();
                while GetMessageA(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }

                // Cleanup hooks when message loop exits
                if let Some(HookHandle(hook_id)) = KEYBOARD_HOOK_HANDLE.lock().unwrap().take() {
                    UnhookWindowsHookEx(hook_id);
                }
                if let Some(HookHandle(hook_id)) = MOUSE_HOOK_HANDLE.lock().unwrap().take() {
                    UnhookWindowsHookEx(hook_id);
                }
            }
        });
    }

    pub fn set_keybinds(&self, keybinds: Vec<Keybind>) {
        *KEYBINDS.lock().unwrap() = keybinds;
    }

    #[instrument(skip(self))]
    pub fn cleanup(&self) {
        info!("Windows keybinds cleanup initiated");

        let mut keyboard_handle = KEYBOARD_HOOK_HANDLE.lock().unwrap();
        let mut mouse_handle = MOUSE_HOOK_HANDLE.lock().unwrap();

        if let Some(HookHandle(hook_id)) = keyboard_handle.take() {
            unsafe {
                UnhookWindowsHookEx(hook_id);
            }
            info!("Keyboard hook removed");
        }

        if let Some(HookHandle(hook_id)) = mouse_handle.take() {
            unsafe {
                UnhookWindowsHookEx(hook_id);
            }
            info!("Mouse hook removed");
        }

        info!("Windows keybinds cleanup completed");
    }
}

fn check_keybind_match(keybinds: &[Keybind]) -> Option<String> {
    let pressed = NOW_PRESSED.lock().unwrap();

    for kb in keybinds {
        let action = match &kb.action {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };

        let required_vks: Vec<u16> = kb.bind.iter().filter_map(|code| parse_js_input(code).and_then(|input| input.to_vk())).collect();

        if required_vks.is_empty() {
            continue;
        }

        if required_vks.iter().all(|vk| pressed.contains(vk)) {
            return Some(action.clone());
        }
    }

    None
}

extern "system" fn keyboard_hook_callback(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        let wparam_int = wparam as u32;
        let keypress: KBDLLHOOKSTRUCT = unsafe { *(lparam as *mut KBDLLHOOKSTRUCT) };
        let vk = keypress.vkCode as u16;

        if wparam_int == WM_KEYDOWN || wparam_int == WM_SYSKEYDOWN {
            NOW_PRESSED.lock().unwrap().insert(vk);

            let keybinds = KEYBINDS.lock().unwrap().clone();
            if let Some(action) = check_keybind_match(&keybinds) {
                if let Some(emitter) = HOOK_EVENT_EMITTER.lock().unwrap().as_ref() {
                    emitter.emit(crate::event_emitter::Event::KeybindPressed(action));
                }

                // Suppress media keys
                if (0xAD..=0xB5).contains(&vk) {
                    // Volume mute, down, up, media keys
                    return 1;
                }
            }
        } else if wparam_int == WM_KEYUP || wparam_int == WM_SYSKEYUP {
            NOW_PRESSED.lock().unwrap().remove(&vk);
        }
    }

    let hook_handle = KEYBOARD_HOOK_HANDLE.lock().unwrap();
    if let Some(HookHandle(hook_id)) = *hook_handle {
        unsafe {
            return CallNextHookEx(hook_id, code, wparam, lparam);
        }
    }
    0
}

extern "system" fn mouse_hook_callback(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        let wparam_int = wparam as u32;
        let mousepress: MSLLHOOKSTRUCT = unsafe { *(lparam as *mut MSLLHOOKSTRUCT) };

        if wparam_int == WM_MOUSEWHEEL || wparam_int == WM_MOUSEHWHEEL {
            let delta = (mousepress.mouseData >> 16) as i16;
            let is_horizontal = wparam_int == WM_MOUSEHWHEEL;

            let wheel_vk: u16 = if is_horizontal {
                if delta > 0 {
                    0x0102 // WheelRight
                } else {
                    0x0103 // WheelLeft
                }
            } else if delta > 0 {
                0x0100 // WheelUp
            } else {
                0x0101 // WheelDown
            };

            NOW_PRESSED.lock().unwrap().insert(wheel_vk);

            let keybinds = KEYBINDS.lock().unwrap().clone();
            if let Some(action) = check_keybind_match(&keybinds) {
                if let Some(emitter) = HOOK_EVENT_EMITTER.lock().unwrap().as_ref() {
                    emitter.emit(crate::event_emitter::Event::KeybindPressed(action));
                }
            }

            // Remove wheel "key" immediately (wheel is a momentary event)
            NOW_PRESSED.lock().unwrap().remove(&wheel_vk);

            let hook_handle = MOUSE_HOOK_HANDLE.lock().unwrap();
            if let Some(HookHandle(hook_id)) = *hook_handle {
                unsafe {
                    return CallNextHookEx(hook_id, code, wparam, lparam);
                }
            }
            return 0;
        }

        let btn_vk: u16 = match wparam_int {
            WM_LBUTTONDOWN | WM_LBUTTONUP => 0x01, // VK_LBUTTON
            WM_RBUTTONDOWN | WM_RBUTTONUP => 0x02, // VK_RBUTTON
            WM_MBUTTONDOWN | WM_MBUTTONUP => 0x04, // VK_MBUTTON
            WM_XBUTTONDOWN | WM_XBUTTONUP => match mousepress.mouseData {
                0x10000 => 0x05, // VK_XBUTTON1
                0x20000 => 0x06, // VK_XBUTTON2
                _ => return 0,
            },
            _ => return 0,
        };

        let is_down = matches!(wparam_int, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN);

        if is_down {
            NOW_PRESSED.lock().unwrap().insert(btn_vk);

            let keybinds = KEYBINDS.lock().unwrap().clone();
            if let Some(action) = check_keybind_match(&keybinds) {
                if let Some(emitter) = HOOK_EVENT_EMITTER.lock().unwrap().as_ref() {
                    emitter.emit(crate::event_emitter::Event::KeybindPressed(action));
                }
            }
        } else {
            NOW_PRESSED.lock().unwrap().remove(&btn_vk);
        }
    }

    let hook_handle = MOUSE_HOOK_HANDLE.lock().unwrap();
    if let Some(HookHandle(hook_id)) = *hook_handle {
        unsafe {
            return CallNextHookEx(hook_id, code, wparam, lparam);
        }
    }
    0
}
