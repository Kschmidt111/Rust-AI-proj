//! Guidance laws — steer the simulated interceptor from track geometry (Phase 5).
//!
//! Laws are **pure functions** (no hidden state). Pipeline / sim call them each step
//! with LOS rate and config gains.

pub mod pn;
pub mod pure_pursuit;

pub use pn::proportional_navigation;
pub use pure_pursuit::pure_pursuit;
