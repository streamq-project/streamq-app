#[napi(object)]
#[derive(Clone, Debug)]
pub struct WidgetBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
