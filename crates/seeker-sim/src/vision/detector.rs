//! ONNX Runtime session for YOLOv8.

use crate::config::VisionConfig;
use crate::domain::{Detection, VisionError};
use crate::vision::decode::RgbImageData;
use crate::vision::postprocess::decode_yolov8_output;
use crate::vision::preprocess::{letterbox_preprocess, PreprocessOutput};
use ort::session::Session;
use ort::value::Tensor;
use tracing::instrument;

/// Loaded YOLO model + settings. Create once, run many times.
///
/// # C# analogy
/// Like holding a singleton `InferenceSession` in ML.NET.
pub struct YoloDetector {
    session: Session,
    config: VisionConfig,
    input_name: String,
}

impl YoloDetector {
    /// Loads ONNX weights from path in config.
    ///
    /// # Errors
    /// Returns [`VisionError::ModelNotFound`] if the `.onnx` file is missing.
    pub fn load(config: &VisionConfig) -> Result<Self, VisionError> {
        let model_path = config.resolve_model_path();

        if !model_path.exists() {
            return Err(VisionError::ModelNotFound { path: model_path });
        }

        tracing::info!(path = %model_path.display(), "loading ONNX model");

        let session = Session::builder()?.commit_from_file(&model_path)?;

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

    /// Runs detection on an already-decoded RGB image.
    #[instrument(name = "vision.detect", skip_all, fields(width, height))]
    pub fn detect_rgb(&mut self, image: &RgbImageData) -> Result<Vec<Detection>, VisionError> {
        tracing::Span::current().record("width", image.width);
        tracing::Span::current().record("height", image.height);

        let preprocessed = letterbox_preprocess(image, self.config.input_size);
        tracing::debug!(
            input_size = preprocessed.meta.input_size,
            scale = preprocessed.meta.scale,
            "letterbox preprocess complete"
        );
        let output = self.run_onnx(&preprocessed)?;
        decode_yolov8_output(
            &output.values,
            &output.shape,
            &preprocessed.meta,
            &self.config,
        )
    }

    fn run_onnx(&mut self, preprocessed: &PreprocessOutput) -> Result<OnnxOutput, VisionError> {
        let size = self.config.input_size as i64;
        let shape = [1_i64, 3, size, size];

        let input_tensor = Tensor::from_array((shape, preprocessed.tensor.clone()))?;

        let outputs = self.session.run(ort::inputs![self.input_name.as_str() => input_tensor])?;

        let (name, value) = outputs
            .iter()
            .next()
            .ok_or_else(|| VisionError::OutputShape("model returned no outputs".into()))?;

        tracing::debug!(output_name = %name, "ONNX inference complete");

        let (shape, data) = value.try_extract_tensor::<f32>()?;
        let shape_usize: Vec<usize> = shape.iter().map(|d| *d as usize).collect();
        let values: Vec<f32> = data.iter().copied().collect();

        Ok(OnnxOutput {
            shape: shape_usize,
            values,
        })
    }
}

struct OnnxOutput {
    shape: Vec<usize>,
    values: Vec<f32>,
}
