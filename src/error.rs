//! Plugin error type.

use thiserror::Error;

/// Errors raised by the Pixeldrain plugin.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Pixeldrain HTTP returned status {status}: {message}")]
    HttpStatus { status: u16, message: String },

    #[error("host function response invalid: {0}")]
    HostResponse(String),

    #[error("URL is not a recognised Pixeldrain resource: {0}")]
    UnsupportedUrl(String),

    #[error("Pixeldrain file is offline or removed: {0}")]
    Offline(String),

    #[error("Pixeldrain API rejected the request: {0}")]
    ApiError(String),
}
