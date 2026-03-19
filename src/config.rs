//! Configuration management for Dakera CLI

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A single named server profile stored in the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub url: String,
    #[serde(default = "default_namespace")]
    pub default_namespace: String,
}

fn default_namespace() -> String {
    "default".to_string()
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            url: "http://localhost:3000".to_string(),
            default_namespace: "default".to_string(),
        }
    }
}

/// On-disk TOML config structure.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default = "default_profile_name")]
    pub active_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_profile_name() -> String {
    "default".to_string()
}

/// Runtime config resolved from file + env overrides.
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
    /// Return the path to the config file (`~/.config/dakera/config.toml`).
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("dakera").join("config.toml"))
    }

    /// Load config: file → env overrides.
    pub fn load() -> Self {
        let mut cfg = Config::default();

        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(file_cfg) = toml::from_str::<ConfigFile>(&content) {
                    let active = &file_cfg.active_profile;
                    if let Some(profile) = file_cfg.profiles.get(active) {
                        cfg.server_url = profile.url.clone();
                        cfg.default_namespace = profile.default_namespace.clone();
                    }
                }
            }
        }

        if let Ok(url) = env::var("DAKERA_URL") {
            cfg.server_url = url;
        }
        if let Ok(ns) = env::var("DAKERA_NAMESPACE") {
            cfg.default_namespace = ns;
        }

        cfg
    }

    /// Persist a profile to the config file.
    /// If this is the first profile, it becomes the active profile.
    pub fn write_profile(name: &str, profile: Profile) -> anyhow::Result<()> {
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file_cfg = if path.exists() {
            let content = fs::read_to_string(&path)?;
            toml::from_str::<ConfigFile>(&content).unwrap_or_default()
        } else {
            ConfigFile::default()
        };

        let is_first = file_cfg.profiles.is_empty();
        file_cfg.profiles.insert(name.to_string(), profile);
        if is_first {
            file_cfg.active_profile = name.to_string();
        }

        fs::write(&path, toml::to_string_pretty(&file_cfg)?)?;
        Ok(())
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

    #[test]
    fn test_config_file_roundtrip() {
        let mut cfg = ConfigFile::default();
        cfg.active_profile = "prod".to_string();
        cfg.profiles.insert(
            "prod".to_string(),
            Profile {
                url: "https://api.example.com".to_string(),
                default_namespace: "agents".to_string(),
            },
        );

        let serialized = toml::to_string_pretty(&cfg).unwrap();
        let deserialized: ConfigFile = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.active_profile, "prod");
        let profile = deserialized.profiles.get("prod").unwrap();
        assert_eq!(profile.url, "https://api.example.com");
        assert_eq!(profile.default_namespace, "agents");
    }
}
