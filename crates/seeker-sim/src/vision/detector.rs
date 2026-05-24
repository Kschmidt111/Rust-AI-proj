//! # Step 2 — Inference: ONNX Runtime forward pass
//!
//! This file is the **glue** between pre-processing and post-processing.
//! It does not implement math on pixels or boxes — it loads the `.onnx` file and
//! calls `session.run()`.
//!
//! ## What ONNX Runtime does
//!
//! ```text
//! Input:  float tensor [1, 3, 640, 640]  (from preprocess.rs)
//! Output: float tensor [1, 84, 8400]     (raw, goes to postprocess.rs)
//! ```
//!
//! The `.onnx` file contains the trained YOLOv8n weights and graph. We treat it as a
//! black box: same input shape and dtype every time → same output shape.
//!
//! ## Why `YoloDetector` is a struct
//!
//! Loading the model from disk is **expensive** (~100–500 ms). We load once in
//! `load()`, then call `detect_rgb()` many times (Phase 3: once per video frame).
//!
//! # C# analogy
//! ```csharp
//! // Load once
//! using var session = new InferenceSession("yolov8n.onnx");
//! // Run many times
//! var results = session.Run(inputs);
//! ```

use crate::config::VisionConfig;
use crate::domain::{Detection, VisionError};
use crate::vision::decode::RgbImageData;
use crate::vision::postprocess::decode_yolov8_output;
use crate::vision::preprocess::{letterbox_preprocess, PreprocessOutput};
use ort::session::Session;
use ort::value::Tensor;
use tracing::instrument;

/// Loaded YOLO model + config. Holds an ONNX Runtime [`Session`] (native handle).
///
/// # C# analogy
/// A service that owns a singleton `InferenceSession` injected at startup.
pub struct YoloDetector {
    /// ONNX Runtime session — loads weights, runs the graph on CPU/GPU.
    session: Session,
    /// Thresholds and paths copied from `config/default.toml` `[vision]` section.
    config: VisionConfig,
    /// Input tensor name inside the ONNX graph (usually `"images"` for Ultralytics exports).
    input_name: String,
}

impl YoloDetector {
    /// Loads ONNX weights from disk and builds an inference session.
    ///
    /// # Errors
    /// * [`VisionError::ModelNotFound`] — `models/yolov8n.onnx` missing (run download script)
    /// * ONNX errors — corrupt file or incompatible opset
    ///
    /// # C# analogy
    /// `new InferenceSession(modelPath)` in constructor / DI startup.
    pub fn load(config: &VisionConfig) -> Result<Self, VisionError> {
        let model_path = config.resolve_model_path();

        if !model_path.exists() {
            return Err(VisionError::ModelNotFound { path: model_path });
        }

        tracing::info!(path = %model_path.display(), "loading ONNX model");

        // commit_from_file parses the ONNX graph and prepares execution providers (CPU/CUDA).
        let session = Session::builder()?.commit_from_file(&model_path)?;

        // ONNX models name their inputs; Ultralytics YOLO typically uses "images".
        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_string())
            .unwrap_or_else(|| "images".to_string());

        tracing::debug!(%input_name, "ONNX input name");

        Ok(Self {
            session,
            config: config.clone(),
            input_name,
        })
    }

    /// Full detection on one decoded RGB image: preprocess → infer → postprocess.
    ///
    /// # Pipeline (this function orchestrates all three vision stages)
    /// 1. [`letterbox_preprocess`] — RGB → NCHW float tensor + [`LetterboxMeta`]
    /// 2. [`run_onnx`] — tensor → raw output `[1, 84, N]`
    /// 3. [`decode_yolov8_output`] — raw output → `Vec<Detection>` in original pixels
    ///
    /// # Returns
    /// Boxes in **original image coordinates**, not 640×640 letterbox space.
    #[instrument(name = "vision.detect", skip_all, fields(width, height))]
    pub fn detect_rgb(&mut self, image: &RgbImageData) -> Result<Vec<Detection>, VisionError> {
        tracing::Span::current().record("width", image.width);
        tracing::Span::current().record("height", image.height);

        // --- PRE-PROCESSING ---
        let preprocessed = letterbox_preprocess(image, self.config.input_size);
        tracing::debug!(
            input_size = preprocessed.meta.input_size,
            scale = preprocessed.meta.scale,
            "letterbox preprocess complete"
        );

        // --- INFERENCE (this file) ---
        let output = self.run_onnx(&preprocessed)?;

        // --- POST-PROCESSING ---
        decode_yolov8_output(
            &output.values,
            &output.shape,
            &preprocessed.meta,
            &self.config,
        )
    }

    /// Runs one forward pass through the YOLO ONNX graph.
    ///
    /// Wraps the flat `Vec<f32>` from preprocess into an ONNX [`Tensor`] with shape
    /// `[batch=1, channels=3, height, width]` (NCHW), then extracts the output tensor.
    fn run_onnx(&mut self, preprocessed: &PreprocessOutput) -> Result<OnnxOutput, VisionError> {
        let size = self.config.input_size as i64;

        // NCHW: batch dimension 1 means "one image in this inference call".
        let shape = [1_i64, 3, size, size];

        // ort expects (shape, contiguous float data). We clone the Vec here; Phase 3+
        // could reuse a buffer to avoid allocation per frame.
        let input_tensor = Tensor::from_array((shape, preprocessed.tensor.clone()))?;

        // Named input binding: { "images" => tensor } → run graph → output map.
        let outputs = self.session.run(ort::inputs![self.input_name.as_str() => input_tensor])?;

        let (name, value) = outputs
            .iter()
            .next()
            .ok_or_else(|| VisionError::OutputShape("model returned no outputs".into()))?;

        tracing::debug!(output_name = %name, "ONNX inference complete");

        // Copy ONNX output into owned Vec for postprocess (ort borrows internally).
        let (shape, data) = value.try_extract_tensor::<f32>()?;
        let shape_usize: Vec<usize> = shape.iter().map(|d| *d as usize).collect();
        let values: Vec<f32> = data.iter().copied().collect();

        Ok(OnnxOutput {
            shape: shape_usize,
            values,
        })
    }
}

/// Raw ONNX output before post-processing (still in model space, not JSON-ready).
struct OnnxOutput {
    /// e.g. `[1, 84, 8400]`
    shape: Vec<usize>,
    /// Flattened f32 values in ONNX layout order.
    values: Vec<f32>,
}
