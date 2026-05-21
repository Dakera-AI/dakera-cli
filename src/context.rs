//! Shared request context threaded through every command execute() call.

use std::time::Instant;

use crate::OutputFormat;

/// Carries the server URL, output format, and verbose flag for a single
/// CLI invocation. Passed by reference into every command module.
pub struct Context {
    pub url: String,
    pub format: OutputFormat,
    pub verbose: bool,
}

impl Context {
    pub fn new(url: impl Into<String>, format: OutputFormat, verbose: bool) -> Self {
        Self {
            url: url.into(),
            format,
            verbose,
        }
    }

    /// Log an outgoing request and return the start time (used in `log_response`).
    /// No-op when verbose is false.
    pub fn log_request(&self, method: &str, path: &str) -> Instant {
        if self.verbose {
            tracing::info!("--> {} {}{}", method, self.url, path);
        }
        Instant::now()
    }

    /// Log the result of a request. `status` is the HTTP status string (e.g. "200 OK").
    /// No-op when verbose is false.
    pub fn log_response(&self, start: Instant, status: &str) {
        if self.verbose {
            tracing::info!("<-- {} ({:.0}ms)", status, start.elapsed().as_millis());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputFormat;

    #[test]
    fn context_new_stores_fields() {
        let ctx = Context::new("http://localhost:3000", OutputFormat::Json, true);
        assert_eq!(ctx.url, "http://localhost:3000");
        assert!(ctx.verbose);
    }

    #[test]
    fn context_non_verbose_log_request_returns_instant() {
        let ctx = Context::new("http://localhost:3000", OutputFormat::Table, false);
        let t = ctx.log_request("GET", "/health");
        // Instant::elapsed should always succeed
        assert!(t.elapsed().as_nanos() < 1_000_000_000);
    }

    #[test]
    fn context_verbose_log_response_does_not_panic() {
        let ctx = Context::new("http://localhost:3000", OutputFormat::Table, true);
        let t = ctx.log_request("POST", "/v1/memory/store");
        ctx.log_response(t, "200 OK");
    }

    #[test]
    fn context_non_verbose_log_response_does_not_panic() {
        let ctx = Context::new("http://localhost:3000", OutputFormat::Table, false);
        let t = ctx.log_request("GET", "/v1/namespaces");
        ctx.log_response(t, "ERR");
    }
}
