//! Target state estimation (Phase 4).
//!
//! Kalman filtering, association, and line-of-sight — wired into the pipeline
//! in later sub-steps.

pub mod associator;
pub mod kalman;
pub mod los;
pub mod roi_tracker;
pub mod tracker;

pub use associator::{associate_best_iou, associate_nearest_point, iou, point_distance, PointAssociation};
pub use kalman::KalmanFilter2d;
pub use los::{line_of_sight, LosEstimator};
pub use roi_tracker::{motion_centroid_roi, RoiMotionTracker};
pub use tracker::PointTracker;
