use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::RuntimeError;

pub const DEFAULT_MAX_IPC_REQUEST_BYTES: usize = 1024 * 1024;

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

pub fn decode_request(body: &[u8], max_bytes: usize) -> Result<IpcRequest, RuntimeError> {
    if body.len() > max_bytes {
        return Err(RuntimeError::RequestTooLarge(format!(
            "IPC request exceeds the {max_bytes} byte limit"
        )));
    }
    serde_json::from_slice(body).map_err(RuntimeError::Json)
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
            RuntimeError::RequestTooLarge(_) => Self {
                code: "request_too_large",
                message: error.to_string(),
            },
            RuntimeError::RateLimited(_) => Self {
                code: "rate_limited",
                message: error.to_string(),
            },
            RuntimeError::TimedOut(_) => Self {
                code: "timeout",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_rejects_oversized_and_malformed_requests() {
        assert!(matches!(
            decode_request(br#"{"id":1}"#, 4),
            Err(RuntimeError::RequestTooLarge(_))
        ));
        assert!(matches!(
            decode_request(b"not-json", DEFAULT_MAX_IPC_REQUEST_BYTES),
            Err(RuntimeError::Json(_))
        ));
    }

    #[test]
    fn decoder_accepts_a_well_formed_request() {
        let request = decode_request(
            br#"{"id":7,"method":"db.info","params":{}}"#,
            DEFAULT_MAX_IPC_REQUEST_BYTES,
        )
        .unwrap();
        assert_eq!(request.id, 7);
        assert_eq!(request.method, "db.info");
    }
}
