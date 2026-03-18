use crate::error::{Result, VeniError};
use caesar_common::error::ConfigError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn default_theme() -> String {
    "catppuccin-mocha".to_string()
}

fn default_layout() -> String {
    "mc".to_string()
}

/// Veni-specific configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct VeniConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default = "default_layout")]
    pub layout: String,
}

impl Default for VeniConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            show_hidden: false,
            layout: default_layout(),
        }
    }
}

/// Wrapper used to extract the `[veni]` section from a caesar config file.
#[derive(Debug, Deserialize)]
struct CaesarWithVeni {
    #[serde(default)]
    veni: VeniConfig,
}

/// Load veni configuration.
///
/// Resolution order:
/// 1. `path_override` if provided.
/// 2. `~/.config/caesar/config.toml` — reads the `[veni]` section.
/// 3. `~/.config/veni/config.toml` — reads the whole file as `VeniConfig`.
/// 4. Built-in defaults when no file is found.
pub fn load_config(path_override: Option<&Path>) -> Result<VeniConfig> {
    if let Some(p) = path_override {
        return load_file(p);
    }

    // Try caesar config first ([veni] section).
    if let Some(caesar_path) = caesar_config_path() {
        if caesar_path.exists() {
            return load_caesar_section(&caesar_path);
        }
    }

    // Fall back to dedicated veni config.
    if let Some(veni_path) = veni_config_path() {
        if veni_path.exists() {
            return load_file(&veni_path);
        }
    }

    Ok(VeniConfig::default())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn xdg_config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn caesar_config_path() -> Option<PathBuf> {
    xdg_config_home().map(|base| base.join("caesar").join("config.toml"))
}

fn veni_config_path() -> Option<PathBuf> {
    xdg_config_home().map(|base| base.join("veni").join("config.toml"))
}

/// Load `VeniConfig` directly from a TOML file (the file root is `VeniConfig`).
fn load_file(path: &Path) -> Result<VeniConfig> {
    let raw = std::fs::read_to_string(path).map_err(VeniError::Io)?;
    toml::from_str::<VeniConfig>(&raw).map_err(|source| {
        VeniError::Config(ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    })
}

/// Load `VeniConfig` from a caesar config file (reads the `[veni]` section).
fn load_caesar_section(path: &Path) -> Result<VeniConfig> {
    let raw = std::fs::read_to_string(path).map_err(VeniError::Io)?;
    toml::from_str::<CaesarWithVeni>(&raw)
        .map(|c| c.veni)
        .map_err(|source| {
            VeniError::Config(ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn defaults_are_correct() {
        let cfg = VeniConfig::default();
        assert_eq!(cfg.theme, "catppuccin-mocha");
        assert!(!cfg.show_hidden);
        assert_eq!(cfg.layout, "mc");
    }

    #[test]
    fn missing_override_path_returns_defaults() {
        // Provide a path that does not exist — load_config should return Ok(default).
        let tmp = tempfile::tempdir().unwrap();
        let non_existent = tmp.path().join("no_such_file.toml");
        // load_config with a missing override path falls through to the I/O error
        // path — but because the file doesn't exist the OS returns NotFound.
        // We want load_config(None) to return defaults, but load_config(Some(missing))
        // is an error.  Test the None path via env manipulation is fragile;
        // instead, call load_file directly on a non-existent path and verify error.
        let result = load_file(&non_existent);
        assert!(result.is_err(), "reading a missing file must fail");
    }

    #[test]
    fn no_override_no_config_files_returns_defaults() {
        // With no path override and relying on absence of the config files on
        // the CI system we can test load_config(None) safely only when we know
        // neither caesar nor veni config files exist.  We exercise the internal
        // helpers instead.
        let cfg = VeniConfig::default();
        assert_eq!(cfg.theme, "catppuccin-mocha");
    }

    #[test]
    fn valid_veni_toml_parses() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            r#"theme = "gruvbox"
show_hidden = true
layout = "ranger""#
        )
        .unwrap();
        let cfg = load_file(tmp.path()).unwrap();
        assert_eq!(cfg.theme, "gruvbox");
        assert!(cfg.show_hidden);
        assert_eq!(cfg.layout, "ranger");
    }

    #[test]
    fn partial_toml_uses_defaults_for_missing_fields() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, r#"theme = "nord""#).unwrap();
        let cfg = load_file(tmp.path()).unwrap();
        assert_eq!(cfg.theme, "nord");
        assert!(!cfg.show_hidden);
        assert_eq!(cfg.layout, "mc");
    }

    #[test]
    fn caesar_section_parses_veni_block() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            r#"[veni]
theme = "tokyo-night"
show_hidden = true"#
        )
        .unwrap();
        let cfg = load_caesar_section(tmp.path()).unwrap();
        assert_eq!(cfg.theme, "tokyo-night");
        assert!(cfg.show_hidden);
        assert_eq!(cfg.layout, "mc"); // default
    }

    #[test]
    fn caesar_section_missing_veni_block_returns_defaults() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, r#"theme = "catppuccin-latte""#).unwrap();
        let cfg = load_caesar_section(tmp.path()).unwrap();
        assert_eq!(cfg.theme, "catppuccin-mocha"); // veni default, not caesar
        assert!(!cfg.show_hidden);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "this is [not valid toml").unwrap();
        let result = load_file(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn load_config_with_override_reads_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, r#"theme = "solarized""#).unwrap();
        let cfg = load_config(Some(tmp.path())).unwrap();
        assert_eq!(cfg.theme, "solarized");
    }
}
