//! Phase 3+ — orchestrates ingest → vision → tracking over a frame sequence.
//!
//! `process_frame_folder` runs YOLO (Phase 3). `track_motion_folder` wires motion
//! centroids + Kalman tracking (Phase 4F).

use crate::config::AppConfig;
use crate::domain::{TrackState, VisionError};
use crate::ingest::{FrameSource, IngestError};
use crate::telemetry::{new_run_id, write_tracks_csv, TelemetryError};
use crate::tracking::{
    associate_nearest_point, LosEstimator, PointAssociation, PointTracker, RoiMotionTracker,
};
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

    #[error(transparent)]
    Telemetry(#[from] TelemetryError),
}

/// Per-frame stats logged during a YOLO folder run.
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

/// Summary after processing every frame in a folder with YOLO.
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

/// Per-frame output from motion + tracker pipeline.
#[derive(Debug, Clone)]
pub struct TrackFrameStats {
    pub index: usize,
    pub path: PathBuf,
    /// Motion centroid from frame differencing, if any.
    pub centroid: Option<(f32, f32)>,
    /// Kalman track state after association, if a track is active.
    pub track: Option<TrackState>,
    pub elapsed_ms: f64,
}

/// Summary after motion tracking a frame folder.
#[derive(Debug, Clone)]
pub struct MotionTrackSummary {
    pub folder: PathBuf,
    pub frame_count: usize,
    /// Stable id when a track was established (usually `1`).
    pub track_id: Option<u64>,
    /// Run folder name under `[paths].output_dir`.
    pub run_id: String,
    /// Path to written `tracks.csv`.
    pub tracks_csv: PathBuf,
    /// Number of rows in `tracks.csv` (excluding header).
    pub track_row_count: usize,
    pub frames: Vec<TrackFrameStats>,
}

/// Processes every PNG/JPEG in a folder through YOLO detection.
///
/// # Pipeline (per frame)
/// 1. [`FrameSource::collect_paths`] — sorted list from disk  
/// 2. [`YoloDetector::load`] — once at start (expensive)  
/// 3. For each path: [`decode::load_rgb_image`] → [`YoloDetector::detect_rgb`]  
/// 4. [`tracing::info!`] with detection count and elapsed ms  
pub fn process_frame_folder(
    config: &AppConfig,
    folder: &Path,
) -> Result<FolderRunSummary, PipelineError> {
    let source = FrameSource::folder(folder);
    let paths = source.collect_paths()?;

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

/// Motion centroids + single-target Kalman tracking over a frame folder.
///
/// # Pipeline (per frame)
/// 1. Decode RGB  
/// 2. **Acquire:** full-frame motion centroid, or **track:** ROI motion around prediction  
/// 3. Associate centroid to Kalman state (distance gate)  
/// 4. Kalman update + [`LosEstimator`] → `TrackState` with `los` / `los_rate`  
/// 5. Drop and allow re-acquire when [`PointTracker::is_lost`]  
///
/// # C# analogy
/// ```csharp
/// foreach (var frame in frames) {
///     var centroid = motion.Detect(frame);
///     var measurement = Associate(tracker.Predicted, centroid, gate);
///     var state = tracker.Step(frameIndex, measurement);
/// }
/// ```
pub fn track_motion_folder(
    config: &AppConfig,
    folder: &Path,
) -> Result<MotionTrackSummary, PipelineError> {
    let paths = FrameSource::folder(folder).collect_paths()?;
    let dt = config.sim.dt_seconds;
    let gate = config.tracking.point_match_distance_px;
    let max_coast = config.tracking.max_coast_frames;
    let roi_half = config.tracking.roi_half_size_px;
    let motion_threshold = config.tracking.motion_threshold;

    let mut motion = RoiMotionTracker::with_settings(motion_threshold, 4);
    let mut los_estimator = LosEstimator::new();
    let mut tracker: Option<PointTracker> = None;
    let mut active_track_id: Option<u64> = None;
    let mut frames = Vec::with_capacity(paths.len());

    for (index, path) in paths.iter().enumerate() {
        let start = Instant::now();
        let rgb = decode::load_rgb_image(path)?;

        // Predict early when tracking so ROI is centered on the Kalman prediction.
        if let Some(trk) = tracker.as_mut() {
            trk.begin_frame(dt);
        }

        let centroid = if tracker.is_some() {
            let predicted = tracker.as_ref().expect("tracker").position();
            motion.detect_roi(&rgb, predicted.0, predicted.1, roi_half)
        } else {
            motion.detect_global(&rgb)
        };

        let track = match (&mut tracker, centroid) {
            (Some(trk), centroid_opt) => {
                let predicted = trk.position();
                let measurement = centroid_opt.and_then(|c| {
                    match associate_nearest_point(predicted, &[c], gate) {
                        PointAssociation::Matched(_) => Some(c),
                        PointAssociation::NoMatch => None,
                    }
                });
                let state = trk.finish_frame(index as u64, measurement);
                Some(apply_los(state, &mut los_estimator, rgb.width, rgb.height, dt))
            }
            (None, Some(c)) => {
                let mut trk = PointTracker::with_coast_limit(1, c.0, c.1, max_coast);
                los_estimator.reset();
                let state = trk.finish_frame(index as u64, Some(c));
                active_track_id = Some(trk.track_id());
                tracker = Some(trk);
                Some(apply_los(state, &mut los_estimator, rgb.width, rgb.height, dt))
            }
            (None, None) => None,
        };

        if tracker.as_ref().is_some_and(|t| t.is_lost()) {
            tracing::info!(frame_index = index, "track lost — ready to re-acquire");
            tracker = None;
            active_track_id = None;
            los_estimator.reset();
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        if let Some(ref state) = track {
            tracing::info!(
                frame_index = index,
                path = %path.display(),
                track_id = state.track_id,
                pos_x = format!("{:.1}", state.position.0),
                pos_y = format!("{:.1}", state.position.1),
                vel_x = format!("{:.1}", state.velocity.0),
                vel_y = format!("{:.1}", state.velocity.1),
                los = format!("{:.4}", state.los),
                los_rate = format!("{:.4}", state.los_rate),
                coast = state.coast_count,
                elapsed_ms = format!("{elapsed_ms:.1}"),
                "track frame"
            );
        } else {
            tracing::info!(
                frame_index = index,
                path = %path.display(),
                has_centroid = centroid.is_some(),
                elapsed_ms = format!("{elapsed_ms:.1}"),
                "track frame (no active track)"
            );
        }

        frames.push(TrackFrameStats {
            index,
            path: path.clone(),
            centroid,
            track,
            elapsed_ms,
        });
    }

    tracing::info!(
        folder = %folder.display(),
        frame_count = frames.len(),
        track_id = ?active_track_id,
        tracked_frames = frames.iter().filter(|f| f.track.is_some()).count(),
        "motion track run complete"
    );

    let run_id = new_run_id();
    let output_root = config.paths.resolve_output_dir();
    let track_states: Vec<TrackState> = frames
        .iter()
        .filter_map(|f| f.track.clone())
        .collect();
    let (tracks_csv, track_row_count) =
        write_tracks_csv(&output_root, &run_id, &track_states)?;

    tracing::info!(
        run_id = %run_id,
        path = %tracks_csv.display(),
        rows = track_row_count,
        "tracks.csv written"
    );

    Ok(MotionTrackSummary {
        folder: folder.to_path_buf(),
        frame_count: frames.len(),
        track_id: active_track_id,
        run_id,
        tracks_csv,
        track_row_count,
        frames,
    })
}

fn apply_los(
    mut state: TrackState,
    los_estimator: &mut LosEstimator,
    image_width: u32,
    image_height: u32,
    dt: f32,
) -> TrackState {
    let (los, los_rate) = los_estimator.update(state.position, image_width, image_height, dt);
    state.los = los;
    state.los_rate = los_rate;
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("seeker_{prefix}_{nanos}"))
    }

    const SKY: Rgb<u8> = Rgb([26, 58, 92]);

    fn write_dot_sequence(dir: &Path, count: usize) {
        fs::create_dir_all(dir).unwrap();
        let bg = SKY;
        for i in 0..count {
            let mut pixels = RgbImage::from_pixel(640, 480, bg);
            let cx = 40 + i as i32 * 5;
            let cy = 120 + i as i32 * 3;
            for y in 0..480_i32 {
                for x in 0..640_i32 {
                    let dx = x - cx;
                    let dy = y - cy;
                    if dx * dx + dy * dy <= 36 {
                        pixels.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                    }
                }
            }
            let path = dir.join(format!("{:04}.png", i + 1));
            pixels.save(path).unwrap();
        }
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

    #[test]
    fn track_motion_folder_produces_stable_track_on_synthetic_dots() {
        let dir = temp_dir("motion_track");
        write_dot_sequence(&dir, 20);
        let config = AppConfig::load().expect("config");

        let summary = track_motion_folder(&config, &dir).expect("track run");

        assert_eq!(summary.frame_count, 20);
        let tracked: Vec<_> = summary.frames.iter().filter_map(|f| f.track.as_ref()).collect();
        assert!(
            tracked.len() >= 15,
            "expected most frames tracked, got {}",
            tracked.len()
        );

        for state in &tracked {
            assert_eq!(state.track_id, 1);
        }

        assert!(summary.tracks_csv.exists(), "tracks.csv should exist");
        assert!(
            summary.track_row_count >= 15,
            "expected many csv rows, got {}",
            summary.track_row_count
        );

        let last = tracked.last().expect("last track state");
        assert!(last.velocity.0 > 50.0, "vx={}", last.velocity.0);
        assert!(last.velocity.1 > 20.0, "vy={}", last.velocity.1);
        assert!(last.los_rate.abs() > 0.01, "los_rate={}", last.los_rate);

        let _ = fs::remove_dir_all(&dir);
    }
}
