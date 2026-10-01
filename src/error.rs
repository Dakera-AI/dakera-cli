//! Structured CLI error types with typed exit codes.

use serde::Serialize;

use crate::api::ApiHttpError;

/// Typed CLI errors with distinct exit codes and machine-readable error codes.
///
/// Exit code mapping:
/// - 0  success
/// - 1  general / unknown error
/// - 2  connection error (server unreachable, TLS failure)
/// - 3  not found (namespace, vector, key, etc.)
/// - 4  permission denied / authentication failure
/// - 5  invalid input / validation error
/// - 6  server-side error (5xx, including 501 feature disabled / 503 busy)
///
/// A `413` (payload too large, hard quota exceeded) is an input error (5); a
/// `403` is a permission error (4), whether the key's scope is too low or the
/// key is pinned to namespaces on a node-wide route.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Invalid input: {0}")]
    Input(String),

    #[error("Server error: {0}")]
    Server(String),

    #[error("{0}")]
    Other(String),
}

impl CliError {
    /// The numeric exit code for this error variant.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Connection(_) => 2,
            CliError::NotFound(_) => 3,
            CliError::Permission(_) => 4,
            CliError::Input(_) => 5,
            CliError::Server(_) => 6,
            CliError::Other(_) => 1,
        }
    }

    /// A short uppercase machine-readable error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            CliError::Connection(_) => "CONNECTION_ERROR",
            CliError::NotFound(_) => "NOT_FOUND",
            CliError::Permission(_) => "PERMISSION_DENIED",
            CliError::Input(_) => "INVALID_INPUT",
            CliError::Server(_) => "SERVER_ERROR",
            CliError::Other(_) => "ERROR",
        }
    }
}

/// An error for input the command itself found invalid (exit code 5).
pub fn input_error(msg: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(CliError::Input(msg.into()))
}

/// JSON representation of an error emitted when `--format json` is active.
///
/// The `http_status`, `server_code`, `details` and `retry_after_secs` fields
/// are present only when the server answered with an error (they carry its
/// v0.12 JSON error body and `Retry-After` header).
#[derive(Serialize)]
pub struct JsonError<'a> {
    pub error: bool,
    pub code: &'a str,
    pub exit_code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
}

impl<'a> JsonError<'a> {
    /// Build the JSON error for `cli_err`, adding the server's error fields
    /// when `err` is (or wraps) an HTTP error answer.
    pub fn from_error(cli_err: &'a CliError, err: &anyhow::Error) -> Self {
        let api = err.downcast_ref::<ApiHttpError>();
        Self {
            error: true,
            code: cli_err.error_code(),
            exit_code: cli_err.exit_code(),
            message: cli_err.to_string(),
            http_status: api.map(|a| a.status),
            server_code: api.and_then(|a| a.code.clone()),
            details: api.and_then(|a| a.details.clone()),
            retry_after_secs: api.and_then(|a| a.retry_after_secs),
        }
    }
}

/// Classify an `anyhow::Error` into a `CliError` by inspecting its message.
pub fn classify(err: &anyhow::Error) -> CliError {
    let msg = err.to_string();

    // A validation failure raised by a command itself.
    if let Some(CliError::Input(m)) = err.downcast_ref::<CliError>() {
        return CliError::Input(m.clone());
    }

    // An HTTP error answer is classified by its status, not by its text.
    if let Some(api) = err.downcast_ref::<ApiHttpError>() {
        return by_status(api.status, msg);
    }

    // The same for the errors of the SDK-backed commands.
    if let Some(sdk) = err.downcast_ref::<dakera_client::ClientError>() {
        use dakera_client::ClientError as E;
        match sdk {
            E::Server { status, .. } | E::Authorization { status, .. } => {
                return by_status(*status, msg)
            }
            E::QuotaExceeded { .. } | E::PayloadTooLarge { .. } => return CliError::Input(msg),
            E::FeatureDisabled { .. } | E::NotImplemented { .. } | E::ServiceUnavailable { .. } => {
                return CliError::Server(msg)
            }
            E::NamespaceNotFound(_) | E::VectorNotFound(_) => return CliError::NotFound(msg),
            E::Connection(_) | E::Timeout => return CliError::Connection(msg),
            E::Http(e) if e.is_connect() || e.is_timeout() => return CliError::Connection(msg),
            _ => {}
        }
    }
    let msg_lower = msg.to_lowercase();

    if msg_lower.contains("connection refused")
        || msg_lower.contains("connection error")
        || msg_lower.contains("failed to connect")
        || msg_lower.contains("tcp connect")
        || msg_lower.contains("dns error")
        || msg_lower.contains("tls")
        || msg_lower.contains("hyper")
        || msg_lower.contains("reqwest")
    {
        CliError::Connection(msg)
    } else if msg_lower.contains("not found") || msg_lower.contains("404") {
        CliError::NotFound(msg)
    } else if msg_lower.contains("unauthorized")
        || msg_lower.contains("forbidden")
        || msg_lower.contains("401")
        || msg_lower.contains("403")
    {
        CliError::Permission(msg)
    } else if msg_lower.contains("500")
        || msg_lower.contains("502")
        || msg_lower.contains("503")
        || msg_lower.contains("server error")
        || msg_lower.contains("internal error")
    {
        CliError::Server(msg)
    } else {
        CliError::Other(msg)
    }
}

/// Exit-code class of an HTTP error status.
fn by_status(status: u16, msg: String) -> CliError {
    match status {
        401 | 403 => CliError::Permission(msg),
        404 => CliError::NotFound(msg),
        400 | 409 | 413 | 415 | 422 => CliError::Input(msg),
        500..=599 => CliError::Server(msg),
        _ => CliError::Other(msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_codes_are_distinct() {
        assert_eq!(CliError::Connection("x".into()).exit_code(), 2);
        assert_eq!(CliError::NotFound("x".into()).exit_code(), 3);
        assert_eq!(CliError::Permission("x".into()).exit_code(), 4);
        assert_eq!(CliError::Input("x".into()).exit_code(), 5);
        assert_eq!(CliError::Server("x".into()).exit_code(), 6);
        assert_eq!(CliError::Other("x".into()).exit_code(), 1);
    }

    #[test]
    fn test_error_codes_are_strings() {
        assert_eq!(
            CliError::Connection("x".into()).error_code(),
            "CONNECTION_ERROR"
        );
        assert_eq!(CliError::NotFound("x".into()).error_code(), "NOT_FOUND");
        assert_eq!(
            CliError::Permission("x".into()).error_code(),
            "PERMISSION_DENIED"
        );
        assert_eq!(CliError::Input("x".into()).error_code(), "INVALID_INPUT");
        assert_eq!(CliError::Server("x".into()).error_code(), "SERVER_ERROR");
        assert_eq!(CliError::Other("x".into()).error_code(), "ERROR");
    }

    #[test]
    fn test_classify_connection_refused() {
        let err = anyhow::anyhow!("error sending request: connection refused");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Connection(_)));
        assert_eq!(cli_err.exit_code(), 2);
    }

    #[test]
    fn test_classify_not_found() {
        let err = anyhow::anyhow!("namespace not found");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::NotFound(_)));
        assert_eq!(cli_err.exit_code(), 3);
    }

    #[test]
    fn test_classify_unauthorized() {
        let err = anyhow::anyhow!("401 Unauthorized");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Permission(_)));
        assert_eq!(cli_err.exit_code(), 4);
    }

    #[test]
    fn test_classify_server_error() {
        let err = anyhow::anyhow!("500 internal server error");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Server(_)));
        assert_eq!(cli_err.exit_code(), 6);
    }

    #[test]
    fn test_classify_other() {
        let err = anyhow::anyhow!("something unexpected happened");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Other(_)));
        assert_eq!(cli_err.exit_code(), 1);
    }

    #[test]
    fn test_json_error_serializes() {
        let err = anyhow::anyhow!("Connection error: refused");
        let cli_err = classify(&err);
        let json_err = JsonError::from_error(&cli_err, &err);
        let s = serde_json::to_string(&json_err).unwrap();
        assert!(s.contains("\"error\":true"));
        assert!(s.contains("\"exit_code\":2"));
        assert!(s.contains("CONNECTION_ERROR"));
        assert!(!s.contains("http_status"));
    }

    fn http_err(status: u16, retry: Option<u64>, code: &str) -> anyhow::Error {
        let body = serde_json::json!({ "error": "refused", "code": code });
        let status = dakera_client::reqwest::StatusCode::from_u16(status).unwrap();
        anyhow::Error::new(ApiHttpError::from_parts(status, retry, &body.to_string()))
    }

    #[test]
    fn test_classify_http_403_is_permission() {
        let err = http_err(403, None, "NAMESPACE_ACCESS_DENIED");
        assert!(matches!(classify(&err), CliError::Permission(_)));
    }

    #[test]
    fn test_classify_http_413_is_input() {
        let err = http_err(413, None, "PAYLOAD_TOO_LARGE");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Input(_)));
        assert_eq!(cli_err.exit_code(), 5);
    }

    #[test]
    fn test_classify_http_501_is_server() {
        let err = http_err(501, None, "FEATURE_DISABLED");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Server(_)));
        assert_eq!(cli_err.exit_code(), 6);
    }

    #[test]
    fn test_classify_http_503_is_server_and_keeps_retry_after() {
        let err = http_err(503, Some(7), "SERVICE_UNAVAILABLE");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Server(_)));
        let json_err = JsonError::from_error(&cli_err, &err);
        let s = serde_json::to_string(&json_err).unwrap();
        assert!(s.contains("\"retry_after_secs\":7"));
        assert!(s.contains("\"http_status\":503"));
        assert!(s.contains("\"server_code\":\"SERVICE_UNAVAILABLE\""));
    }

    #[test]
    fn test_classify_input_error_keeps_exit_code_5() {
        let err = input_error("no such file");
        let cli_err = classify(&err);
        assert!(matches!(cli_err, CliError::Input(_)));
        assert_eq!(cli_err.exit_code(), 5);
    }

    #[test]
    fn test_classify_http_404_is_not_found() {
        let err = http_err(404, None, "VECTOR_NOT_FOUND");
        assert!(matches!(classify(&err), CliError::NotFound(_)));
    }

    #[test]
    fn test_classify_http_error_survives_context() {
        let err = http_err(403, None, "INSUFFICIENT_SCOPE").context("while restoring");
        assert!(matches!(classify(&err), CliError::Permission(_)));
    }
}
