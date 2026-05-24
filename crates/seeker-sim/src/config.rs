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
    /// Tracker settings (`[tracking]` in TOML).
    pub tracking: TrackingConfig,
    /// Guidance law settings (`[guidance]` in TOML) — Phase 5+.
    pub guidance: GuidanceConfig,
    /// Simulation timestep (`[sim]` in TOML) — used as frame `dt` until ingest exposes timestamps.
    pub sim: SimConfig,
    /// Output paths (`[paths]` in TOML).
    pub paths: PathsConfig,
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

/// `[tracking]` section — association and coast limits (Phase 4+).
#[derive(Debug, Clone, Deserialize)]
pub struct TrackingConfig {
    /// Minimum IoU to match a YOLO detection to a track (bbox path).
    pub iou_match_threshold: f32,
    /// Drop track after this many frames without a measurement.
    pub max_coast_frames: u32,
    /// Max pixel distance to associate a motion centroid to a predicted track point.
    pub point_match_distance_px: f32,
    /// Half-width of ROI motion search window (pixels) while tracking.
    pub roi_half_size_px: u32,
    /// Brightness-change threshold for motion differencing `[0.0, 1.0]`.
    pub motion_threshold: f32,
}

/// `[guidance]` section — PN / pure pursuit parameters (Phase 5+).
#[derive(Debug, Clone, Deserialize)]
pub struct GuidanceConfig {
    /// `"pp"` = pure pursuit, `"pn"` = proportional navigation.
    pub law: String,
    /// Navigation constant `N` for PN (dimensionless).
    pub navigation_constant: f32,
    /// Closing velocity `V_c` for PN (sim units).
    pub closing_velocity: f32,
}

impl GuidanceConfig {
    /// Returns true when config selects proportional navigation.
    pub fn is_pn(&self) -> bool {
        self.law.eq_ignore_ascii_case("pn")
    }

    /// Returns true when config selects pure pursuit.
    pub fn is_pp(&self) -> bool {
        self.law.eq_ignore_ascii_case("pp")
    }
}

/// `[sim]` section — simulation timing (Phase 5+); `dt_seconds` reused for tracking steps.
#[derive(Debug, Clone, Deserialize)]
pub struct SimConfig {
    /// Seconds per frame step (~30 fps default).
    pub dt_seconds: f32,
    /// Initial miss distance in sim units (Phase 5).
    pub initial_miss_distance: f32,
}

/// `[paths]` section — where run artifacts are written.
#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    /// Base directory for per-run output (e.g. `data/output`).
    pub output_dir: String,
}

impl PathsConfig {
    /// Resolves `output_dir` relative to repo root when not absolute.
    pub fn resolve_output_dir(&self) -> PathBuf {
        let path = PathBuf::from(&self.output_dir);
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
        assert_eq!(config.tracking.max_coast_frames, 15);
        assert_eq!(config.tracking.roi_half_size_px, 32);
        assert_eq!(config.guidance.law, "pn");
        assert!((config.guidance.navigation_constant - 3.0).abs() < 1e-6);
        assert!((config.guidance.closing_velocity - 100.0).abs() < 1e-6);
        assert!(config.guidance.is_pn());
        assert!(!config.guidance.is_pp());
        assert!((config.sim.dt_seconds - 0.033).abs() < 1e-6);
        assert_eq!(config.paths.output_dir, "data/output");
        assert!(config.server.socket_addr().is_ok());
    }
}
