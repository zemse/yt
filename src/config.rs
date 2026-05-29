//! Configuration: optional API key + defaults, resolved from flag → env →
//! `~/.yt` file. No OAuth is ever used.

use crate::error::{Result, YtError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invidious_instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookies_from_browser: Option<String>,
}

/// Resolved runtime configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: Option<String>,
    pub invidious_instance: String,
}

pub const DEFAULT_INVIDIOUS: &str = "https://yewtu.be";

/// Path to the config file: `$YT_CONFIG` if set, else `~/.yt`.
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("YT_CONFIG") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".yt")
}

pub fn load_file() -> ConfigFile {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => ConfigFile::default(),
    }
}

/// Resolve config. Precedence for the API key: `--api-key` flag → `YT_API_KEY`
/// env (handled by clap) → `~/.yt`. Invidious instance: flag → file → default.
pub fn resolve(api_key_flag: Option<String>, invidious_flag: Option<String>) -> Config {
    let file = load_file();
    let api_key = api_key_flag
        .filter(|s| !s.is_empty())
        .or(file.api_key.filter(|s| !s.is_empty()));
    let invidious_instance = invidious_flag
        .or(file.invidious_instance)
        .unwrap_or_else(|| DEFAULT_INVIDIOUS.to_string());
    Config {
        api_key,
        invidious_instance,
    }
}

/// Require an API key or return a helpful auth error.
pub fn require_api_key(cfg: &Config) -> Result<&str> {
    cfg.api_key.as_deref().ok_or_else(|| {
        YtError::Auth(
            "this command needs a YouTube Data API key. Set one with \
             `yt config set api-key <KEY>`, the YT_API_KEY env var, or --api-key."
                .into(),
        )
    })
}

/// Persist a single field to the config file, preserving other fields.
pub fn set_field(field: &str, value: &str) -> Result<PathBuf> {
    let mut file = load_file();
    match field {
        "api-key" | "api_key" => file.api_key = Some(value.to_string()),
        "invidious-instance" | "invidious_instance" => {
            file.invidious_instance = Some(value.to_string())
        }
        "cookies-from-browser" | "cookies_from_browser" => {
            file.cookies_from_browser = Some(value.to_string())
        }
        other => return Err(YtError::Input(format!("unknown config field: {other}"))),
    }
    let path = config_path();
    let toml = toml::to_string_pretty(&file)
        .map_err(|e| YtError::Other(format!("serialize config: {e}")))?;
    std::fs::write(&path, toml)?;
    set_permissions_0600(&path);
    Ok(path)
}

pub fn get_field(field: &str) -> Result<Option<String>> {
    let file = load_file();
    Ok(match field {
        "api-key" | "api_key" => file.api_key,
        "invidious-instance" | "invidious_instance" => file.invidious_instance,
        "cookies-from-browser" | "cookies_from_browser" => file.cookies_from_browser,
        other => return Err(YtError::Input(format!("unknown config field: {other}"))),
    })
}

/// Mask a secret for display: keep first/last few chars.
pub fn mask(secret: &str) -> String {
    let n = secret.len();
    if n <= 8 {
        "*".repeat(n)
    } else {
        format!("{}…{}", &secret[..4], &secret[n - 4..])
    }
}

#[cfg(unix)]
fn set_permissions_0600(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_permissions_0600(_path: &std::path::Path) {}
