//! 2D simulation state types (Phase 5B).
//!
//! Coordinates: **x right, y up** (math convention — differs from image top-left y-down).
//! Pipeline mapping from image bearing → sim plane happens in Phase 5C.

use serde::Serialize;

/// 2D vector in simulation plane (meters or arbitrary sim units).
///
/// # C# analogy
/// A lightweight `struct Vector2` like `System.Numerics.Vector2` without a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// Zero vector.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates `(x, y)`.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean length.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Returns `self + other`.
    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    /// Returns `self - other`.
    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    /// Returns `self * scalar`.
    pub fn scale(self, scalar: f32) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

/// Interceptor (seeker) position and velocity in the sim plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SeekerState {
    pub position: Vec2,
    pub velocity: Vec2,
}

/// Target position and constant velocity for kinematic stepping.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TargetState {
    pub position: Vec2,
    pub velocity: Vec2,
}

/// One snapshot of the simulation for telemetry / CSV (Phase 5C).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SimState {
    /// Simulated time in seconds.
    pub time_s: f64,
    pub interceptor: Vec2,
    pub target: Vec2,
    pub interceptor_vel: Vec2,
    /// Euclidean distance between interceptor and target (sim units).
    pub miss_distance: f32,
}

impl SimState {
    /// Builds a snapshot from engine components.
    pub fn from_parts(
        time_s: f64,
        seeker: SeekerState,
        target: TargetState,
    ) -> Self {
        Self {
            time_s,
            interceptor: seeker.position,
            target: target.position,
            interceptor_vel: seeker.velocity,
            miss_distance: miss_distance(seeker.position, target.position),
        }
    }
}

/// Euclidean distance between two points in the sim plane.
pub fn miss_distance(a: Vec2, b: Vec2) -> f32 {
    a.sub(b).length()
}

/// Line-of-sight angle from `from` to `to` (radians).
///
/// Uses the same convention as [`crate::tracking::los::line_of_sight`]:
/// `atan2(dx, dy)` so `0` means the target lies along +y from the seeker.
///
/// # C# analogy
/// `Math.Atan2(target.X - seeker.X, target.Y - seeker.Y)`.
pub fn line_of_sight(from: Vec2, to: Vec2) -> f32 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    dx.atan2(dy)
}

/// Maps image-plane track `(position, velocity)` to sim coordinates.
///
/// Image: origin top-left, +x right, +y down (pixels).  
/// Sim: origin at image center, +x right, +y up (see ARCHITECTURE §5).
///
/// # Returns
/// `(sim_position, sim_velocity)`.
pub fn map_image_to_sim(
    position: (f32, f32),
    velocity: (f32, f32),
    image_width: u32,
    image_height: u32,
) -> (Vec2, Vec2) {
    let cx = image_width as f32 / 2.0;
    let cy = image_height as f32 / 2.0;
    let sim_pos = Vec2::new(position.0 - cx, cy - position.1);
    let sim_vel = Vec2::new(velocity.0, -velocity.1);
    (sim_pos, sim_vel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miss_distance_at_same_point_is_zero() {
        let p = Vec2::new(10.0, 20.0);
        assert!(miss_distance(p, p).abs() < 1e-6);
    }

    #[test]
    fn miss_distance_is_euclidean() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(3.0, 4.0);
        assert!((miss_distance(a, b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn los_target_straight_ahead_along_plus_y_is_zero() {
        let seeker = Vec2::new(0.0, 0.0);
        let target = Vec2::new(0.0, 100.0);
        assert!(line_of_sight(seeker, target).abs() < 1e-5);
    }

    #[test]
    fn los_target_to_the_right_is_positive() {
        let los = line_of_sight(Vec2::new(0.0, 0.0), Vec2::new(50.0, 0.0));
        assert!(los > 0.0);
    }

    #[test]
    fn map_image_center_to_sim_origin() {
        let (pos, vel) = map_image_to_sim((320.0, 240.0), (90.0, -30.0), 640, 480);
        assert!(pos.x.abs() < 1e-5 && pos.y.abs() < 1e-5);
        assert!((vel.x - 90.0).abs() < 1e-5);
        assert!((vel.y - 30.0).abs() < 1e-5);
    }
}
