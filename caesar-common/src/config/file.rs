use crate::error::{CommonError, ConfigError};
use crate::theme::Theme;
use std::path::PathBuf;

fn default_theme() -> String {
    "catppuccin-mocha".to_string()
}

/// Workspace-level caesar configuration.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CaesarConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub custom_themes: Vec<Theme>,
    #[serde(default)]
    pub plugin_paths: Vec<PathBuf>,
}

impl Default for CaesarConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            custom_themes: Vec::new(),
            plugin_paths: Vec::new(),
        }
    }
}

/// Return the caesar config directory, respecting XDG_CONFIG_HOME.
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("caesar"))
}

/// Load the workspace-level config from `~/.config/caesar/config.toml`.
/// Returns defaults if the file doesn't exist.
pub fn load_workspace_config() -> Result<CaesarConfig, CommonError> {
    let path = match config_dir().map(|d| d.join("config.toml")) {
        Some(p) => p,
        None => return Ok(CaesarConfig::default()),
    };
    if !path.exists() {
        return Ok(CaesarConfig::default());
    }
    let raw = std::fs::read_to_string(&path).map_err(|source| {
        CommonError::Config(ConfigError::Read {
            path: path.clone(),
            source,
        })
    })?;
    let config: CaesarConfig = toml::from_str(&raw).map_err(|source| {
        CommonError::Config(ConfigError::Parse {
            path: path.clone(),
            source,
        })
    })?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_mocha_theme() {
        let config = CaesarConfig::default();
        assert_eq!(config.theme, "catppuccin-mocha");
        assert!(config.custom_themes.is_empty());
        assert!(config.plugin_paths.is_empty());
    }

    #[test]
    fn missing_config_returns_default() {
        // load_workspace_config with a non-existent path returns defaults
        let config = load_workspace_config().unwrap();
        // This will return default if no config file exists at the XDG path
        assert_eq!(config.theme, "catppuccin-mocha");
    }

    #[test]
    fn config_dir_returns_some() {
        // In most environments HOME is set
        if std::env::var("HOME").is_ok() {
            assert!(config_dir().is_some());
        }
    }
}
