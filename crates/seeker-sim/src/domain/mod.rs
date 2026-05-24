//! Shared domain types and errors (no HTTP, no ONNX imports).

pub mod coco;
pub mod error;
pub mod types;

pub use error::VisionError;
pub use types::{BBox, Detection, TrackState};
