#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

pub mod config;
pub mod error;
pub mod input_codes;
pub mod models;
use config::Config;

mod api;
mod event_emitter;
mod thumbnails;

mod platforms;

use napi::Result;
use napi::bindgen_prelude::{Function, Unknown};
use std::sync::mpsc;

#[derive(Clone)]
struct ChannelWriter {
    tx: mpsc::Sender<String>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            let _ = self.tx.send(s.to_string());
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ChannelWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[napi]
pub struct StreamQNative {
    config: Config,
    event_emitter: std::sync::Arc<event_emitter::EventEmitter>,
    log_rx: std::sync::Arc<std::sync::Mutex<Option<mpsc::Receiver<String>>>>,
    #[cfg(target_os = "windows")]
    keybinds_manager: std::sync::Arc<platforms::windows::keybinds::KeybindsManager>,
    #[cfg(target_os = "linux")]
    keybinds_manager: std::sync::Arc<platforms::linux::keybinds::KeybindsManager>,
    #[cfg(target_os = "windows")]
    media_manager: std::sync::Arc<platforms::windows::media_session::MediaSessionManager>,
    #[cfg(target_os = "linux")]
    media_manager: std::sync::Arc<platforms::linux::media_session::MediaSessionManager>,
    #[cfg(target_os = "linux")]
    mpris_manager: std::sync::Arc<platforms::linux::mpris::MprisManager>,
}

#[napi]
impl StreamQNative {
    #[napi(constructor)]
    pub fn new(config: Config) -> Self {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        let log_level = if config.debug { tracing::Level::DEBUG } else { tracing::Level::INFO };

        let (log_tx, log_rx) = mpsc::channel::<String>();

        let channel_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(ChannelWriter { tx: log_tx })
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true);

        tracing_subscriber::registry()
            .with(channel_layer)
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("streamq_native={}", log_level))),
            )
            .try_init()
            .ok();

        std::panic::set_hook(Box::new(|panic_info| {
            let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                *s
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };

            let location = panic_info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "unknown".to_string());

            eprintln!("Rust panicked at {location}: {msg}");
            tracing::error!(sentry = true, location, panic = msg, "Rust panicked");

            std::thread::sleep(std::time::Duration::from_millis(200));
        }));

        tracing::info!(debug = config.debug, platform = std::env::consts::OS, "streamq-native initializing");

        let event_emitter = std::sync::Arc::new(event_emitter::EventEmitter::new());

        #[cfg(target_os = "windows")]
        let keybinds_manager = {
            use platforms::windows::keybinds::KeybindsManager;
            std::sync::Arc::new(KeybindsManager::new(Config::from(config.clone()), event_emitter.clone()))
        };

        #[cfg(target_os = "linux")]
        let keybinds_manager = {
            use platforms::linux::keybinds::KeybindsManager;
            std::sync::Arc::new(KeybindsManager::new(Config::from(config.clone()), event_emitter.clone()))
        };

        #[cfg(target_os = "windows")]
        let media_manager = {
            use platforms::windows::media_session::MediaSessionManager;
            std::sync::Arc::new(MediaSessionManager::new(Config::from(config.clone()), event_emitter.clone()))
        };

        #[cfg(target_os = "linux")]
        let media_manager = {
            use platforms::linux::media_session::MediaSessionManager;
            std::sync::Arc::new(MediaSessionManager::new(Config::from(config.clone()), event_emitter.clone()))
        };

        #[cfg(target_os = "linux")]
        let mpris_manager = std::sync::Arc::new(platforms::linux::mpris::MprisManager::new(Config::from(config.clone()), event_emitter.clone()));

        Self {
            config,
            event_emitter,
            log_rx: std::sync::Arc::new(std::sync::Mutex::new(Some(log_rx))),
            keybinds_manager,
            media_manager,
            #[cfg(target_os = "linux")]
            mpris_manager,
        }
    }

    #[napi(getter)]
    pub fn window(&self) -> api::Window {
        api::Window::new(self.config.clone())
    }

    #[napi(getter)]
    pub fn widgets(&self) -> api::Widgets {
        api::Widgets::new()
    }

    #[napi(getter)]
    pub fn media(&self) -> api::Media {
        api::Media::from_manager(self.media_manager.clone())
    }

    #[napi(getter)]
    pub fn audio(&self) -> api::Audio {
        api::Audio::from_manager(self.media_manager.clone())
    }

    #[napi(getter)]
    pub fn keybinds(&self) -> api::Keybinds {
        api::Keybinds::from_manager(self.keybinds_manager.clone())
    }

    #[napi(getter)]
    pub fn mpris(&self) -> api::Mpris {
        #[cfg(target_os = "linux")]
        return api::Mpris::from_manager(self.mpris_manager.clone());

        #[cfg(target_os = "windows")]
        return api::Mpris::new_dummy();
    }

    #[napi]
    pub fn attach_logger<'env>(
        &self,
        #[napi(ts_arg_type = "(err: Error | null, rawMsg: string) => void")] callback: Function<'env, Unknown<'static>, ()>,
    ) -> Result<()> {
        let rx = self
            .log_rx
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| napi::Error::from_reason("Logger already attached"))?;

        let tsfn = callback
            .build_threadsafe_function::<String>()
            .callee_handled::<true>()
            .build_callback(|ctx| Ok(ctx.value))?;

        std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                tsfn.call(Ok(msg), napi::threadsafe_function::ThreadsafeFunctionCallMode::Blocking);
            }
        });

        Ok(())
    }

    #[napi]
    pub fn sleep(&self, ms: i64) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }

    #[napi]
    pub fn on<'env>(&self, ev: String, #[napi(ts_arg_type = "(...args: any[]) => any")] callback: Function<'env, Unknown<'static>, ()>) -> Result<()> {
        self.event_emitter.register_js_listener(&ev, callback)?;
        Ok(())
    }

    #[napi]
    pub fn cleanup(&self) {
        self.keybinds_manager.cleanup();
    }
}
