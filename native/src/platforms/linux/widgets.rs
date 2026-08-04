use crate::error::NativeError;
use crate::models::widgets::WidgetBounds;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use webkit2gtk::{SettingsExt, WebViewExt};

const FIRST_WIDGET_ID: u32 = 1 << 30;

struct Widget(gtk::Window);

impl Drop for Widget {
    fn drop(&mut self) {
        self.0.close();
    }
}

pub struct WidgetsManager {
    widgets: RefCell<HashMap<u32, Widget>>,
    next_id: Cell<u32>,
}

impl WidgetsManager {
    pub fn new() -> Self {
        Self {
            widgets: RefCell::new(HashMap::new()),
            next_id: Cell::new(FIRST_WIDGET_ID),
        }
    }

    pub fn create_overlay(&self, url: String, bounds: WidgetBounds) -> Result<u32, NativeError> {
        gtk::init().map_err(|error| NativeError::OperationFailed(format!("Failed to initialize GTK: {error}")))?;
        if !gtk_layer_shell::is_supported() {
            return Err(NativeError::OperationFailed(
                "The current Wayland compositor does not support zwlr_layer_shell_v1".into(),
            ));
        }

        let monitor = find_monitor(&bounds).ok_or_else(|| NativeError::OperationFailed("Unable to find the selected GTK monitor".into()))?;
        let id = self.next_id.get();

        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_decorated(false);
        window.set_app_paintable(true);

        if let Some(screen) = gtk::prelude::WidgetExt::screen(&window) {
            if let Some(visual) = screen.rgba_visual() {
                window.set_visual(Some(&visual));
            }
        }

        window.init_layer_shell();
        window.set_namespace("streamq-widget");
        window.set_layer(Layer::Overlay);
        window.set_monitor(&monitor);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_exclusive_zone(-1);
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            window.set_anchor(edge, true);
        }
        set_click_through(&window);
        window.connect_map(|window| set_click_through(window));

        let web_view = webkit2gtk::WebView::new();
        web_view.set_background_color(&gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));
        if let Some(settings) = WebViewExt::settings(&web_view) {
            settings.set_enable_plugins(false);
            settings.set_enable_media_capabilities(false);
        }
        // set_click_through(&web_view);
        // web_view.connect_map(|web_view| set_click_through(web_view));
        web_view.connect_context_menu(|_, _, _, _| true);
        web_view.connect_create(|_, _| None);
        web_view.connect_load_failed(move |_, _, failing_url, error| {
            tracing::error!(
                __redact = "url",
                widget_id = id,
                url = failing_url,
                error = %error,
                "Unable to load native widget"
            );
            false
        });
        web_view.load_uri(&url);

        window.add(&web_view);
        window.show_all();

        self.next_id.set(id + 1);
        self.widgets.borrow_mut().insert(id, Widget(window));
        tracing::info!(widget_id = id, "Native layer shell widget created");

        Ok(id)
    }

    pub fn destroy(&self, id: u32) -> Result<(), NativeError> {
        if self.widgets.borrow_mut().remove(&id).is_some() {
            tracing::info!(widget_id = id, "Native layer shell widget destroyed");
        }
        Ok(())
    }

    pub fn destroy_all(&self) -> Result<(), NativeError> {
        self.widgets.borrow_mut().clear();
        Ok(())
    }
}

fn set_click_through(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.as_ref();
    let empty_region = gtk::cairo::Region::create();
    widget.input_shape_combine_region(Some(&empty_region));
    if let Some(surface) = widget.window() {
        surface.set_pass_through(true);
    }
}

fn find_monitor(bounds: &WidgetBounds) -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;
    let center_x = bounds.x.saturating_add((bounds.width / 2) as i32);
    let center_y = bounds.y.saturating_add((bounds.height / 2) as i32);
    display.monitor_at_point(center_x, center_y).or_else(|| display.primary_monitor())
}
