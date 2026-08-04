#[napi(object)]
#[derive(Clone, Debug)]
pub struct Keybind {
    pub action: Option<String>,
    pub label: Option<String>,
    // Array of JS code strings (e.g., ["AltLeft", "KeyA"], ["M_1"], ["WheelUp"])
    pub bind: Vec<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct Config {
    pub debug: bool,
    pub keybinds: Vec<Keybind>,
    pub extract_thumbnails: Option<bool>,
}
