//! Configuration management for Dakera CLI

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

        write_private(&path, &toml::to_string_pretty(&file_cfg)?)?;
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
        write_private(&path, &toml::to_string_pretty(&file_cfg)?)?;
        Ok(())
    }
}

/// Write the config file readable by its owner only (`0600`, in a `0700`
/// directory when this creates it): server URLs may carry credentials, and
/// the file is where a token would go. An existing file is narrowed to `0600`
/// as well. On non-Unix systems the file is written with default permissions.
pub fn write_private(path: &Path, contents: &str) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)?;
            }
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        // `mode` applies only when the file is created.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
        Ok(())
    }
}

/// A warning when the config file at `path` can be read by other users
/// (the "other" read bit, or the group's), with the command that fixes it.
/// `None` when it cannot (or does not exist, or on non-Unix systems).
pub fn permission_warning(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path).ok()?.permissions().mode() & 0o777;
        if mode & 0o044 == 0 {
            return None;
        }
        Some(format!(
            "{} is readable by other users (mode {mode:o}); run `chmod 600 {}`",
            path.display(),
            path.display()
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Whether `url` sends requests in clear text to another machine: `http://`
/// to a host that is not the loopback interface. An API key sent there can be
/// read on the network.
pub fn is_cleartext_remote(url: &str) -> bool {
    let Some(rest) = url.trim().strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let host = match host_port.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host_port.split(':').next().unwrap_or(""),
    };
    let host = host.to_ascii_lowercase();
    let loopback = host == "localhost"
        || host.ends_with(".localhost")
        || host == "::1"
        || host.starts_with("127.");
    !loopback
}

/// `url` with the password of any `user:password@` part replaced by `***`.
pub fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    match authority.rsplit_once('@') {
        Some((userinfo, host)) => {
            let user = userinfo.split(':').next().unwrap_or("");
            let masked = if userinfo.contains(':') {
                format!("{user}:***")
            } else {
                "***".to_string()
            };
            format!("{scheme}://{masked}@{host}{tail}")
        }
        None => url.to_string(),
    }
}

/// An API key as `config show` prints it: its first 3 and last 4 characters,
/// or only `****` when it is too short to show any of it safely.
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 24 {
        return format!("**** ({} characters)", chars.len());
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail} ({} characters)", chars.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn config_file_is_written_owner_only_and_narrowed() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("dk-config-perm-{}", std::process::id()));
        let path = dir.join("sub").join("config.toml");
        write_private(&path, "active_profile = \"default\"\n").unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        assert!(permission_warning(&path).is_none());

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let warning = permission_warning(&path).unwrap();
        assert!(warning.contains("chmod 600"), "{warning}");
        write_private(&path, "x = 1\n").unwrap();
        assert_eq!(mode(&path), 0o600);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keys_and_url_passwords_are_masked() {
        let key = "dk_live_abcdefghijklmnop1234";
        let masked = mask_key(key);
        assert!(masked.starts_with("dk_…1234"), "{masked}");
        assert!(!masked.contains("abcdefgh"));
        assert_eq!(mask_key("short"), "**** (5 characters)");
        assert_eq!(
            redact_url("https://me:s3cret@db.example.com:3000/x"),
            "https://me:***@db.example.com:3000/x"
        );
        assert_eq!(redact_url("http://localhost:3000"), "http://localhost:3000");
    }

    #[test]
    fn cleartext_remote_urls_are_told_apart() {
        assert!(is_cleartext_remote("http://dakera.example.com:3000"));
        assert!(is_cleartext_remote("http://10.1.2.3"));
        assert!(!is_cleartext_remote("https://dakera.example.com"));
        assert!(!is_cleartext_remote("http://localhost:3000"));
        assert!(!is_cleartext_remote("http://127.0.0.1:3000/"));
        assert!(!is_cleartext_remote("http://[::1]:3000"));
    }

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
        let mut cfg = ConfigFile {
            active_profile: "prod".to_string(),
            ..Default::default()
        };
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
        let mut cfg = ConfigFile {
            active_profile: "staging".to_string(),
            ..Default::default()
        };
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
        assert!(!cfg.profiles.contains_key("missing"));
    }

    #[test]
    fn test_config_load_returns_config_type() {
        // Smoke test: load() returns a Config without panicking
        let cfg = Config::load();
        assert!(!cfg.server_url.is_empty());
        assert!(!cfg.default_namespace.is_empty());
    }
}
