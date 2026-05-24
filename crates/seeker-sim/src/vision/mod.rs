//! # Vision pipeline — single-image object detection (Phase 2)
//!
//! This module turns a JPEG/PNG on disk into a list of [`Detection`] values
//! (class name, confidence, bounding box in **original image pixels**).
//!
//! ## End-to-end flow
//!
//! ```text
//!  disk (.jpg)          PREPROCESS              INFERENCE           POSTPROCESS
//! ┌───────────┐    ┌──────────────────┐    ┌─────────────┐    ┌────────────────────┐
//! │ decode    │ -> │ letterbox        │ -> │ YOLO ONNX   │ -> │ parse 8400 preds │
//! │ RGB pixels│    │ normalize NCHW   │    │ run session │    │ threshold + NMS    │
//! └───────────┘    └──────────────────┘    └─────────────┘    └────────────────────┘
//!      │                    │                      │                      │
//!   e.g. 810×1080      e.g. 640×640            raw f32 tensor         Vec<Detection>
//!   uint8 RGB          float32 [0,1]           [1, 84, 8400]
//! ```
//!
//! ## Why four files?
//!
//! | File | Role |
//! |------|------|
//! | [`decode`] | File bytes → RGB pixel buffer (human image format) |
//! | [`preprocess`] | RGB → model input tensor (**pre-processing**) |
//! | [`detector`] | Load ONNX, run inference (the neural network) |
//! | [`postprocess`] | Raw tensor → boxes in original coords (**post-processing**) |
//!
//! The neural network only understands fixed-size float tensors. Everything before
//! `session.run()` is **pre-processing**; everything after is **post-processing**.
//!
//! # C# analogy
//! Like an ML.NET pipeline: `LoadImage` → `Featurize` → `PredictionEngine.Predict` → map outputs to DTOs.

pub mod decode;
mod detector;
pub mod motion;
mod postprocess;
mod preprocess;

use crate::config::AppConfig;
use crate::domain::{Detection, VisionError};
use std::path::Path;

pub use detector::YoloDetector;
pub use motion::{MotionDetector, motion_centroid};

/// Runs the full detect pipeline on one image file.
///
/// Orchestrates decode → preprocess → ONNX → postprocess. This is the public
/// entry point used by the CLI `detect` subcommand.
///
/// # Pipeline steps
/// 1. **Decode** — read JPEG/PNG into RGB pixels (`decode::load_rgb_image`)
/// 2. **Preprocess** — letterbox resize + normalize to `[1, 3, 640, 640]` tensor
/// 3. **Inference** — ONNX Runtime forward pass (`YoloDetector::detect_rgb`)
/// 4. **Postprocess** — decode 8400 candidate boxes, filter by confidence, NMS
///
/// # Returns
/// Detections in **original image pixel coordinates** (not letterboxed 640-space).
///
/// # C# analogy
/// `Task<List<Detection>> DetectAsync(string imagePath)` on a vision service.
pub fn detect_on_image(config: &AppConfig, image_path: &Path) -> Result<Vec<Detection>, VisionError> {
    let rgb = decode::load_rgb_image(image_path)?;
    let mut detector = YoloDetector::load(&config.vision)?;
    detector.detect_rgb(&rgb)
}

/// Same as [`detect_on_image`], but returns pretty-printed JSON for CLI output.
pub fn detect_on_image_json(config: &AppConfig, image_path: &Path) -> Result<String, VisionError> {
    let detections = detect_on_image(config, image_path)?;
    Ok(serde_json::to_string_pretty(&detections)?)
}
