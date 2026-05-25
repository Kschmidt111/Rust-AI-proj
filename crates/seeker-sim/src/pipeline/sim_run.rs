//! Pure kinematic sim run — no vision ingest (Phase 6B).
//!
//! Used by `POST /v1/sim/run` and (later) the sim viewer canvas.

use crate::config::AppConfig;
use crate::guidance::{lateral_accel, UnknownGuidanceLaw};
use crate::sim::{SimEngine, Vec2};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default cap on integration steps (~20 s at 30 fps).
const DEFAULT_MAX_STEPS: u32 = 600;

/// Stop early when miss distance drops below this (simulated intercept).
const INTERCEPT_THRESHOLD: f32 = 10.0;

/// Request body for a pure sim run (sim plane coordinates: x right, y up).
///
/// # C# analogy
/// A POST DTO / command object deserialized from JSON.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SimRunRequest {
    /// Target position x (sim units).
    pub target_x: f32,
    /// Target position y (sim units).
    pub target_y: f32,
    /// Target velocity x (sim units / s).
    pub target_vx: f32,
    /// Target velocity y (sim units / s).
    pub target_vy: f32,
    /// Guidance law: `"pn"` or `"pp"`.
    pub law: String,
    /// Overrides `[guidance].navigation_constant` when set.
    #[serde(default)]
    pub navigation_constant: Option<f32>,
    /// Overrides `[guidance].closing_velocity` when set.
    #[serde(default)]
    pub closing_velocity: Option<f32>,
    /// Overrides `[sim].dt_seconds` when set.
    #[serde(default)]
    pub dt_seconds: Option<f32>,
    /// Overrides `[sim].initial_miss_distance` when set.
    #[serde(default)]
    pub initial_miss_distance: Option<f32>,
    /// Max integration steps (default 600).
    #[serde(default)]
    pub max_steps: Option<u32>,
}

/// One frame snapshot returned to the canvas / client.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SimRunFrame {
    /// 0-based frame index.
    pub index: u32,
    /// Simulated time in seconds.
    pub time_s: f64,
    /// Interceptor position.
    pub interceptor: Vec2,
    /// Target position.
    pub target: Vec2,
    /// Euclidean miss distance (sim units).
    pub miss_distance: f32,
    /// Line-of-sight angle (radians).
    pub los: f32,
    /// Commanded lateral acceleration applied on this step.
    pub commanded_lateral_accel: f32,
}

/// Response from a completed pure sim run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SimRunResponse {
    /// Guidance law used (`pn` or `pp`).
    pub law: String,
    /// Number of frames in `frames`.
    pub frame_count: u32,
    /// Minimum miss distance observed during the run.
    pub min_miss_distance: f32,
    /// Time step per frame (seconds).
    pub dt_seconds: f32,
    /// Frame snapshots (includes initial state at index 0).
    pub frames: Vec<SimRunFrame>,
}

/// Errors while validating or running a pure sim.
#[derive(Debug, Error, PartialEq)]
pub enum SimRunError {
    #[error(transparent)]
    GuidanceLaw(#[from] UnknownGuidanceLaw),

    #[error("dt_seconds must be positive, got {0}")]
    InvalidDt(f32),

    #[error("max_steps must be at least 1, got {0}")]
    InvalidMaxSteps(u32),
}

/// Runs guidance + 2D kinematic sim from explicit initial conditions (no vision).
///
/// # Arguments
/// * `config` — Defaults for gains, timestep, and initial miss distance.
/// * `request` — Target state, law, and optional overrides.
///
/// # Returns
/// Frame snapshots suitable for canvas animation.
pub fn run_pure_sim(config: &AppConfig, request: &SimRunRequest) -> Result<SimRunResponse, SimRunError> {
    let dt = request.dt_seconds.unwrap_or(config.sim.dt_seconds);
    if dt <= 0.0 {
        return Err(SimRunError::InvalidDt(dt));
    }

    let max_steps = request.max_steps.unwrap_or(DEFAULT_MAX_STEPS);
    if max_steps == 0 {
        return Err(SimRunError::InvalidMaxSteps(max_steps));
    }

    let n = request
        .navigation_constant
        .unwrap_or(config.guidance.navigation_constant);
    let v_c = request
        .closing_velocity
        .unwrap_or(config.guidance.closing_velocity);
    let initial_miss = request
        .initial_miss_distance
        .unwrap_or(config.sim.initial_miss_distance);

    let target_pos = Vec2::new(request.target_x, request.target_y);
    let target_vel = Vec2::new(request.target_vx, request.target_vy);

    let mut engine = SimEngine::chase_target(initial_miss, v_c, target_pos, target_vel);

    let mut frames = Vec::with_capacity(max_steps as usize + 1);
    let mut min_miss = f32::MAX;
    let mut prev_los: Option<f32> = None;

    let push_frame = |index: u32, engine: &SimEngine, a_cmd: f32, out: &mut Vec<SimRunFrame>| {
        let los = engine.los_to_target();
        let miss = engine.miss_distance();
        out.push(SimRunFrame {
            index,
            time_s: engine.time_s,
            interceptor: engine.seeker.position,
            target: engine.target.position,
            miss_distance: miss,
            los,
            commanded_lateral_accel: a_cmd,
        });
        miss
    };

    let miss0 = push_frame(0, &engine, 0.0, &mut frames);
    min_miss = min_miss.min(miss0);

    for step in 1..=max_steps {
        let los = engine.los_to_target();
        let los_rate = match prev_los {
            Some(prev) => (los - prev) / dt,
            None => 0.0,
        };
        prev_los = Some(los);

        let a_cmd = lateral_accel(&request.law, n, v_c, los, los_rate)?;
        engine.step(dt, a_cmd);

        let miss = push_frame(step, &engine, a_cmd, &mut frames);
        min_miss = min_miss.min(miss);

        if miss < INTERCEPT_THRESHOLD {
            break;
        }
    }

    Ok(SimRunResponse {
        law: request.law.clone(),
        frame_count: frames.len() as u32,
        min_miss_distance: min_miss,
        dt_seconds: dt,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn test_config() -> AppConfig {
        AppConfig::load().expect("default config")
    }

    #[test]
    fn run_pure_sim_returns_initial_and_stepped_frames() {
        let config = test_config();
        let req = SimRunRequest {
            target_x: 400.0,
            target_y: 80.0,
            target_vx: 50.0,
            target_vy: 10.0,
            law: "pn".into(),
            navigation_constant: Some(4.0),
            closing_velocity: Some(150.0),
            dt_seconds: Some(0.033),
            initial_miss_distance: Some(300.0),
            max_steps: Some(100),
        };

        let resp = run_pure_sim(&config, &req).expect("sim run");
        assert!(resp.frame_count >= 2);
        assert_eq!(resp.frames[0].index, 0);
        assert!(resp.frames[0].commanded_lateral_accel.abs() < 1e-6);
        assert!(resp.min_miss_distance > 0.0);
    }

    #[test]
    fn pn_achieves_lower_min_miss_on_offset_chase() {
        let config = test_config();
        let pn_req = SimRunRequest {
            target_x: 500.0,
            target_y: 120.0,
            target_vx: 20.0,
            target_vy: 0.0,
            law: "pn".into(),
            navigation_constant: Some(4.0),
            closing_velocity: Some(150.0),
            dt_seconds: Some(0.033),
            initial_miss_distance: Some(500.0),
            max_steps: Some(800),
        };

        let pn = run_pure_sim(&config, &pn_req).expect("pn run");
        assert!(
            pn.min_miss_distance < 100.0,
            "PN should close within 100 sim units, got {}",
            pn.min_miss_distance
        );
    }

    #[test]
    fn rejects_unknown_law() {
        let config = test_config();
        let req = SimRunRequest {
            target_x: 0.0,
            target_y: 100.0,
            target_vx: 0.0,
            target_vy: 0.0,
            law: "bad".into(),
            navigation_constant: None,
            closing_velocity: None,
            dt_seconds: None,
            initial_miss_distance: None,
            max_steps: Some(10),
        };

        assert!(matches!(
            run_pure_sim(&config, &req),
            Err(SimRunError::GuidanceLaw(_))
        ));
    }

    #[test]
    fn rejects_zero_dt() {
        let config = test_config();
        let req = SimRunRequest {
            target_x: 0.0,
            target_y: 100.0,
            target_vx: 0.0,
            target_vy: 0.0,
            law: "pn".into(),
            navigation_constant: None,
            closing_velocity: None,
            dt_seconds: Some(0.0),
            initial_miss_distance: None,
            max_steps: Some(10),
        };

        assert!(matches!(
            run_pure_sim(&config, &req),
            Err(SimRunError::InvalidDt(_))
        ));
    }
}
