//! CLI command implementations

pub mod admin;
pub mod admin_v012;
pub mod agent;
pub mod attachment;
pub mod capabilities;
pub mod completion;
pub mod config;
pub mod health;
pub mod index;
pub mod init;
pub mod keys;
pub mod knowledge;
pub mod memory;
pub mod namespace;
pub mod session;
pub mod text;

/// The longest wait for the next bytes of an answer.
pub(crate) const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(330);

/// Build a reqwest client that forwards DAKERA_API_KEY as a Bearer token when set.
pub(crate) fn authed_client() -> dakera_client::reqwest::Client {
    use dakera_client::reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
    let mut headers = HeaderMap::new();
    if let Ok(key) = std::env::var("DAKERA_API_KEY") {
        if let Ok(mut v) = HeaderValue::from_str(&format!("Bearer {key}")) {
            v.set_sensitive(true);
            headers.insert(AUTHORIZATION, v);
        }
    }
    dakera_client::reqwest::Client::builder()
        .user_agent(format!("dakera-cli/{}", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        // A server that does not answer must not hang the command: 10 s to
        // connect, and at most READ_TIMEOUT between two reads (longer than
        // the 300 s an `encryption-rotate --wait-secs` may ask the server to
        // wait before it answers).
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(READ_TIMEOUT)
        .build()
        .unwrap_or_default()
}
