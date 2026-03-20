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
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default = "default_profile_name")]
    pub active_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_profile_name() -> String {
    "default".to_string()
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            active_profile: default_profile_name(),
            profiles: HashMap::new(),
        }
    }
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
    /// Return the path to the config file (`~/.dakera/config.toml`).
    pub fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|d| d.join(".dakera").join("config.toml"))
    }

    /// Load config using the active profile, then apply env overrides.
    pub fn load() -> Self {
        Self::load_inner(None)
    }

    /// Load config forcing a specific named profile, then apply env overrides.
    pub fn load_with_profile(profile_name: &str) -> Self {
        Self::load_inner(Some(profile_name))
    }

    fn load_inner(profile_override: Option<&str>) -> Self {
        let mut cfg = Config::default();

        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(file_cfg) = toml::from_str::<ConfigFile>(&content) {
                    let active = profile_override
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| file_cfg.active_profile.clone());
                    if let Some(profile) = file_cfg.profiles.get(&active) {
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

    /// Read the raw on-disk config file (for profile listing / inspection).
    pub fn read_config_file() -> anyhow::Result<ConfigFile> {
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        if !path.exists() {
            return Ok(ConfigFile::default());
        }
        let content = fs::read_to_string(&path)?;
        Ok(toml::from_str::<ConfigFile>(&content).unwrap_or_default())
    }

    /// Persist a profile to the config file.
    /// If this is the first profile, it becomes the active profile.
    pub fn write_profile(name: &str, profile: Profile) -> anyhow::Result<()> {
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

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

    /// Switch the active profile in the config file.
    pub fn use_profile(name: &str) -> anyhow::Result<()> {
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file_cfg = if path.exists() {
            let content = fs::read_to_string(&path)?;
            toml::from_str::<ConfigFile>(&content).unwrap_or_default()
        } else {
            ConfigFile::default()
        };

        if !file_cfg.profiles.contains_key(name) {
            anyhow::bail!(
                "Profile '{}' not found. Run `dk config profile list` to see available profiles.",
                name
            );
        }

        file_cfg.active_profile = name.to_string();
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
    fn test_config_path_is_home_dakera() {
        if let Some(path) = Config::config_path() {
            let path_str = path.to_string_lossy();
            assert!(
                path_str.contains(".dakera"),
                "Config path should be under ~/.dakera, got: {}",
                path_str
            );
        }
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

    #[test]
    fn test_profile_default_values() {
        let p = Profile::default();
        assert_eq!(p.url, "http://localhost:3000");
        assert_eq!(p.default_namespace, "default");
    }

    #[test]
    fn test_config_file_default_active_profile() {
        let cfg = ConfigFile::default();
        assert_eq!(cfg.active_profile, "default");
        assert!(cfg.profiles.is_empty());
    }

    #[test]
    fn test_config_file_multiple_profiles_roundtrip() {
        let mut cfg = ConfigFile::default();
        cfg.active_profile = "staging".to_string();
        cfg.profiles.insert(
            "prod".to_string(),
            Profile {
                url: "https://prod.example.com".to_string(),
                default_namespace: "prod-ns".to_string(),
            },
        );
        cfg.profiles.insert(
            "staging".to_string(),
            Profile {
                url: "https://staging.example.com".to_string(),
                default_namespace: "staging-ns".to_string(),
            },
        );
        let serialized = toml::to_string_pretty(&cfg).unwrap();
        let deserialized: ConfigFile = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.active_profile, "staging");
        assert_eq!(deserialized.profiles.len(), 2);
        assert_eq!(
            deserialized.profiles["prod"].url,
            "https://prod.example.com"
        );
        assert_eq!(
            deserialized.profiles["staging"].default_namespace,
            "staging-ns"
        );
    }

    #[test]
    fn test_profile_default_namespace_missing_from_toml() {
        // When default_namespace is absent, serde should fill in "default"
        let toml_str = r#"url = "https://example.com""#;
        let profile: Profile = toml::from_str(toml_str).unwrap();
        assert_eq!(profile.default_namespace, "default");
    }

    #[test]
    fn test_config_file_active_profile_defaults_when_absent() {
        // A config file with no active_profile field should default to "default"
        let toml_str = "[profiles.prod]\nurl = \"https://prod.example.com\"\n";
        let cfg: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.active_profile, "default");
    }

    #[test]
    fn test_config_file_nonexistent_active_profile_yields_no_match() {
        // active_profile points to a name not in profiles — load_inner would fall back to defaults
        let toml_str =
            "active_profile = \"missing\"\n[profiles.prod]\nurl = \"https://prod.example.com\"\n";
        let cfg: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.active_profile, "missing");
        assert!(cfg.profiles.get("missing").is_none());
    }

    #[test]
    fn test_config_load_returns_config_type() {
        // Smoke test: load() returns a Config without panicking
        let cfg = Config::load();
        assert!(!cfg.server_url.is_empty());
        assert!(!cfg.default_namespace.is_empty());
    }
}
