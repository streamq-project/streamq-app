use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsInput {
    Key(u16, &'static str),
    OsSpecificKey(u16),
    MouseBtn(u16, &'static str),
    Scroll { amount: i32, is_horizontal: bool },
}

impl OsInput {
    pub fn to_vk(&self) -> Option<u16> {
        match self {
            OsInput::Key(vk, _) => Some(*vk),
            OsInput::OsSpecificKey(vk) => Some(*vk),
            OsInput::MouseBtn(vk, _) => Some(*vk),
            OsInput::Scroll { is_horizontal: false, amount } => {
                if *amount > 0 { Some(0x0100) } else { Some(0x0101) } // WheelUp / WheelDown
            }
            OsInput::Scroll { is_horizontal: true, amount } => {
                if *amount > 0 { Some(0x0102) } else { Some(0x0103) } // WheelRight / WheelLeft
            }
        }
    }

    pub fn to_xkb(&self) -> Option<&'static str> {
        match self {
            OsInput::Key(_, xkb_name) => Some(*xkb_name),
            OsInput::OsSpecificKey(_) => None,
            OsInput::MouseBtn(_, xkb_name) => Some(*xkb_name),
            OsInput::Scroll { is_horizontal: false, amount } => {
                if *amount > 0 {
                    Some("Button4")
                } else {
                    Some("Button5")
                }
            }
            OsInput::Scroll { is_horizontal: true, amount } => {
                if *amount > 0 {
                    Some("Button6")
                } else {
                    Some("Button7")
                }
            }
        }
    }

    pub fn is_mouse(&self) -> bool {
        matches!(self, OsInput::MouseBtn(..) | OsInput::Scroll { .. })
    }
}

pub fn parse_js_input(code: &str) -> Option<OsInput> {
    if let Some(vk_str) = code.strip_prefix("VK_") {
        return vk_str.parse().ok().map(OsInput::OsSpecificKey);
    }

    Some(match code {
        "Mouse0" => OsInput::MouseBtn(0x01, "Button1"),
        "Mouse1" | "Middle" => OsInput::MouseBtn(0x04, "Button2"),
        "Mouse2" => OsInput::MouseBtn(0x02, "Button3"),
        "Mouse3" | "X1" | "Back" => OsInput::MouseBtn(0x05, "Button8"),
        "Mouse4" | "X2" | "Forward" => OsInput::MouseBtn(0x06, "Button9"),

        "WheelUp" => OsInput::Scroll {
            amount: 1,
            is_horizontal: false,
        },
        "WheelDown" => OsInput::Scroll {
            amount: -1,
            is_horizontal: false,
        },
        "WheelLeft" => OsInput::Scroll {
            amount: -1,
            is_horizontal: true,
        },
        "WheelRight" => OsInput::Scroll {
            amount: 1,
            is_horizontal: true,
        },

        "ShiftLeft" => OsInput::Key(0xA0, "SHIFT"),
        "ShiftRight" => OsInput::Key(0xA1, "SHIFT"),
        "Shift" => OsInput::Key(0x10, "SHIFT"),
        "ControlLeft" => OsInput::Key(0xA2, "CTRL"),
        "ControlRight" => OsInput::Key(0xA3, "CTRL"),
        "Control" | "Ctrl" => OsInput::Key(0x11, "CTRL"),
        "AltLeft" => OsInput::Key(0xA4, "ALT"),
        "AltRight" => OsInput::Key(0xA5, "ALT"),
        "Alt" => OsInput::Key(0x12, "ALT"),
        "MetaLeft" | "OSLeft" => OsInput::Key(0x5B, "LOGO"),
        "MetaRight" | "OSRight" => OsInput::Key(0x5C, "LOGO"),
        "Meta" | "OS" | "Win" | "Super" => OsInput::Key(0x5B, "LOGO"),

        "KeyA" | "A" => OsInput::Key(0x41, "a"),
        "KeyB" | "B" => OsInput::Key(0x42, "b"),
        "KeyC" | "C" => OsInput::Key(0x43, "c"),
        "KeyD" | "D" => OsInput::Key(0x44, "d"),
        "KeyE" | "E" => OsInput::Key(0x45, "e"),
        "KeyF" | "F" => OsInput::Key(0x46, "f"),
        "KeyG" | "G" => OsInput::Key(0x47, "g"),
        "KeyH" | "H" => OsInput::Key(0x48, "h"),
        "KeyI" | "I" => OsInput::Key(0x49, "i"),
        "KeyJ" | "J" => OsInput::Key(0x4A, "j"),
        "KeyK" | "K" => OsInput::Key(0x4B, "k"),
        "KeyL" | "L" => OsInput::Key(0x4C, "l"),
        "KeyM" | "M" => OsInput::Key(0x4D, "m"),
        "KeyN" | "N" => OsInput::Key(0x4E, "n"),
        "KeyO" | "O" => OsInput::Key(0x4F, "o"),
        "KeyP" | "P" => OsInput::Key(0x50, "p"),
        "KeyQ" | "Q" => OsInput::Key(0x51, "q"),
        "KeyR" | "R" => OsInput::Key(0x52, "r"),
        "KeyS" | "S" => OsInput::Key(0x53, "s"),
        "KeyT" | "T" => OsInput::Key(0x54, "t"),
        "KeyU" | "U" => OsInput::Key(0x55, "u"),
        "KeyV" | "V" => OsInput::Key(0x56, "v"),
        "KeyW" | "W" => OsInput::Key(0x57, "w"),
        "KeyX" | "X" => OsInput::Key(0x58, "x"),
        "KeyY" | "Y" => OsInput::Key(0x59, "y"),
        "KeyZ" | "Z" => OsInput::Key(0x5A, "z"),

        "Digit0" | "D0" => OsInput::Key(0x30, "0"),
        "Digit1" | "D1" => OsInput::Key(0x31, "1"),
        "Digit2" | "D2" => OsInput::Key(0x32, "2"),
        "Digit3" | "D3" => OsInput::Key(0x33, "3"),
        "Digit4" | "D4" => OsInput::Key(0x34, "4"),
        "Digit5" | "D5" => OsInput::Key(0x35, "5"),
        "Digit6" | "D6" => OsInput::Key(0x36, "6"),
        "Digit7" | "D7" => OsInput::Key(0x37, "7"),
        "Digit8" | "D8" => OsInput::Key(0x38, "8"),
        "Digit9" | "D9" => OsInput::Key(0x39, "9"),

        "F1" => OsInput::Key(0x70, "F1"),
        "F2" => OsInput::Key(0x71, "F2"),
        "F3" => OsInput::Key(0x72, "F3"),
        "F4" => OsInput::Key(0x73, "F4"),
        "F5" => OsInput::Key(0x74, "F5"),
        "F6" => OsInput::Key(0x75, "F6"),
        "F7" => OsInput::Key(0x76, "F7"),
        "F8" => OsInput::Key(0x77, "F8"),
        "F9" => OsInput::Key(0x78, "F9"),
        "F10" => OsInput::Key(0x79, "F10"),
        "F11" => OsInput::Key(0x7A, "F11"),
        "F12" => OsInput::Key(0x7B, "F12"),
        "F13" => OsInput::Key(0x7C, "F13"),
        "F14" => OsInput::Key(0x7D, "F14"),
        "F15" => OsInput::Key(0x7E, "F15"),
        "F16" => OsInput::Key(0x7F, "F16"),
        "F17" => OsInput::Key(0x80, "F17"),
        "F18" => OsInput::Key(0x81, "F18"),
        "F19" => OsInput::Key(0x82, "F19"),
        "F20" => OsInput::Key(0x83, "F20"),
        "F21" => OsInput::Key(0x84, "F21"),
        "F22" => OsInput::Key(0x85, "F22"),
        "F23" => OsInput::Key(0x86, "F23"),
        "F24" => OsInput::Key(0x87, "F24"),

        "ArrowUp" | "Up" => OsInput::Key(0x26, "Up"),
        "ArrowDown" | "Down" => OsInput::Key(0x28, "Down"),
        "ArrowLeft" | "Left" => OsInput::Key(0x25, "Left"),
        "ArrowRight" | "Right" => OsInput::Key(0x27, "Right"),
        "Home" => OsInput::Key(0x24, "Home"),
        "End" => OsInput::Key(0x23, "End"),
        "PageUp" | "Prior" => OsInput::Key(0x21, "Prior"),
        "PageDown" | "Next" => OsInput::Key(0x22, "Next"),
        "Insert" => OsInput::Key(0x2D, "Insert"),
        "Delete" => OsInput::Key(0x2E, "Delete"),

        "Enter" | "Return" => OsInput::Key(0x0D, "Return"),
        "Backspace" => OsInput::Key(0x08, "BackSpace"),
        "Tab" => OsInput::Key(0x09, "Tab"),
        "Escape" | "Esc" => OsInput::Key(0x1B, "Escape"),
        "Space" => OsInput::Key(0x20, "space"),

        "CapsLock" => OsInput::Key(0x14, "Caps_Lock"),
        "NumLock" => OsInput::Key(0x90, "Num_Lock"),
        "ScrollLock" => OsInput::Key(0x91, "Scroll_Lock"),

        "Numpad0" | "Num0" => OsInput::Key(0x60, "KP_0"),
        "Numpad1" | "Num1" => OsInput::Key(0x61, "KP_1"),
        "Numpad2" | "Num2" => OsInput::Key(0x62, "KP_2"),
        "Numpad3" | "Num3" => OsInput::Key(0x63, "KP_3"),
        "Numpad4" | "Num4" => OsInput::Key(0x64, "KP_4"),
        "Numpad5" | "Num5" => OsInput::Key(0x65, "KP_5"),
        "Numpad6" | "Num6" => OsInput::Key(0x66, "KP_6"),
        "Numpad7" | "Num7" => OsInput::Key(0x67, "KP_7"),
        "Numpad8" | "Num8" => OsInput::Key(0x68, "KP_8"),
        "Numpad9" | "Num9" => OsInput::Key(0x69, "KP_9"),
        "NumpadMultiply" | "NumMultiply" => OsInput::Key(0x6A, "KP_Multiply"),
        "NumpadAdd" | "NumAdd" => OsInput::Key(0x6B, "KP_Add"),
        "NumpadSubtract" | "NumSubtract" => OsInput::Key(0x6D, "KP_Subtract"),
        "NumpadDecimal" | "NumDecimal" => OsInput::Key(0x6E, "KP_Decimal"),
        "NumpadDivide" | "NumDivide" => OsInput::Key(0x6F, "KP_Divide"),
        "NumpadEnter" | "NumEnter" => OsInput::Key(0x0D, "KP_Enter"),
        "NumpadSeparator" | "NumSeparator" => OsInput::Key(0x6C, "KP_Separator"),

        "AudioVolumeMute" | "VolumeMute" => OsInput::Key(0xAD, "XF86AudioMute"),
        "AudioVolumeDown" | "VolumeDown" => OsInput::Key(0xAE, "XF86AudioLowerVolume"),
        "AudioVolumeUp" | "VolumeUp" => OsInput::Key(0xAF, "XF86AudioRaiseVolume"),
        "MediaPlayPause" => OsInput::Key(0xB3, "XF86AudioPlay"),
        "MediaStop" => OsInput::Key(0xB2, "XF86AudioStop"),
        "MediaTrackNext" | "MediaNext" => OsInput::Key(0xB0, "XF86AudioNext"),
        "MediaTrackPrevious" | "MediaPrevious" => OsInput::Key(0xB1, "XF86AudioPrev"),
        "MediaSelect" => OsInput::Key(0xB5, "XF86AudioMedia"),
        "LaunchMail" => OsInput::Key(0xB4, "XF86Mail"),
        "LaunchApp1" => OsInput::Key(0xB6, "XF86Launch1"),
        "LaunchApp2" => OsInput::Key(0xB7, "XF86Launch2"),

        "BrowserBack" => OsInput::Key(0xA6, "XF86Back"),
        "BrowserForward" => OsInput::Key(0xA7, "XF86Forward"),
        "BrowserRefresh" => OsInput::Key(0xA8, "XF86Refresh"),
        "BrowserStop" => OsInput::Key(0xA9, "XF86Stop"),
        "BrowserSearch" => OsInput::Key(0xAA, "XF86Search"),
        "BrowserFavorites" => OsInput::Key(0xAB, "XF86Favorites"),
        "BrowserHome" => OsInput::Key(0xAC, "XF86HomePage"),

        "Semicolon" => OsInput::Key(0xBA, "semicolon"),
        "Equal" => OsInput::Key(0xBB, "equal"),
        "Comma" => OsInput::Key(0xBC, "comma"),
        "Minus" => OsInput::Key(0xBD, "minus"),
        "Period" => OsInput::Key(0xBE, "period"),
        "Slash" => OsInput::Key(0xBF, "slash"),
        "Backquote" | "Grave" => OsInput::Key(0xC0, "grave"),
        "BracketLeft" => OsInput::Key(0xDB, "bracketleft"),
        "Backslash" => OsInput::Key(0xDC, "backslash"),
        "BracketRight" => OsInput::Key(0xDD, "bracketright"),
        "Quote" => OsInput::Key(0xDE, "apostrophe"),

        "Pause" | "Break" => OsInput::Key(0x13, "Pause"),
        "PrintScreen" | "Snapshot" => OsInput::Key(0x2A, "Print"),
        "ContextMenu" | "Apps" | "Menu" => OsInput::Key(0x5D, "Menu"),
        "Sleep" => OsInput::Key(0x5F, "XF86Sleep"),
        "WakeUp" => OsInput::Key(0xE3, "XF86WakeUp"),

        "IntlBackslash" => OsInput::Key(0xE2, "backslash"),
        "IntlRo" => OsInput::Key(0xE1, "ro"),
        "IntlYen" => OsInput::Key(0xE3, "yen"),

        _ => return None,
    })
}

pub fn bind_to_xkb_trigger(bind: &[OsInput]) -> Option<String> {
    if bind.is_empty() {
        return None;
    }

    let parts: Vec<&str> = bind.iter().filter_map(|item| item.to_xkb()).collect();
    if parts.is_empty() { None } else { Some(parts.join("+")) }
}
