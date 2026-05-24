//! 2D kinematic intercept simulation (Phase 5B).

pub mod engine;
pub mod state;

pub use engine::SimEngine;
pub use state::{line_of_sight, map_image_to_sim, miss_distance, SeekerState, SimState, TargetState, Vec2};
