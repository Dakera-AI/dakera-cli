//! Configuration management for Dakera CLI

use std::env;

/// CLI Configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub default_namespace: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".to_string(),
            default_namespace: "default".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn load() -> Self {
        Self {
            server_url: env::var("DAKERA_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            default_namespace: env::var("DAKERA_NAMESPACE")
                .unwrap_or_else(|_| "default".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server_url, "http://localhost:3000");
        assert_eq!(config.default_namespace, "default");
    }
}
