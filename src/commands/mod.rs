//! CLI command implementations

pub mod admin;
pub mod agent;
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
        .default_headers(headers)
        .build()
        .unwrap_or_default()
}
