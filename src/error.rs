//! Structured CLI error types with typed exit codes.

use serde::Serialize;

/// Typed CLI errors with distinct exit codes and machine-readable error codes.
///
/// Exit code mapping:
/// - 0  success
/// - 1  general / unknown error
/// - 2  connection error (server unreachable, TLS failure)
/// - 3  not found (namespace, vector, key, etc.)
/// - 4  permission denied / authentication failure
/// - 5  invalid input / validation error
/// - 6  server-side error (5xx)
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

/// JSON representation of an error emitted when `--format json` is active.
#[derive(Serialize)]
pub struct JsonError<'a> {
    pub error: bool,
    pub code: &'a str,
    pub exit_code: i32,
    pub message: String,
}

/// Classify an `anyhow::Error` into a `CliError` by inspecting its message.
pub fn classify(err: &anyhow::Error) -> CliError {
    let msg = err.to_string();
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
        let cli_err = CliError::Connection("refused".into());
        let json_err = JsonError {
            error: true,
            code: cli_err.error_code(),
            exit_code: cli_err.exit_code(),
            message: cli_err.to_string(),
        };
        let s = serde_json::to_string(&json_err).unwrap();
        assert!(s.contains("\"error\":true"));
        assert!(s.contains("\"exit_code\":2"));
        assert!(s.contains("CONNECTION_ERROR"));
    }
}
