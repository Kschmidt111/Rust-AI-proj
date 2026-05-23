//! Vision pipeline errors.

use thiserror::Error;

/// Errors from image load, preprocess, ONNX inference, or postprocess.
#[derive(Debug, Error)]
pub enum VisionError {
    #[error("failed to read image '{path}': {source}")]
    ReadImage {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("failed to decode image: {0}")]
    DecodeImage(String),

    #[error("ONNX model not found at '{path}' — run scripts/download-model.ps1")]
    ModelNotFound { path: std::path::PathBuf },

    #[error("ONNX Runtime error: {0}")]
    Ort(#[from] ort::Error),

    #[error("unexpected ONNX output shape: {0}")]
    OutputShape(String),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
