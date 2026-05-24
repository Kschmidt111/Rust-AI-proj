//! Application configuration loaded from TOML files.
//!
//! Phase 1 reads `[server].bind` only. Additional `[vision]`, `[tracking]`, etc.
//! sections stay in the file but are wired in as those modules are implemented.

use serde::Deserialize;
use std::fs;
use std::net::{AddrParseError, SocketAddr};
use std::path::PathBuf;
use thiserror::Error;

/// Root config file shape — add fields as new phases need them.
///
/// # C# analogy
/// Like binding `appsettings.json` to an `IOptions<AppSettings>` POCO.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// HTTP server settings (`[server]` in TOML).
    pub server: ServerConfig,
    /// Vision / ONNX settings (`[vision]` in TOML).
    pub vision: VisionConfig,
}

/// `[server]` section — where the Axum listener binds.
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Host and port, e.g. `127.0.0.1:8080`.
    pub bind: String,
}

impl ServerConfig {
    /// Parses `bind` into a type-safe socket address for `TcpListener`.
    ///
    /// # C# analogy
    /// `IPEndPoint.Parse(configuration["server:bind"])`.
    pub fn socket_addr(&self) -> Result<SocketAddr, AddrParseError> {
        self.bind.parse()
    }
}

/// `[vision]` section — YOLO ONNX detector settings (Phase 2+).
#[derive(Debug, Clone, Deserialize)]
pub struct VisionConfig {
    /// Path to `.onnx` file (relative to repo root or absolute).
    pub model_path: String,
    /// Square input size (YOLOv8 default 640).
    pub input_size: u32,
    /// Drop detections below this score.
    pub confidence_threshold: f32,
    /// IoU threshold for non-maximum suppression.
    pub iou_threshold: f32,
    /// If non-empty, filter to this COCO class name only.
    pub target_class: String,
}

impl VisionConfig {
    /// Resolves `model_path` relative to repo root when not absolute.
    pub fn resolve_model_path(&self) -> PathBuf {
        let path = PathBuf::from(&self.model_path);
        if path.is_absolute() {
            return path;
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}

/// Errors while locating, reading, or parsing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file '{path}': {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file '{path}': {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

impl AppConfig {
    /// Loads config from `SEEKER_SIM_CONFIG` or the repo default file.
    ///
    /// # C# analogy
    /// `ConfigurationBuilder().AddJsonFile("appsettings.json").Build()`.
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_file_path();
        Self::load_from_path(&path)
    }

    /// Loads config from an explicit path (tests, custom deployments).
    pub fn load_from_path(path: &PathBuf) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.clone(),
            source,
        })?;

        toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: path.clone(),
            source,
        })
    }
}

/// Resolves which TOML file to load.
///
/// Priority:
/// 1. `SEEKER_SIM_CONFIG` environment variable
/// 2. `<repo>/config/default.toml` (relative to crate manifest)
fn config_file_path() -> PathBuf {
    if let Ok(custom) = std::env::var("SEEKER_SIM_CONFIG") {
        return PathBuf::from(custom);
    }

    default_config_path()
}

/// Default: `config/default.toml` when crate lives in `crates/seeker-sim`.
pub fn default_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/default.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let path = default_config_path();
        let config = AppConfig::load_from_path(&path).expect("default config should parse");
        assert_eq!(config.server.bind, "127.0.0.1:8080");
        assert_eq!(config.vision.input_size, 640);
        assert!(config.server.socket_addr().is_ok());
    }
}
