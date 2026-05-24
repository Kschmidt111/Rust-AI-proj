//! Single-target tracker — wraps Kalman and emits [`TrackState`] each frame.

use crate::domain::TrackState;
use crate::tracking::KalmanFilter2d;

/// Tracks one point target using a constant-velocity Kalman filter.
///
/// Call [`Self::step`] once per frame with an optional centroid measurement.
/// When no measurement arrives, the filter coasts and `coast_count` rises.
///
/// # C# analogy
/// A small stateful service holding `_trackId`, `_kalman`, and `_coastFrames`
/// — similar to a single-target tracker class in a WPF/sim loop.
#[derive(Debug, Clone)]
pub struct PointTracker {
    track_id: u64,
    filter: KalmanFilter2d,
    coast_count: u32,
    max_coast_frames: u32,
}

impl PointTracker {
    /// Starts a new track at the given image position.
    ///
    /// # Arguments
    /// * `track_id` — stable identifier for telemetry (usually `1` for single-target demo).
    /// * `x` — initial horizontal position (pixels).
    /// * `y` — initial vertical position (pixels).
    ///
    /// # Returns
    /// Tracker using the default coast limit of 15 frames.
    pub fn new(track_id: u64, x: f32, y: f32) -> Self {
        Self::with_coast_limit(track_id, x, y, 15)
    }

    /// Same as [`Self::new`] but with an explicit coast limit from config.
    ///
    /// # Arguments
    /// * `max_coast_frames` — drop the track after this many consecutive missed measurements.
    pub fn with_coast_limit(track_id: u64, x: f32, y: f32, max_coast_frames: u32) -> Self {
        Self {
            track_id,
            filter: KalmanFilter2d::new(x, y),
            coast_count: 0,
            max_coast_frames,
        }
    }

    /// Stable track identifier.
    pub fn track_id(&self) -> u64 {
        self.track_id
    }

    /// `true` when coast count exceeds the configured maximum.
    pub fn is_lost(&self) -> bool {
        self.coast_count > self.max_coast_frames
    }

    /// Filtered center `(x, y)` — use after [`Self::begin_frame`] as the association anchor.
    pub fn position(&self) -> (f32, f32) {
        self.filter.position()
    }

    /// Prediction step only — call before associating a measurement to this frame.
    pub fn begin_frame(&mut self, dt: f32) {
        self.filter.predict(dt);
    }

    /// Update (or coast) after association, then return the frame [`TrackState`].
    pub fn finish_frame(
        &mut self,
        frame_index: u64,
        measurement: Option<(f32, f32)>,
    ) -> TrackState {
        match measurement {
            Some((mx, my)) => {
                self.filter.update(mx, my);
                self.coast_count = 0;
            }
            None => {
                self.coast_count += 1;
            }
        }

        let (x, y) = self.filter.position();
        let (vx, vy) = self.filter.velocity();

        TrackState {
            track_id: self.track_id,
            frame_index,
            position: (x, y),
            velocity: (vx, vy),
            los: 0.0,
            los_rate: 0.0,
            coast_count: self.coast_count,
        }
    }

    /// Advances the filter one frame and returns the current [`TrackState`].
    ///
    /// # Arguments
    /// * `frame_index` — zero-based frame number in the run.
    /// * `dt` — seconds since the previous frame (e.g. `1.0 / 30.0`).
    /// * `measurement` — `Some((x, y))` when a detection/centroid exists; `None` to coast.
    ///
    /// # Returns
    /// Snapshot suitable for logging or CSV export.
    pub fn step(
        &mut self,
        frame_index: u64,
        dt: f32,
        measurement: Option<(f32, f32)>,
    ) -> TrackState {
        self.begin_frame(dt);
        self.finish_frame(frame_index, measurement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_with_measurement_keeps_coast_count_at_zero() {
        let mut tracker = PointTracker::new(1, 10.0, 20.0);
        let state = tracker.step(0, 1.0 / 30.0, Some((10.0, 20.0)));

        assert_eq!(state.track_id, 1);
        assert_eq!(state.frame_index, 0);
        assert_eq!(state.coast_count, 0);
        assert!(state.position.0 > 0.0);
    }

    #[test]
    fn step_without_measurement_increments_coast_count() {
        let mut tracker = PointTracker::new(1, 0.0, 0.0);
        tracker.step(0, 1.0 / 30.0, Some((0.0, 0.0)));

        let state = tracker.step(1, 1.0 / 30.0, None);

        assert_eq!(state.coast_count, 1);
    }

    #[test]
    fn constant_velocity_run_produces_nonzero_velocity() {
        let dt = 1.0 / 30.0;
        let true_vx = 90.0;
        let true_vy = 30.0;
        let mut tracker = PointTracker::new(42, 0.0, 0.0);

        for frame in 0..40_u64 {
            let t = frame as f32 * dt;
            let mx = true_vx * t;
            let my = true_vy * t;
            tracker.step(frame, dt, Some((mx, my)));
        }

        let state = tracker.step(40, dt, Some((true_vx * 40.0 * dt, true_vy * 40.0 * dt)));

        assert_eq!(state.track_id, 42);
        assert!(state.velocity.0.abs() > 10.0);
        assert!(state.velocity.1.abs() > 5.0);
    }

    #[test]
    fn is_lost_after_max_coast_exceeded() {
        let mut tracker = PointTracker::with_coast_limit(1, 0.0, 0.0, 2);
        tracker.step(0, 1.0 / 30.0, Some((0.0, 0.0)));

        tracker.step(1, 1.0 / 30.0, None);
        assert!(!tracker.is_lost());

        tracker.step(2, 1.0 / 30.0, None);
        assert!(!tracker.is_lost());

        tracker.step(3, 1.0 / 30.0, None);
        assert!(tracker.is_lost());
    }
}
