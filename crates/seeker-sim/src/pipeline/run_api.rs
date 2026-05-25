//! Folder pipeline runs for HTTP API (Phase 6D).
//!
//! `POST /v1/runs` calls the same functions as CLI `track` / `intercept`.

use crate::config::AppConfig;
use crate::pipeline::{intercept_motion_folder, track_motion_folder, InterceptSummary, MotionTrackSummary, PipelineError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

const ALLOWED_ARTIFACTS: &[&str] = &["tracks.csv", "guidance.csv", "sim.csv", "trajectory.png"];

/// Request body for `POST /v1/runs`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CreateRunRequest {
    /// Path to a folder of PNG/JPEG frames (relative to repo root or absolute).
    pub input_path: String,
    /// `"track"` or `"intercept"` (default `"intercept"`).
    #[serde(default = "default_run_mode")]
    pub mode: String,
}

fn default_run_mode() -> String {
    "intercept".to_string()
}

/// Artifact download URLs returned to the client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RunArtifacts {
    pub tracks_csv: Option<String>,
    pub guidance_csv: Option<String>,
    pub sim_csv: Option<String>,
    pub trajectory_png: Option<String>,
}

/// Summary returned after a successful folder run or from `GET /v1/runs/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunStatusResponse {
    pub run_id: String,
    /// `"completed"` when `tracks.csv` exists under the run folder.
    pub status: String,
    pub mode: Option<String>,
    pub input_path: Option<String>,
    pub frame_count: Option<u32>,
    pub track_row_count: Option<u32>,
    pub guidance_row_count: Option<u32>,
    pub sim_row_count: Option<u32>,
    pub min_miss_distance: Option<f32>,
    pub artifacts: RunArtifacts,
}

/// Errors for folder run API helpers.
#[derive(Debug, Error)]
pub enum FolderRunError {
    #[error("invalid input path: {0}")]
    InvalidInputPath(String),

    #[error("unknown run mode '{0}' — expected 'track' or 'intercept'")]
    UnknownMode(String),

    #[error("invalid run id '{0}'")]
    InvalidRunId(String),

    #[error("artifact not allowed: '{0}'")]
    InvalidArtifact(String),

    #[error("run not found: {0}")]
    RunNotFound(String),

    #[error(transparent)]
    Pipeline(#[from] PipelineError),

    #[error("failed to read run status: {0}")]
    Io(#[from] std::io::Error),
}

/// Repository root (`Rust AI proj/`).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Resolves and validates `input_path` stays inside the repository.
///
/// Tries paths relative to the **repo root** and to the **current working directory**
/// (so `../../data/frames/...` works when invoked from `crates/seeker-sim`).
///
/// # Returns
/// Canonical absolute path to the frame folder.
pub fn resolve_input_path(raw: &str) -> Result<PathBuf, FolderRunError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(FolderRunError::InvalidInputPath(
            "input_path must not be empty".into(),
        ));
    }

    let root = repo_root()
        .canonicalize()
        .map_err(|e| FolderRunError::InvalidInputPath(e.to_string()))?;

    let candidate = PathBuf::from(raw);
    let mut attempts: Vec<PathBuf> = Vec::new();

    if candidate.is_absolute() {
        attempts.push(candidate);
    } else {
        attempts.push(root.join(&candidate));
        if let Ok(cwd) = std::env::current_dir() {
            let from_cwd = cwd.join(&candidate);
            if !attempts.contains(&from_cwd) {
                attempts.push(from_cwd);
            }
        }
    }

    for absolute in attempts {
        let Ok(canonical) = absolute.canonicalize() else {
            continue;
        };
        if !canonical.starts_with(&root) {
            continue;
        }
        if canonical.is_dir() {
            return Ok(canonical);
        }
    }

    Err(FolderRunError::InvalidInputPath(format!(
        "path not found: {raw} — from repo root use e.g. data/frames/dot_run_001; run .\\scripts\\generate-dot-video.ps1 if missing"
    )))
}

/// Returns true when `run_id` matches `run_<digits>`.
pub fn is_valid_run_id(run_id: &str) -> bool {
    let Some(suffix) = run_id.strip_prefix("run_") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
}

/// Runs motion track or intercept pipeline on a validated frame folder.
///
/// # Arguments
/// * `config` — Application config (output dir, guidance, tracking).
/// * `folder` — Canonical path to PNG/JPEG frames.
/// * `mode` — `"track"` or `"intercept"`.
pub fn run_folder_pipeline(
    config: &AppConfig,
    folder: &Path,
    mode: &str,
) -> Result<RunStatusResponse, FolderRunError> {
    let input_path = folder.display().to_string();

    match mode.to_ascii_lowercase().as_str() {
        "intercept" => {
            let summary = intercept_motion_folder(config, folder)?;
            Ok(intercept_to_status(summary, input_path))
        }
        "track" => {
            let summary = track_motion_folder(config, folder)?;
            Ok(track_to_status(summary, input_path))
        }
        other => Err(FolderRunError::UnknownMode(other.to_string())),
    }
}

/// Loads run status from `data/output/{run_id}/` without re-running the pipeline.
pub fn load_run_status(config: &AppConfig, run_id: &str) -> Result<RunStatusResponse, FolderRunError> {
    if !is_valid_run_id(run_id) {
        return Err(FolderRunError::InvalidRunId(run_id.to_string()));
    }

    let output_root = config.paths.resolve_output_dir();
    let run_dir = output_root.join(run_id);

    if !run_dir.is_dir() {
        return Err(FolderRunError::RunNotFound(run_id.to_string()));
    }

    let tracks = run_dir.join("tracks.csv");
    if !tracks.is_file() {
        return Err(FolderRunError::RunNotFound(run_id.to_string()));
    }

    let guidance = run_dir.join("guidance.csv");
    let sim = run_dir.join("sim.csv");
    let _trajectory = run_dir.join("trajectory.png");

    let mode = if guidance.is_file() {
        Some("intercept".to_string())
    } else {
        Some("track".to_string())
    };

    let track_row_count: u32 = csv_data_row_count(&tracks)?;
    let guidance_row_count: Option<u32> = if guidance.is_file() {
        Some(csv_data_row_count(&guidance)?)
    } else {
        None
    };
    let sim_row_count: Option<u32> = if sim.is_file() {
        Some(csv_data_row_count(&sim)?)
    } else {
        None
    };

    let min_miss_distance: Option<f32> = if sim.is_file() {
        read_min_miss_from_sim_csv(&sim)?
    } else {
        None
    };

    Ok(RunStatusResponse {
        run_id: run_id.to_string(),
        status: "completed".to_string(),
        mode,
        input_path: None,
        frame_count: Some(track_row_count),
        track_row_count: Some(track_row_count),
        guidance_row_count,
        sim_row_count,
        min_miss_distance,
        artifacts: artifacts_for_run(run_id, &run_dir),
    })
}

/// Resolves a single artifact file under a run directory.
pub fn resolve_artifact_path(
    config: &AppConfig,
    run_id: &str,
    file_name: &str,
) -> Result<PathBuf, FolderRunError> {
    if !is_valid_run_id(run_id) {
        return Err(FolderRunError::InvalidRunId(run_id.to_string()));
    }

    if !ALLOWED_ARTIFACTS.contains(&file_name) {
        return Err(FolderRunError::InvalidArtifact(file_name.to_string()));
    }

    let path = config.paths.resolve_output_dir().join(run_id).join(file_name);

    if !path.is_file() {
        return Err(FolderRunError::RunNotFound(format!(
            "{run_id}/{file_name}"
        )));
    }

    Ok(path)
}

fn intercept_to_status(summary: InterceptSummary, input_path: String) -> RunStatusResponse {
    RunStatusResponse {
        run_id: summary.run_id.clone(),
        status: "completed".to_string(),
        mode: Some("intercept".to_string()),
        input_path: Some(input_path),
        frame_count: Some(summary.frame_count as u32),
        track_row_count: Some(summary.track_row_count as u32),
        guidance_row_count: Some(summary.guidance_row_count as u32),
        sim_row_count: Some(summary.sim_row_count as u32),
        min_miss_distance: summary.min_miss_distance,
        artifacts: artifacts_from_paths(&summary.run_id, &summary),
    }
}

fn track_to_status(summary: MotionTrackSummary, input_path: String) -> RunStatusResponse {
    RunStatusResponse {
        run_id: summary.run_id.clone(),
        status: "completed".to_string(),
        mode: Some("track".to_string()),
        input_path: Some(input_path),
        frame_count: Some(summary.frame_count as u32),
        track_row_count: Some(summary.track_row_count as u32),
        guidance_row_count: None,
        sim_row_count: None,
        min_miss_distance: None,
        artifacts: RunArtifacts {
            tracks_csv: Some(artifact_url(&summary.run_id, "tracks.csv")),
            guidance_csv: None,
            sim_csv: None,
            trajectory_png: None,
        },
    }
}

fn artifacts_from_paths(run_id: &str, summary: &InterceptSummary) -> RunArtifacts {
    RunArtifacts {
        tracks_csv: artifact_url_if_exists(run_id, &summary.tracks_csv),
        guidance_csv: artifact_url_if_exists(run_id, &summary.guidance_csv),
        sim_csv: artifact_url_if_exists(run_id, &summary.sim_csv),
        trajectory_png: artifact_url_if_exists(run_id, &summary.trajectory_png),
    }
}

fn artifacts_for_run(run_id: &str, run_dir: &Path) -> RunArtifacts {
    RunArtifacts {
        tracks_csv: url_if_file(run_id, run_dir, "tracks.csv"),
        guidance_csv: url_if_file(run_id, run_dir, "guidance.csv"),
        sim_csv: url_if_file(run_id, run_dir, "sim.csv"),
        trajectory_png: url_if_file(run_id, run_dir, "trajectory.png"),
    }
}

fn url_if_file(run_id: &str, run_dir: &Path, name: &str) -> Option<String> {
    let path = run_dir.join(name);
    if path.is_file() {
        Some(artifact_url(run_id, name))
    } else {
        None
    }
}

fn artifact_url_if_exists(run_id: &str, path: &Path) -> Option<String> {
    if path.is_file() {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| artifact_url(run_id, name))
    } else {
        None
    }
}

fn artifact_url(run_id: &str, name: &str) -> String {
    format!("/v1/runs/{run_id}/artifacts/{name}")
}

/// Returns the number of data rows in a CSV file (header excluded).
fn csv_data_row_count(path: &Path) -> Result<u32, FolderRunError> {
    let content = std::fs::read_to_string(path)?;
    let rows = content.lines().filter(|l| !l.is_empty()).count();
    Ok(rows.saturating_sub(1) as u32)
}

fn read_min_miss_from_sim_csv(path: &Path) -> Result<Option<f32>, FolderRunError> {
    let content = std::fs::read_to_string(path)?;
    let mut min: Option<f32> = None;
    for (i, line) in content.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 9 {
            continue;
        }
        if let Ok(v) = cols[8].parse::<f32>() {
            min = Some(min.map_or(v, |m| m.min(v)));
        }
    }
    Ok(min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use image::{Rgb, RgbImage};
    use std::fs;
    use std::path::Path;

    const SKY: Rgb<u8> = Rgb([26, 58, 92]);

    fn write_dot_sequence(dir: &Path, count: usize) {
        fs::create_dir_all(dir).unwrap();
        for i in 0..count {
            let mut pixels = RgbImage::from_pixel(640, 480, SKY);
            let cx = 40 + i as i32 * 5;
            let cy = 120 + i as i32 * 3;
            for y in 0..480_i32 {
                for x in 0..640_i32 {
                    let dx = x - cx;
                    let dy = y - cy;
                    if dx * dx + dy * dy <= 36 {
                        pixels.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                    }
                }
            }
            pixels.save(dir.join(format!("{:04}.png", i + 1))).unwrap();
        }
    }

    #[test]
    fn resolve_input_path_rejects_outside_repo() {
        let err = resolve_input_path("C:\\Windows").unwrap_err();
        assert!(matches!(err, FolderRunError::InvalidInputPath(_)));
    }

    #[test]
    fn is_valid_run_id_checks_format() {
        assert!(is_valid_run_id("run_123"));
        assert!(!is_valid_run_id("run_"));
        assert!(!is_valid_run_id("../run_1"));
    }

    #[test]
    fn run_folder_intercept_writes_artifacts() {
        let frames_dir = repo_root().join("data/frames/run_api_test");
        if frames_dir.exists() {
            let _ = fs::remove_dir_all(&frames_dir);
        }
        write_dot_sequence(&frames_dir, 12);
        let config = AppConfig::load().expect("config");
        let folder = resolve_input_path("data/frames/run_api_test").expect("resolve");

        let status = run_folder_pipeline(&config, &folder, "intercept").expect("run");
        assert_eq!(status.status, "completed");
        assert_eq!(status.mode.as_deref(), Some("intercept"));
        assert!(status.artifacts.tracks_csv.is_some());
        assert!(status.artifacts.guidance_csv.is_some());

        let loaded = load_run_status(&config, &status.run_id).expect("load");
        assert_eq!(loaded.run_id, status.run_id);
        assert_eq!(loaded.track_row_count, status.track_row_count);

        let _ = fs::remove_dir_all(&frames_dir);
    }
}
