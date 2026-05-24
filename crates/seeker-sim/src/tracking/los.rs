//! Line-of-sight bearing and rate from filtered track position (Phase 4).
//!
//! Convention (locked here — see ARCHITECTURE §5):
//! - Image origin top-left, +x right, +y down.
//! - Seeker reference = image center.
//! - `los = atan2(dx, dy)` radians from +y axis toward target.

/// Bearing from image center to `position` (radians).
///
/// # Arguments
/// * `position` — filtered target `(x, y)` in pixels.
/// * `image_width` / `image_height` — frame size for center reference.
///
/// # Returns
/// Line-of-sight angle; `0` when target is straight below center (+y).
///
/// # C# analogy
/// `Math.Atan2(dx, dy)` on offsets from a fixed seeker boresight point.
pub fn line_of_sight(
    position: (f32, f32),
    image_width: u32,
    image_height: u32,
) -> f32 {
    let center_x = image_width as f32 / 2.0;
    let center_y = image_height as f32 / 2.0;
    let dx = position.0 - center_x;
    let dy = position.1 - center_y;
    dx.atan2(dy)
}

/// Stateful finite-difference LOS rate estimator.
///
/// Call [`Self::update`] once per frame after Kalman update with the same `dt`
/// used by the tracker.
#[derive(Debug, Clone, Default)]
pub struct LosEstimator {
    prev_los: Option<f32>,
}

impl LosEstimator {
    /// Creates an estimator with no prior sample.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears history (call on track loss or re-acquire).
    pub fn reset(&mut self) {
        self.prev_los = None;
    }

    /// Computes `(los, los_rate)` for this frame.
    ///
    /// # Arguments
    /// * `position` — filtered target center (pixels).
    /// * `image_width` / `image_height` — frame dimensions.
    /// * `dt` — seconds since previous frame.
    ///
    /// # Returns
    /// `(los, los_rate)` in radians and rad/s; rate is `0` on the first sample.
    pub fn update(
        &mut self,
        position: (f32, f32),
        image_width: u32,
        image_height: u32,
        dt: f32,
    ) -> (f32, f32) {
        let los = line_of_sight(position, image_width, image_height);
        let los_rate = match (self.prev_los, dt > 0.0) {
            (Some(prev), true) => (los - prev) / dt,
            _ => 0.0,
        };
        self.prev_los = Some(los);
        (los, los_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    #[test]
    fn target_at_center_has_zero_los() {
        let los = line_of_sight((320.0, 240.0), 640, 480);
        assert!(los.abs() < 1e-5);
    }

    #[test]
    fn target_to_the_right_is_positive_los() {
        let los = line_of_sight((420.0, 240.0), 640, 480);
        assert!(los > 0.0);
    }

    #[test]
    fn target_down_and_right_matches_atan2_dx_dy() {
        let los = line_of_sight((420.0, 340.0), 640, 480);
        assert!((los - FRAC_PI_4).abs() < 0.01);
    }

    #[test]
    fn constant_horizontal_motion_yields_positive_los_rate() {
        let dt = 1.0 / 30.0;
        let mut est = LosEstimator::new();
        // Slightly below center so dy > 0 and los changes as x increases.
        let y = 250.0;

        est.update((400.0, y), 640, 480, dt);
        let (_, rate) = est.update((410.0, y), 640, 480, dt);

        assert!(rate > 0.0, "los_rate={rate}");
    }
}
