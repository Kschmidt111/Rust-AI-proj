//! Write per-run CSV artifacts under `data/output/{run_id}/`.

use crate::domain::TrackState;
use crate::telemetry::record::{GuidanceRecord, SimRecord, TrackRecord};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Plain plot failure message as a proper [`std::error::Error`] for `thiserror` chaining.
#[derive(Debug)]
pub struct PlotRenderError(String);

impl PlotRenderError {
    /// Wraps a human-readable plotters failure message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for PlotRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlotRenderError {}

/// Errors while creating run output directories or writing CSV files.
#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("failed to create output directory '{path}': {source}")]
    CreateDir {
        path: PathBuf,
        source: io::Error,
    },

    #[error("failed to write telemetry file '{path}': {source}")]
    Write {
        path: PathBuf,
        source: io::Error,
    },

    #[error("cannot plot empty sim data")]
    EmptyPlotData,

    #[error("failed to render plot '{path}'")]
    Plot {
        path: PathBuf,
        #[source]
        source: PlotRenderError,
    },
}

/// Generates a unique run folder name (no external UUID dependency).
///
/// # Returns
/// String like `run_1716572487445123456` (unix nanos).
pub fn new_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("run_{nanos}")
}

/// Appends track rows to `{output_dir}/{run_id}/tracks.csv`.
///
/// # C# analogy
/// `StreamWriter` opened once with a header row, then one `WriteLine` per track update.
pub struct TracksCsvWriter {
    path: PathBuf,
    file: File,
    row_count: usize,
}

impl TracksCsvWriter {
    /// Creates `{output_root}/{run_id}/` and opens `tracks.csv` with a header row.
    ///
    /// # Arguments
    /// * `output_root` — base directory (e.g. `data/output` from config).
    /// * `run_id` — subfolder name for this run.
    pub fn create(output_root: &Path, run_id: &str) -> Result<Self, TelemetryError> {
        let run_dir = output_root.join(run_id);
        fs::create_dir_all(&run_dir).map_err(|source| TelemetryError::CreateDir {
            path: run_dir.clone(),
            source,
        })?;

        let path = run_dir.join("tracks.csv");
        let mut file = File::create(&path).map_err(|source| TelemetryError::Write {
            path: path.clone(),
            source,
        })?;

        writeln!(file, "{}", TrackRecord::csv_header()).map_err(|source| TelemetryError::Write {
            path: path.clone(),
            source,
        })?;

        Ok(Self {
            path,
            file,
            row_count: 0,
        })
    }

    /// Path to the open `tracks.csv` file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Number of data rows written (excluding header).
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Appends one track state row.
    pub fn write_track(&mut self, state: &TrackState) -> Result<(), TelemetryError> {
        let record = TrackRecord::from(state);
        writeln!(self.file, "{}", record.to_csv_row()).map_err(|source| TelemetryError::Write {
            path: self.path.clone(),
            source,
        })?;
        self.row_count += 1;
        Ok(())
    }

    /// Flushes and returns the CSV path.
    pub fn finish(mut self) -> Result<PathBuf, TelemetryError> {
        self.file.flush().map_err(|source| TelemetryError::Write {
            path: self.path.clone(),
            source,
        })?;
        Ok(self.path)
    }
}

/// Writes all track states from a slice to a new run CSV.
///
/// # Returns
/// Path to `tracks.csv` and the `run_id` folder name used.
pub fn write_tracks_csv(
    output_root: &Path,
    run_id: &str,
    states: &[TrackState],
) -> Result<(PathBuf, usize), TelemetryError> {
    let mut writer = TracksCsvWriter::create(output_root, run_id)?;
    for state in states {
        writer.write_track(state)?;
    }
    let path = writer.finish()?;
    Ok((path, states.len()))
}

/// Paths to all intercept run CSV artifacts.
#[derive(Debug, Clone)]
pub struct InterceptCsvPaths {
    pub tracks_csv: PathBuf,
    pub guidance_csv: PathBuf,
    pub sim_csv: PathBuf,
    pub trajectory_png: PathBuf,
    pub track_rows: usize,
    pub guidance_rows: usize,
    pub sim_rows: usize,
}

/// Writes `tracks.csv`, `guidance.csv`, and `sim.csv` under `{output_root}/{run_id}/`.
pub fn write_intercept_csvs(
    output_root: &Path,
    run_id: &str,
    tracks: &[TrackState],
    guidance: &[GuidanceRecord],
    sim: &[SimRecord],
) -> Result<InterceptCsvPaths, TelemetryError> {
    let (tracks_csv, track_rows) = write_tracks_csv(output_root, run_id, tracks)?;

    let run_dir = output_root.join(run_id);
    let guidance_csv = write_guidance_csv(&run_dir, guidance)?;
    let sim_csv = write_sim_csv(&run_dir, sim)?;
    let trajectory_png = run_dir.join("trajectory.png");
    crate::telemetry::plot::write_trajectory_png(&trajectory_png, sim, guidance)?;

    Ok(InterceptCsvPaths {
        tracks_csv,
        guidance_csv,
        sim_csv,
        trajectory_png,
        track_rows,
        guidance_rows: guidance.len(),
        sim_rows: sim.len(),
    })
}

fn write_guidance_csv(run_dir: &Path, rows: &[GuidanceRecord]) -> Result<PathBuf, TelemetryError> {
    let path = run_dir.join("guidance.csv");
    let mut file = File::create(&path).map_err(|source| TelemetryError::Write {
        path: path.clone(),
        source,
    })?;
    writeln!(file, "{}", GuidanceRecord::csv_header()).map_err(|source| TelemetryError::Write {
        path: path.clone(),
        source,
    })?;
    for row in rows {
        writeln!(file, "{}", row.to_csv_row()).map_err(|source| TelemetryError::Write {
            path: path.clone(),
            source,
        })?;
    }
    file.flush().map_err(|source| TelemetryError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn write_sim_csv(run_dir: &Path, rows: &[SimRecord]) -> Result<PathBuf, TelemetryError> {
    let path = run_dir.join("sim.csv");
    let mut file = File::create(&path).map_err(|source| TelemetryError::Write {
        path: path.clone(),
        source,
    })?;
    writeln!(file, "{}", SimRecord::csv_header()).map_err(|source| TelemetryError::Write {
        path: path.clone(),
        source,
    })?;
    for row in rows {
        writeln!(file, "{}", row.to_csv_row()).map_err(|source| TelemetryError::Write {
            path: path.clone(),
            source,
        })?;
    }
    file.flush().map_err(|source| TelemetryError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TrackState;
    use crate::telemetry::record::{GuidanceRecord, SimRecord};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_output() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("seeker_telemetry_{nanos}"))
    }

    fn sample_state(frame: u64) -> TrackState {
        TrackState {
            track_id: 1,
            frame_index: frame,
            position: (10.0 + frame as f32, 20.0),
            velocity: (100.0, 30.0),
            los: 0.0,
            los_rate: 0.0,
            coast_count: 0,
        }
    }

    #[test]
    fn writes_header_and_rows() {
        let root = temp_output();
        let run_id = "test_run";

        let (path, count) =
            write_tracks_csv(&root, run_id, &[sample_state(1), sample_state(2)]).expect("write");

        assert_eq!(count, 2);
        assert!(path.ends_with("tracks.csv"));

        let text = fs::read_to_string(&path).expect("read csv");
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], TrackRecord::csv_header());
        assert!(lines[1].starts_with("1,1,"));
        assert!(lines[2].starts_with("2,1,"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn writes_intercept_csv_bundle() {
        let root = temp_output();
        let run_id = "intercept_run";

        let guidance = vec![GuidanceRecord {
            frame_index: 1,
            track_id: 1,
            los: 0.05,
            los_rate: -0.01,
            law: "pn".into(),
            commanded_lateral_accel: 3.0,
        }];
        let sim = vec![SimRecord {
            frame_index: 1,
            time_s: 0.033,
            interceptor_x: 0.0,
            interceptor_y: 0.0,
            target_x: 100.0,
            target_y: 10.0,
            interceptor_vx: 150.0,
            interceptor_vy: 0.0,
            miss_distance: 100.5,
        }];

        let paths = write_intercept_csvs(
            &root,
            run_id,
            &[sample_state(1)],
            &guidance,
            &sim,
        )
        .expect("write intercept");

        assert_eq!(paths.track_rows, 1);
        assert_eq!(paths.guidance_rows, 1);
        assert_eq!(paths.sim_rows, 1);
        assert!(paths.tracks_csv.exists());
        assert!(paths.guidance_csv.exists());
        assert!(paths.sim_csv.exists());
        assert!(paths.trajectory_png.exists());
        assert!(fs::metadata(&paths.trajectory_png).unwrap().len() > 500);

        let _ = fs::remove_dir_all(&root);
    }
}
