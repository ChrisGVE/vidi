use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("cannot read file: {path}: {source}")]
    FileUnreadable {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("theme error: {0}")]
    Theme(String),

    #[error("detection error: {0}")]
    Detection(String),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

pub type Result<T> = std::result::Result<T, CommonError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn file_not_found_displays_path() {
        let err = CommonError::FileNotFound(PathBuf::from("/tmp/missing.txt"));
        assert!(err.to_string().contains("/tmp/missing.txt"));
    }

    #[test]
    fn file_unreadable_displays_path() {
        let err = CommonError::FileUnreadable {
            path: PathBuf::from("/tmp/locked.bin"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        };
        let msg = err.to_string();
        assert!(msg.contains("/tmp/locked.bin"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn theme_error_displays_message() {
        let err = CommonError::Theme("unknown theme 'xyz'".to_string());
        assert!(err.to_string().contains("xyz"));
    }

    #[test]
    fn detection_error_displays_message() {
        let err = CommonError::Detection("unrecognised format".to_string());
        assert!(err.to_string().contains("unrecognised format"));
    }

    #[test]
    fn config_read_error_displays_path() {
        let err = ConfigError::Read {
            path: PathBuf::from("/etc/caesar/config.toml"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("/etc/caesar/config.toml"));
    }

    #[test]
    fn config_parse_error_displays_path() {
        // Build a toml::de::Error by attempting to parse invalid TOML.
        let raw_err = toml::from_str::<toml::Value>("invalid = [").unwrap_err();
        let err = ConfigError::Parse {
            path: PathBuf::from("/etc/caesar/config.toml"),
            source: raw_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("/etc/caesar/config.toml"));
    }

    #[test]
    fn config_error_converts_into_common_error() {
        let raw_err = toml::from_str::<toml::Value>("bad = [").unwrap_err();
        let config_err = ConfigError::Parse {
            path: PathBuf::from("/tmp/cfg.toml"),
            source: raw_err,
        };
        let common: CommonError = config_err.into();
        assert!(common.to_string().contains("configuration error"));
    }
}
