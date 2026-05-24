//! Core types produced by the vision pipeline.

use serde::Serialize;

/// Axis-aligned bounding box in **original image pixel coordinates**.
///
/// # C# analogy
/// A small record/DTO like `record BBox(float X1, float Y1, float X2, float Y2)`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl BBox {
    /// Center point `(cx, cy)` — useful for tracking and guidance later.
    pub fn center(&self) -> (f32, f32) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }
}

/// One object detection from a single frame.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Detection {
    pub class_id: u32,
    pub class_name: String,
    pub confidence: f32,
    pub bbox: BBox,
}

/// Filtered target state for one frame (Phase 4+).
///
/// Produced by the tracker after Kalman predict/update. Line-of-sight fields
/// stay at `0.0` until `tracking/los.rs` is wired in a later sub-step.
///
/// # C# analogy
/// A DTO returned from a tracking service — like `TrackSnapshot` with id,
/// position, velocity, and coast metadata for telemetry CSV rows.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TrackState {
    /// Stable id for this target across frames.
    pub track_id: u64,
    /// Zero-based frame index in the current run.
    pub frame_index: u64,
    /// Filtered center `(x, y)` in image pixels.
    pub position: (f32, f32),
    /// Filtered velocity `(vx, vy)` in pixels per second.
    pub velocity: (f32, f32),
    /// Bearing from seeker reference to target (radians); filled by `los.rs` later.
    pub los: f32,
    /// Rate of change of line-of-sight (rad/s); filled by `los.rs` later.
    pub los_rate: f32,
    /// Consecutive frames without a measurement (Kalman coast).
    pub coast_count: u32,
}
