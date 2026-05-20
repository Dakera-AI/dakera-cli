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
