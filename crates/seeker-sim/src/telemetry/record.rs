//! One CSV row schema for `tracks.csv` (Phase 4E) and Phase 5 guidance/sim rows.

use crate::domain::TrackState;
use crate::sim::SimState;

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

/// Single row in `guidance.csv` — PN command for one frame (Phase 5C).
#[derive(Debug, Clone, PartialEq)]
pub struct GuidanceRecord {
    pub frame_index: u64,
    pub track_id: u64,
    pub los: f32,
    pub los_rate: f32,
    /// `"pn"` or `"pp"` from config.
    pub law: String,
    pub commanded_lateral_accel: f32,
}

impl GuidanceRecord {
    /// CSV header line for `guidance.csv`.
    pub fn csv_header() -> &'static str {
        "frame_index,track_id,los,los_rate,law,commanded_lateral_accel"
    }

    /// Formats this row as a comma-separated line (no trailing newline).
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{:.6},{:.6},{},{:.6}",
            self.frame_index,
            self.track_id,
            self.los,
            self.los_rate,
            self.law,
            self.commanded_lateral_accel,
        )
    }
}

/// Single row in `sim.csv` — interceptor/target snapshot for one frame (Phase 5C).
#[derive(Debug, Clone, PartialEq)]
pub struct SimRecord {
    pub frame_index: u64,
    pub time_s: f64,
    pub interceptor_x: f32,
    pub interceptor_y: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub interceptor_vx: f32,
    pub interceptor_vy: f32,
    pub miss_distance: f32,
}

impl SimRecord {
    /// CSV header line for `sim.csv`.
    pub fn csv_header() -> &'static str {
        "frame_index,time_s,interceptor_x,interceptor_y,target_x,target_y,interceptor_vx,interceptor_vy,miss_distance"
    }

    /// Formats this row as a comma-separated line (no trailing newline).
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{:.6},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}",
            self.frame_index,
            self.time_s,
            self.interceptor_x,
            self.interceptor_y,
            self.target_x,
            self.target_y,
            self.interceptor_vx,
            self.interceptor_vy,
            self.miss_distance,
        )
    }
}

impl SimRecord {
    /// Builds a row from a frame index and [`SimState`] snapshot.
    pub fn from_snapshot(frame_index: u64, state: &SimState) -> Self {
        Self {
            frame_index,
            time_s: state.time_s,
            interceptor_x: state.interceptor.x,
            interceptor_y: state.interceptor.y,
            target_x: state.target.x,
            target_y: state.target.y,
            interceptor_vx: state.interceptor_vel.x,
            interceptor_vy: state.interceptor_vel.y,
            miss_distance: state.miss_distance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_csv_row_format() {
        let row = GuidanceRecord {
            frame_index: 3,
            track_id: 1,
            los: 0.1,
            los_rate: -0.02,
            law: "pn".into(),
            commanded_lateral_accel: 6.0,
        };
        assert!(row.to_csv_row().starts_with("3,1,0.100000,-0.020000,pn,6.000000"));
    }

    #[test]
    fn sim_csv_row_from_snapshot() {
        use crate::sim::{SeekerState, SimState, TargetState, Vec2};

        let snap = SimState::from_parts(
            0.033,
            SeekerState {
                position: Vec2::new(0.0, 0.0),
                velocity: Vec2::new(100.0, 5.0),
            },
            TargetState {
                position: Vec2::new(400.0, 50.0),
                velocity: Vec2::ZERO,
            },
        );
        let row = SimRecord::from_snapshot(1, &snap);
        assert!(row.to_csv_row().starts_with("1,0.033000,0.000,0.000,400.000,50.000"));
    }

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
