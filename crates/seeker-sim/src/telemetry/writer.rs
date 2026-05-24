//! Write per-run CSV artifacts under `data/output/{run_id}/`.

use crate::domain::TrackState;
use crate::telemetry::record::TrackRecord;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TrackState;
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
}
