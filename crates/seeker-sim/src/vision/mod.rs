//! Phase 2 — single-image object detection via YOLO ONNX.

mod decode;
mod detector;
mod postprocess;
mod preprocess;

use crate::config::AppConfig;
use crate::domain::{Detection, VisionError};
use std::path::Path;

pub use detector::YoloDetector;

/// Runs the full detect pipeline on one image file.
///
/// # Pipeline
/// 1. Decode JPEG/PNG  
/// 2. Letterbox preprocess → tensor  
/// 3. ONNX inference  
/// 4. Postprocess (threshold + NMS) → [`Detection`] list  
///
/// # C# analogy
/// Like a service method `Task<List<Detection>> DetectAsync(string imagePath)`.
pub fn detect_on_image(config: &AppConfig, image_path: &Path) -> Result<Vec<Detection>, VisionError> {
    let rgb = decode::load_rgb_image(image_path)?;
    let mut detector = YoloDetector::load(&config.vision)?;
    detector.detect_rgb(&rgb)
}

/// Serializes detections as pretty JSON (CLI output).
pub fn detect_on_image_json(config: &AppConfig, image_path: &Path) -> Result<String, VisionError> {
    let detections = detect_on_image(config, image_path)?;
    Ok(serde_json::to_string_pretty(&detections)?)
}
