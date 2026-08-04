use napi::bindgen_prelude::{Function, Unknown};
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::Status;
use std::sync::{Arc, Mutex};

use crate::models::media::{AppMediaSession, AudioSource};

pub type Tsfn<T> = ThreadsafeFunction<T, (), T, Status, true>;

macro_rules! define_events {
    (
        $( $js_name:expr => $variant:ident($payload:ty) => $field:ident ),* $(,)?
    ) => {
        pub enum Event {
            $( $variant($payload), )*
        }

        pub enum EventListener {
            $( $variant(Tsfn<$payload>), )*
        }

        #[derive(Default)]
        struct Listeners {
            $( $field: Vec<Tsfn<$payload>>, )*
        }

        #[derive(Clone, Default)]
        pub struct EventEmitter {
            listeners: Arc<Mutex<Listeners>>,
        }

        impl EventEmitter {
            pub fn new() -> Self {
                Self {
                    listeners: Arc::new(Mutex::new(Listeners::default())),
                }
            }

            pub fn on(&self, listener: EventListener) {
                let mut listeners = self.listeners.lock().unwrap();
                match listener {
                    $( EventListener::$variant(tsfn) => listeners.$field.push(tsfn), )*
                }
            }

            pub fn emit(&self, event: Event) {
                let listeners = self.listeners.lock().unwrap();
                match event {
                    $(
                        Event::$variant(payload) => {
                            for tsfn in &listeners.$field {
                                tsfn.call(Ok(payload.clone()), ThreadsafeFunctionCallMode::Blocking);
                            }
                        }
                    )*
                }
            }

            pub fn register_js_listener<'env>(&self, ev: &str, callback: Function<'env, Unknown<'static>, ()>) -> napi::Result<()> {
                match ev {
                    $(
                        $js_name => {
                            let tsfn = callback
                                .build_threadsafe_function::<$payload>()
                                .callee_handled::<true>()
                                .build_callback(|ctx| Ok(ctx.value))?;
                            self.on(EventListener::$variant(tsfn));
                        }
                    )*
                    _ => {}
                }
                Ok(())
            }
        }
    };
}

define_events! {
    "keyDown" => KeyDown(u32) => key_down,
    "keyUp" => KeyUp(u32) => key_up,
    "mouseDown" => MouseDown(u32) => mouse_down,
    "mouseUp" => MouseUp(u32) => mouse_up,
    "keybindPressed" => KeybindPressed(String) => keybind_pressed,
    "mediaAction" => MediaAction(serde_json::Value) => media_action,
    "mediaSessionsChanged" => MediaSessionsChanged(Vec<AppMediaSession>) => media_sessions_changed,
    "sourcesChanged" => SourcesChanged(Vec<AudioSource>) => sources_changed,
    "appAudioSourceChanged" => AppAudioSourceChanged(Option<AudioSource>) => app_audio_source_changed,
}
