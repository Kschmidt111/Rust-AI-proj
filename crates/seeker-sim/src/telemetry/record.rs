//! One CSV row schema for `tracks.csv` (Phase 4E).

use crate::domain::TrackState;

/// Single row in `tracks.csv` — filtered track state for one frame.
///
/// # C# analogy
/// A flat DTO for CSV export, like a `TrackRow` used with `CsvWriter`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackRecord {
    pub frame_index: u64,
    pub track_id: u64,
    pub pos_x: f32,
    pub pos_y: f32,
    pub vel_x: f32,
    pub vel_y: f32,
    pub los: f32,
    pub los_rate: f32,
    pub coast_count: u32,
}

impl TrackRecord {
    /// CSV header line for `tracks.csv`.
    pub fn csv_header() -> &'static str {
        "frame_index,track_id,pos_x,pos_y,vel_x,vel_y,los,los_rate,coast_count"
    }

    /// Formats this row as a comma-separated line (no trailing newline).
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{:.3},{:.3},{:.3},{:.3},{:.6},{:.6},{}",
            self.frame_index,
            self.track_id,
            self.pos_x,
            self.pos_y,
            self.vel_x,
            self.vel_y,
            self.los,
            self.los_rate,
            self.coast_count,
        )
    }
}

impl From<&TrackState> for TrackRecord {
    fn from(state: &TrackState) -> Self {
        Self {
            frame_index: state.frame_index,
            track_id: state.track_id,
            pos_x: state.position.0,
            pos_y: state.position.1,
            vel_x: state.velocity.0,
            vel_y: state.velocity.1,
            los: state.los,
            los_rate: state.los_rate,
            coast_count: state.coast_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_row_contains_track_fields() {
        let state = TrackState {
            track_id: 1,
            frame_index: 5,
            position: (100.5, 200.25),
            velocity: (90.0, 30.0),
            los: 0.0,
            los_rate: 0.0,
            coast_count: 0,
        };
        let row = TrackRecord::from(&state);
        assert!(row.to_csv_row().starts_with("5,1,100.500,200.250,90.000,30.000"));
    }
}
