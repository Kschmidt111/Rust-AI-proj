//! Simulation engine — integrate seeker and target over `dt` (Phase 5B).
//!
//! Seeker uses **constant speed**: lateral acceleration from PN rotates velocity
//! direction without changing magnitude (typical intercept teaching model).

use crate::sim::state::{
    line_of_sight, miss_distance, SeekerState, SimState, TargetState, Vec2,
};

/// Kinematic 2D intercept simulation (seeker + constant-velocity target).
///
/// # C# analogy
/// A small `SimulationEngine` class with `Step(dt, lateralAccel)` called each frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SimEngine {
    pub time_s: f64,
    pub seeker: SeekerState,
    pub target: TargetState,
}

impl SimEngine {
    /// Creates an engine from explicit initial conditions.
    ///
    /// # Arguments
    /// * `seeker` — interceptor position and velocity.
    /// * `target` — target position and constant velocity.
    pub fn new(seeker: SeekerState, target: TargetState) -> Self {
        Self {
            time_s: 0.0,
            seeker,
            target,
        }
    }

    /// Seeker starts `initial_miss_distance` from target, velocity aimed at target.
    ///
    /// Offset direction: opposite target velocity when moving; otherwise `-x`.
    /// Used by the intercept pipeline so initial heading matches the mapped track.
    pub fn chase_target(
        initial_miss_distance: f32,
        closing_speed: f32,
        target_position: Vec2,
        target_velocity: Vec2,
    ) -> Self {
        let behind = if target_velocity.length() > 1e-3 {
            target_velocity.scale(-1.0 / target_velocity.length())
        } else {
            Vec2::new(-1.0, 0.0)
        };
        let seeker_pos = target_position.add(behind.scale(initial_miss_distance));
        let to_target = target_position.sub(seeker_pos);
        let range = to_target.length().max(1e-6);
        let seeker = SeekerState {
            position: seeker_pos,
            velocity: to_target.scale(closing_speed / range),
        };
        let target = TargetState {
            position: target_position,
            velocity: target_velocity,
        };
        Self::new(seeker, target)
    }

    /// Head-on chase setup: seeker starts `initial_miss_distance` behind target on -x,
    /// with velocity `(closing_speed, 0)`; target at `target_position` with `target_velocity`.
    ///
    /// Useful for tests and Phase 5C pipeline bootstrap.
    pub fn head_on_chase(
        initial_miss_distance: f32,
        closing_speed: f32,
        target_position: Vec2,
        target_velocity: Vec2,
    ) -> Self {
        let seeker_pos = target_position.sub(Vec2::new(initial_miss_distance, 0.0));
        let seeker = SeekerState {
            position: seeker_pos,
            velocity: Vec2::new(closing_speed, 0.0),
        };
        let target = TargetState {
            position: target_position,
            velocity: target_velocity,
        };
        Self::new(seeker, target)
    }

    /// Current miss distance (sim units).
    pub fn miss_distance(&self) -> f32 {
        miss_distance(self.seeker.position, self.target.position)
    }

    /// LOS from seeker to target (radians), same convention as `tracking/los`.
    pub fn los_to_target(&self) -> f32 {
        line_of_sight(self.seeker.position, self.target.position)
    }

    /// Returns a telemetry snapshot of the current state.
    pub fn snapshot(&self) -> SimState {
        SimState::from_parts(self.time_s, self.seeker, self.target)
    }

    /// Advances one timestep: target moves at constant velocity; seeker turns with
    /// `commanded_lateral_accel` applied **perpendicular** to velocity (constant speed).
    ///
    /// # Arguments
    /// * `dt` — step size in seconds.
    /// * `commanded_lateral_accel` — PN lateral command (sim units, e.g. m/s²).
    pub fn step(&mut self, dt: f32, commanded_lateral_accel: f32) {
        if dt <= 0.0 {
            return;
        }

        self.target.position = self
            .target
            .position
            .add(self.target.velocity.scale(dt));

        self.advance_seeker_constant_speed(dt, commanded_lateral_accel);
        self.time_s += f64::from(dt);
    }

    /// Advances **seeker only** — target position must be set externally (Phase 5C video coupling).
    ///
    /// # Arguments
    /// * `dt` — step size in seconds.
    /// * `commanded_lateral_accel` — PN lateral command (sim units).
    pub fn step_seeker_only(&mut self, dt: f32, commanded_lateral_accel: f32) {
        if dt <= 0.0 {
            return;
        }
        self.advance_seeker_constant_speed(dt, commanded_lateral_accel);
        self.time_s += f64::from(dt);
    }

    /// Sets target position/velocity from a mapped image track (does not move seeker).
    pub fn sync_target(&mut self, position: Vec2, velocity: Vec2) {
        self.target.position = position;
        self.target.velocity = velocity;
    }

    /// Rotates seeker velocity using lateral accel, preserving speed; then translates.
    fn advance_seeker_constant_speed(&mut self, dt: f32, a_lat: f32) {
        let speed = self.seeker.velocity.length();
        if speed < 1e-6 {
            return;
        }

        let tangent = self.seeker.velocity.scale(1.0 / speed);
        // Right-hand normal (CW +90°): positive a_lat turns clockwise, matching
        // `line_of_sight` convention — decreasing LOS needs negative rate → negative
        // a_cmd → CCW turn toward the target.
        let normal = Vec2::new(tangent.y, -tangent.x);
        let v_after_accel = self
            .seeker
            .velocity
            .add(normal.scale(a_lat * dt));

        let new_len = v_after_accel.length();
        self.seeker.velocity = if new_len > 1e-6 {
            v_after_accel.scale(speed / new_len)
        } else {
            self.seeker.velocity
        };

        self.seeker.position = self
            .seeker
            .position
            .add(self.seeker.velocity.scale(dt));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guidance::proportional_navigation;

    #[test]
    fn target_moves_with_constant_velocity() {
        let mut sim = SimEngine::new(
            SeekerState {
                position: Vec2::ZERO,
                velocity: Vec2::new(100.0, 0.0),
            },
            TargetState {
                position: Vec2::new(500.0, 0.0),
                velocity: Vec2::new(0.0, 30.0),
            },
        );

        sim.step(1.0, 0.0);
        assert!((sim.target.position.y - 30.0).abs() < 1e-4);
        assert!((sim.target.position.x - 500.0).abs() < 1e-4);
    }

    #[test]
    fn zero_lateral_accel_keeps_seeker_heading_on_straight_flight() {
        let mut sim = SimEngine::head_on_chase(500.0, 100.0, Vec2::new(500.0, 0.0), Vec2::ZERO);
        let heading_before = sim.seeker.velocity;

        sim.step(0.1, 0.0);

        let dx = sim.seeker.velocity.x - heading_before.x;
        let dy = sim.seeker.velocity.y - heading_before.y;
        assert!(dx.abs() < 1e-5 && dy.abs() < 1e-5);
    }

    #[test]
    fn positive_lateral_accel_rotates_velocity_clockwise() {
        let mut sim = SimEngine::head_on_chase(500.0, 100.0, Vec2::new(500.0, 0.0), Vec2::ZERO);

        sim.step(0.5, 50.0);

        assert!(
            sim.seeker.velocity.y < 0.0,
            "expected CW turn (negative vy), got {:?}",
            sim.seeker.velocity
        );
        assert!((sim.seeker.velocity.length() - 100.0).abs() < 0.01);
    }

    #[test]
    fn snapshot_matches_engine_fields() {
        let sim = SimEngine::head_on_chase(300.0, 80.0, Vec2::new(300.0, 50.0), Vec2::new(10.0, 0.0));
        let snap = sim.snapshot();
        assert!((snap.time_s - 0.0).abs() < 1e-9);
        assert_eq!(snap.interceptor, sim.seeker.position);
        assert_eq!(snap.target, sim.target.position);
        assert!((snap.miss_distance - 300.0).abs() < 1e-3);
    }

    #[test]
    fn chase_target_velocity_points_at_target() {
        let target_pos = Vec2::new(400.0, 80.0);
        let target_vel = Vec2::new(50.0, 10.0);
        let sim = SimEngine::chase_target(300.0, 120.0, target_pos, target_vel);

        assert!((sim.miss_distance() - 300.0).abs() < 1.0);
        let to_target = target_pos.sub(sim.seeker.position);
        let cross = sim.seeker.velocity.x * to_target.y - sim.seeker.velocity.y * to_target.x;
        assert!(
            cross.abs() < 1.0,
            "velocity should align with line to target"
        );
    }

    #[test]
    fn step_seeker_only_leaves_target_fixed() {
        let mut sim = SimEngine::head_on_chase(200.0, 100.0, Vec2::new(200.0, 50.0), Vec2::ZERO);
        let target_before = sim.target.position;

        sim.step_seeker_only(0.1, 20.0);

        assert_eq!(sim.target.position, target_before);
        assert!(sim.time_s > 0.0);
    }

    #[test]
    fn pn_steering_beats_un_guided_flight_on_offset_head_on() {
        const N: f32 = 4.0;
        const V_C: f32 = 150.0;
        const DT: f32 = 0.033;
        const STEPS: u32 = 800;

        let initial_seeker = SeekerState {
            position: Vec2::new(0.0, 0.0),
            velocity: Vec2::new(V_C, 0.0),
        };
        let initial_target = TargetState {
            position: Vec2::new(500.0, 120.0),
            velocity: Vec2::new(20.0, 0.0),
        };

        let mut unguided = SimEngine::new(initial_seeker, initial_target);
        let mut guided = SimEngine::new(initial_seeker, initial_target);

        let mut prev_los: Option<f32> = None;
        let mut min_guided = f32::MAX;
        let mut min_unguided = f32::MAX;

        for _ in 0..STEPS {
            unguided.step(DT, 0.0);
            min_unguided = min_unguided.min(unguided.miss_distance());

            let los = guided.los_to_target();
            let los_rate = match prev_los {
                Some(prev) => (los - prev) / DT,
                None => 0.0,
            };
            prev_los = Some(los);

            let a_cmd = proportional_navigation(N, V_C, los_rate);
            guided.step(DT, a_cmd);
            min_guided = min_guided.min(guided.miss_distance());
        }

        assert!(
            min_guided < min_unguided,
            "PN should outperform straight flight: guided={min_guided}, unguided={min_unguided}"
        );
        assert!(
            min_guided < 30.0,
            "expected near intercept with PN, min_guided={min_guided}"
        );
    }
}
