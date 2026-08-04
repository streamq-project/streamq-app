use napi::bindgen_prelude::{JsObjectValue, PromiseRaw, ToNapiValue};
use std::collections::HashMap;
use std::future::Future;
use strum_macros::EnumDiscriminants;
use thiserror::Error;

#[cfg(target_os = "windows")]
use windows::core::Error as WindowsError;

#[derive(Error, Debug, EnumDiscriminants)]
#[strum_discriminants(name(NativeErrorCode))]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(napi(string_enum = "UPPER_SNAKE"))]
pub enum NativeError {
    #[error("Media session error: {0}")]
    MediaSession(String),

    #[error("D-Bus connection failed: {0}")]
    DbusConnection(String),

    #[error("Player not found: {0}")]
    PlayerNotFound(String),

    #[error("Invalid volume: {0}. Must be between 0.0 and 1.0")]
    InvalidVolume(f64),

    #[error("Volume is not controllable for this player")]
    VolumeNotControllable,

    #[error("Portal error: {0}")]
    Portal(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Window effect failed: {0}")]
    WindowEffectFailed(String),

    #[error("Window not found")]
    WindowNotFound,

    #[error("Invalid window handle")]
    InvalidWindowHandle,

    #[error("Audio source not found: {0}")]
    AudioSourceNotFound(String),

    #[error("No sink inputs found for current app")]
    AppAudioStreamNotFound,

    #[error("Generic error: {0}")]
    Generic(String),
}

impl NativeError {
    pub fn code(&self) -> NativeErrorCode {
        NativeErrorCode::from(self)
    }

    pub fn context(&self) -> HashMap<String, String> {
        let mut ctx = HashMap::new();
        match self {
            NativeError::PlayerNotFound(app) => {
                ctx.insert("app".into(), app.clone());
            }
            NativeError::InvalidVolume(vol) => {
                ctx.insert("volume".into(), vol.to_string());
                ctx.insert("validRange".into(), "0.0 - 1.0".into());
            }
            NativeError::DbusConnection(d) | NativeError::MediaSession(d) | NativeError::Portal(d) | NativeError::OperationFailed(d) => {
                ctx.insert("details".into(), d.clone());
            }
            NativeError::VolumeNotControllable => {
                ctx.insert("reason".into(), "Player does not support volume control".into());
            }
            NativeError::WindowEffectFailed(details) => {
                ctx.insert("details".into(), details.clone());
            }
            NativeError::WindowNotFound | NativeError::InvalidWindowHandle | NativeError::AppAudioStreamNotFound => {}
            NativeError::AudioSourceNotFound(id) => {
                ctx.insert("source".into(), id.clone());
            }
            NativeError::Generic(details) => {
                ctx.insert("details".into(), details.clone());
            }
        }
        ctx
    }

    pub fn into_js_error(self, env: &napi::Env) -> napi::Error {
        let code = self.code();
        let message = self.to_string();
        let context = self.context();

        let mut js_error = match env.create_error(napi::Error::from_reason(message)) {
            Ok(error) => error,
            Err(error) => return error,
        };

        if let Err(error) = js_error.set_named_property("name", "NativeError") {
            return error;
        }

        if let Err(error) = js_error.set_named_property("code", code) {
            return error;
        }

        let js_context = match env.to_js_value(&context) {
            Ok(context) => context,
            Err(error) => return error,
        };

        if let Err(error) = js_error.set_named_property("context", js_context) {
            return error;
        }

        match js_error.into_unknown(env) {
            Ok(error) => napi::Error::from(error),
            Err(error) => error,
        }
    }
}

#[cfg(target_os = "linux")]
impl From<zbus::Error> for NativeError {
    fn from(err: zbus::Error) -> Self {
        NativeError::DbusConnection(err.to_string())
    }
}

#[cfg(target_os = "windows")]
impl From<WindowsError> for NativeError {
    fn from(err: WindowsError) -> Self {
        NativeError::MediaSession(err.to_string())
    }
}

impl From<anyhow::Error> for NativeError {
    fn from(err: anyhow::Error) -> Self {
        NativeError::OperationFailed(err.to_string())
    }
}

pub trait ResultExt<T> {
    fn into_napi(self, env: &napi::Env) -> napi::Result<T>;
}

impl<T, E: Into<NativeError>> ResultExt<T> for Result<T, E> {
    fn into_napi(self, env: &napi::Env) -> napi::Result<T> {
        self.map_err(|error| error.into().into_js_error(env))
    }
}

pub fn execute_napi_future<'env, T, E, F>(env: &'env napi::Env, future: F) -> napi::Result<PromiseRaw<'env, T>>
where
    T: 'static + Send + ToNapiValue,
    E: 'static + Send + Into<NativeError>,
    F: 'static + Send + Future<Output = Result<T, E>>,
{
    env.spawn_future_with_callback(async move { Ok(future.await) }, |env, result| result.into_napi(env))
}
