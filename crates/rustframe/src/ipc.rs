use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::RuntimeError;

#[derive(Debug, Deserialize)]
pub struct IpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct IpcResponse {
    pub id: u64,
    pub ok: bool,
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcErrorResponse>,
}

#[derive(Debug, Serialize)]
pub struct IpcErrorResponse {
    pub code: &'static str,
    pub message: String,
}

impl IpcResponse {
    pub fn success(id: u64, data: Value) -> Self {
        Self {
            id,
            ok: true,
            data,
            error: None,
        }
    }

    pub fn failure(id: u64, error: &RuntimeError) -> Self {
        Self {
            id,
            ok: false,
            data: Value::Null,
            error: Some(IpcErrorResponse::from(error)),
        }
    }
}

impl From<&RuntimeError> for IpcErrorResponse {
    fn from(error: &RuntimeError) -> Self {
        match error {
            RuntimeError::MissingAssets => Self {
                code: "missing_assets",
                message: error.to_string(),
            },
            RuntimeError::DatabaseUnavailable => Self {
                code: "database_unavailable",
                message: error.to_string(),
            },
            RuntimeError::InvalidConfiguration(_) => Self {
                code: "invalid_configuration",
                message: error.to_string(),
            },
            RuntimeError::InvalidParameter(_) => Self {
                code: "invalid_parameter",
                message: error.to_string(),
            },
            RuntimeError::PermissionDenied(_) => Self {
                code: "permission_denied",
                message: error.to_string(),
            },
            RuntimeError::RecordNotFound(_) => Self {
                code: "not_found",
                message: error.to_string(),
            },
            RuntimeError::UnknownMethod(_) => Self {
                code: "unknown_method",
                message: error.to_string(),
            },
            RuntimeError::Database(_) => Self {
                code: "database_error",
                message: error.to_string(),
            },
            RuntimeError::Io(_) => Self {
                code: "io_error",
                message: error.to_string(),
            },
            RuntimeError::Json(_) => Self {
                code: "invalid_request",
                message: error.to_string(),
            },
            RuntimeError::Time(_) => Self {
                code: "time_error",
                message: error.to_string(),
            },
            #[cfg(feature = "desktop")]
            RuntimeError::Window(_) => Self {
                code: "window_error",
                message: error.to_string(),
            },
            #[cfg(feature = "desktop")]
            RuntimeError::WebView(_) => Self {
                code: "webview_error",
                message: error.to_string(),
            },
        }
    }
}
