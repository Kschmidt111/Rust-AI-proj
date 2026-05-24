//! Phase 3 — orchestrates ingest → vision over a frame sequence.
//!
//! Owns the per-frame loop: load ONNX **once**, then decode + detect each frame.
//! CLI and future HTTP routes call into this module (ADR-010).

use crate::config::AppConfig;
use crate::domain::VisionError;
use crate::ingest::{FrameSource, IngestError};
use crate::vision::{decode, YoloDetector};
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;

/// Errors while running the folder pipeline.
#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Ingest(#[from] IngestError),

    #[error(transparent)]
    Vision(#[from] VisionError),
}

/// Per-frame stats logged during a folder run.
#[derive(Debug, Clone)]
pub struct FrameStats {
    /// Zero-based index in sorted frame list.
    pub index: usize,
    /// Path to the source image file.
    pub path: PathBuf,
    /// Number of detections after threshold + NMS.
    pub detection_count: usize,
    /// Wall time for decode + detect on this frame (milliseconds).
    pub elapsed_ms: f64,
}

/// Summary after processing every frame in a folder.
#[derive(Debug, Clone)]
pub struct FolderRunSummary {
    /// Folder that was processed.
    pub folder: PathBuf,
    /// How many frames were processed.
    pub frame_count: usize,
    /// Sum of detections across all frames.
    pub total_detections: usize,
    /// Per-frame breakdown.
    pub frames: Vec<FrameStats>,
}

/// Processes every PNG/JPEG in a folder through YOLO detection.
///
/// # Pipeline (per frame)
/// 1. [`FrameSource::collect_paths`] — sorted list from disk  
/// 2. [`YoloDetector::load`] — once at start (expensive)  
/// 3. For each path: [`decode::load_rgb_image`] → [`YoloDetector::detect_rgb`]  
/// 4. [`tracing::info!`] with detection count and elapsed ms  
///
/// # Arguments
/// * `config` — app config (vision thresholds, model path)
/// * `folder` — directory containing extracted frames
///
/// # C# analogy
/// ```csharp
/// await foreach (var frame in frameSource) {
///     var detections = await detector.DetectAsync(frame);
///     logger.LogInformation("frame {I}: {Count} detections", i, detections.Count);
/// }
/// ```
pub fn process_frame_folder(
    config: &AppConfig,
    folder: &Path,
) -> Result<FolderRunSummary, PipelineError> {
    let source = FrameSource::folder(folder);
    let paths = source.collect_paths()?;

    // Load ONNX session once — reloading per frame would waste hundreds of ms each time.
    let mut detector = YoloDetector::load(&config.vision)?;

    let mut frames = Vec::with_capacity(paths.len());

    for (index, path) in paths.iter().enumerate() {
        let start = Instant::now();

        let rgb = decode::load_rgb_image(path)?;
        let detections = detector.detect_rgb(&rgb)?;

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let detection_count = detections.len();

        tracing::info!(
            frame_index = index,
            path = %path.display(),
            detections = detection_count,
            elapsed_ms = format!("{elapsed_ms:.1}"),
            "frame processed"
        );

        frames.push(FrameStats {
            index,
            path: path.clone(),
            detection_count,
            elapsed_ms,
        });
    }

    let total_detections: usize = frames.iter().map(|f| f.detection_count).sum();

    tracing::info!(
        folder = %folder.display(),
        frame_count = frames.len(),
        total_detections,
        "folder run complete"
    );

    Ok(FolderRunSummary {
        folder: folder.to_path_buf(),
        frame_count: frames.len(),
        total_detections,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("seeker_{prefix}_{nanos}"))
    }

    #[test]
    fn missing_folder_returns_ingest_error() {
        let dir = temp_dir("missing");
        let err = process_frame_folder(
            &AppConfig::load().expect("config"),
            &dir,
        )
        .unwrap_err();
        assert!(matches!(err, PipelineError::Ingest(IngestError::FolderNotFound { .. })));
    }

    #[test]
    fn empty_folder_returns_ingest_error() {
        let dir = temp_dir("empty_pipe");
        fs::create_dir_all(&dir).unwrap();
        let err = process_frame_folder(
            &AppConfig::load().expect("config"),
            &dir,
        )
        .unwrap_err();
        assert!(matches!(err, PipelineError::Ingest(IngestError::EmptyFolder { .. })));
        let _ = fs::remove_dir_all(&dir);
    }
}
