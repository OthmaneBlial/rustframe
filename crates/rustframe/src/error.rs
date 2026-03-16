use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("missing embedded assets")]
    MissingAssets,
    #[error("database capability is not enabled for this app")]
    DatabaseUnavailable,
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("timed out: {0}")]
    TimedOut(String),
    #[error("record not found: {0}")]
    RecordNotFound(String),
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Time(#[from] time::error::Format),
    #[cfg(feature = "desktop")]
    #[error(transparent)]
    Window(#[from] tao::error::OsError),
    #[cfg(feature = "desktop")]
    #[error(transparent)]
    WebView(#[from] wry::Error),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
