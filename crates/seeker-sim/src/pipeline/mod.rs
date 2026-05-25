//! Phase 3+ — orchestrates ingest → vision → tracking → guidance → sim.
//!
//! `process_frame_folder` runs YOLO (Phase 3). `track_motion_folder` wires motion
//! centroids + Kalman tracking (Phase 4F). `intercept_motion_folder` adds PN + sim (Phase 5C).
//! `run_pure_sim` runs kinematic sim only (Phase 6B).

mod sim_run;
mod run_api;

pub use run_api::{
    load_run_status, repo_root, resolve_artifact_path, resolve_input_path, run_folder_pipeline,
    CreateRunRequest, FolderRunError, RunArtifacts, RunStatusResponse,
};
pub use sim_run::{run_pure_sim, SimRunError, SimRunFrame, SimRunRequest, SimRunResponse};

use crate::config::AppConfig;
use crate::domain::{TrackState, VisionError};
use crate::guidance::lateral_accel;
use crate::ingest::{FrameSource, IngestError};
use crate::sim::{map_image_to_sim, SimEngine};
use crate::telemetry::{
    new_run_id, write_intercept_csvs, write_tracks_csv, GuidanceRecord, InterceptCsvPaths,
    SimRecord, TelemetryError,
};
use crate::tracking::{
    associate_nearest_point, LosEstimator, PointAssociation, PointTracker, RoiMotionTracker,
};
use crate::vision::{decode, YoloDetector};
use crate::vision::decode::RgbImageData;
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

/// Per-frame output when running track + guidance + sim.
#[derive(Debug, Clone)]
pub struct InterceptFrameStats {
    pub index: usize,
    pub path: PathBuf,
    pub centroid: Option<(f32, f32)>,
    pub track: Option<TrackState>,
    /// PN lateral command when sim was active.
    pub commanded_lateral_accel: Option<f32>,
    /// Sim miss distance after this frame's step.
    pub miss_distance: Option<f32>,
    pub elapsed_ms: f64,
}

/// Summary after motion track + PN guidance + 2D sim over a frame folder.
#[derive(Debug, Clone)]
pub struct InterceptSummary {
    pub folder: PathBuf,
    pub frame_count: usize,
    pub track_id: Option<u64>,
    pub run_id: String,
    pub tracks_csv: PathBuf,
    pub guidance_csv: PathBuf,
    pub sim_csv: PathBuf,
    pub trajectory_png: PathBuf,
    pub track_row_count: usize,
    pub guidance_row_count: usize,
    pub sim_row_count: usize,
    /// Smallest miss distance observed during the run.
    pub min_miss_distance: Option<f32>,
    pub frames: Vec<InterceptFrameStats>,
}

/// Mutable state for motion + Kalman tracking over a frame sequence.
struct MotionTrackSession {
    motion: RoiMotionTracker,
    los_estimator: LosEstimator,
    tracker: Option<PointTracker>,
    active_track_id: Option<u64>,
}

impl MotionTrackSession {
    fn new(motion_threshold: f32) -> Self {
        Self {
            motion: RoiMotionTracker::with_settings(motion_threshold, 4),
            los_estimator: LosEstimator::new(),
            tracker: None,
            active_track_id: None,
        }
    }

    fn track_id(&self) -> Option<u64> {
        self.active_track_id
    }

    /// Processes one RGB frame; returns centroid and optional track state.
    fn process_frame(
        &mut self,
        index: usize,
        rgb: &RgbImageData,
        dt: f32,
        gate: f32,
        max_coast: u32,
        roi_half: u32,
    ) -> (Option<(f32, f32)>, Option<TrackState>) {
        if let Some(trk) = self.tracker.as_mut() {
            trk.begin_frame(dt);
        }

        let centroid = if self.tracker.is_some() {
            let predicted = self.tracker.as_ref().expect("tracker").position();
            self.motion
                .detect_roi(rgb, predicted.0, predicted.1, roi_half)
        } else {
            self.motion.detect_global(rgb)
        };

        let track = match (&mut self.tracker, centroid) {
            (Some(trk), centroid_opt) => {
                let predicted = trk.position();
                let measurement = centroid_opt.and_then(|c| {
                    match associate_nearest_point(predicted, &[c], gate) {
                        PointAssociation::Matched(_) => Some(c),
                        PointAssociation::NoMatch => None,
                    }
                });
                let state = trk.finish_frame(index as u64, measurement);
                Some(apply_los(
                    state,
                    &mut self.los_estimator,
                    rgb.width,
                    rgb.height,
                    dt,
                ))
            }
            (None, Some(c)) => {
                let mut trk = PointTracker::with_coast_limit(1, c.0, c.1, max_coast);
                self.los_estimator.reset();
                let state = trk.finish_frame(index as u64, Some(c));
                self.active_track_id = Some(trk.track_id());
                self.tracker = Some(trk);
                Some(apply_los(
                    state,
                    &mut self.los_estimator,
                    rgb.width,
                    rgb.height,
                    dt,
                ))
            }
            (None, None) => None,
        };

        if self.tracker.as_ref().is_some_and(|t| t.is_lost()) {
            tracing::info!(frame_index = index, "track lost — ready to re-acquire");
            self.tracker = None;
            self.active_track_id = None;
            self.los_estimator.reset();
        }

        (centroid, track)
    }
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

    let mut session = MotionTrackSession::new(config.tracking.motion_threshold);
    let mut frames = Vec::with_capacity(paths.len());

    for (index, path) in paths.iter().enumerate() {
        let start = Instant::now();
        let rgb = decode::load_rgb_image(path)?;
        let (centroid, track) = session.process_frame(
            index,
            &rgb,
            dt,
            gate,
            max_coast,
            roi_half,
        );

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        log_track_frame(index, path, centroid, track.as_ref(), elapsed_ms);

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
        track_id = ?session.track_id(),
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
        track_id: session.track_id(),
        run_id,
        tracks_csv,
        track_row_count,
        frames,
    })
}

/// Motion track + proportional navigation + 2D sim over a frame folder.
///
/// # Pipeline (per frame, when track is active)
/// 1. Same motion + Kalman path as [`track_motion_folder`]  
/// 2. Map track position to sim plane; sync target on [`SimEngine`]  
/// 3. `a_cmd = N * V_c * los_rate` from vision track  
/// 4. [`SimEngine::step_seeker_only`] — seeker integrates, target follows video track  
/// 5. Append rows to `tracks.csv`, `guidance.csv`, `sim.csv`
pub fn intercept_motion_folder(
    config: &AppConfig,
    folder: &Path,
) -> Result<InterceptSummary, PipelineError> {
    let paths = FrameSource::folder(folder).collect_paths()?;
    let dt = config.sim.dt_seconds;
    let gate = config.tracking.point_match_distance_px;
    let max_coast = config.tracking.max_coast_frames;
    let roi_half = config.tracking.roi_half_size_px;
    let law = config.guidance.law.clone();

    let mut session = MotionTrackSession::new(config.tracking.motion_threshold);
    let mut sim: Option<SimEngine> = None;
    let mut frames = Vec::with_capacity(paths.len());
    let mut guidance_records = Vec::new();
    let mut sim_records = Vec::new();
    let mut min_miss: Option<f32> = None;

    for (index, path) in paths.iter().enumerate() {
        let start = Instant::now();
        let rgb = decode::load_rgb_image(path)?;
        let (centroid, track) = session.process_frame(
            index,
            &rgb,
            dt,
            gate,
            max_coast,
            roi_half,
        );

        let mut commanded_lateral_accel = None;
        let mut frame_miss = None;

        if let Some(ref state) = track {
            if sim.is_none() {
                sim = Some(init_sim_from_track(config, state, rgb.width, rgb.height));
            }

            if let Some(engine) = sim.as_mut() {
                let (target_pos, target_vel) = map_image_to_sim(
                    state.position,
                    state.velocity,
                    rgb.width,
                    rgb.height,
                );
                engine.sync_target(target_pos, target_vel);

                let a_cmd = guidance_accel(config, state);
                engine.step_seeker_only(dt, a_cmd);

                commanded_lateral_accel = Some(a_cmd);
                let miss = engine.miss_distance();
                frame_miss = Some(miss);
                min_miss = Some(min_miss.map_or(miss, |m| m.min(miss)));

                guidance_records.push(GuidanceRecord {
                    frame_index: state.frame_index,
                    track_id: state.track_id,
                    los: state.los,
                    los_rate: state.los_rate,
                    law: law.clone(),
                    commanded_lateral_accel: a_cmd,
                });
                sim_records.push(SimRecord::from_snapshot(
                    state.frame_index,
                    &engine.snapshot(),
                ));
            }
        } else {
            sim = None;
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        log_intercept_frame(
            index,
            path,
            centroid,
            track.as_ref(),
            commanded_lateral_accel,
            frame_miss,
            elapsed_ms,
        );

        frames.push(InterceptFrameStats {
            index,
            path: path.clone(),
            centroid,
            track,
            commanded_lateral_accel,
            miss_distance: frame_miss,
            elapsed_ms,
        });
    }

    tracing::info!(
        folder = %folder.display(),
        frame_count = frames.len(),
        track_id = ?session.track_id(),
        min_miss_distance = ?min_miss,
        "intercept run complete"
    );

    let run_id = new_run_id();
    let output_root = config.paths.resolve_output_dir();
    let track_states: Vec<TrackState> = frames
        .iter()
        .filter_map(|f| f.track.clone())
        .collect();
    let InterceptCsvPaths {
        tracks_csv,
        guidance_csv,
        sim_csv,
        trajectory_png,
        track_rows,
        guidance_rows,
        sim_rows,
    } = write_intercept_csvs(
        &output_root,
        &run_id,
        &track_states,
        &guidance_records,
        &sim_records,
    )?;

    tracing::info!(
        run_id = %run_id,
        tracks = %tracks_csv.display(),
        guidance = %guidance_csv.display(),
        sim = %sim_csv.display(),
        plot = %trajectory_png.display(),
        track_rows,
        guidance_rows,
        sim_rows,
        "intercept CSVs written"
    );

    Ok(InterceptSummary {
        folder: folder.to_path_buf(),
        frame_count: frames.len(),
        track_id: session.track_id(),
        run_id,
        tracks_csv,
        guidance_csv,
        sim_csv,
        trajectory_png,
        track_row_count: track_rows,
        guidance_row_count: guidance_rows,
        sim_row_count: sim_rows,
        min_miss_distance: min_miss,
        frames,
    })
}

fn init_sim_from_track(
    config: &AppConfig,
    track: &TrackState,
    image_width: u32,
    image_height: u32,
) -> SimEngine {
    let (target_pos, target_vel) =
        map_image_to_sim(track.position, track.velocity, image_width, image_height);
    SimEngine::chase_target(
        config.sim.initial_miss_distance,
        config.guidance.closing_velocity,
        target_pos,
        target_vel,
    )
}

fn guidance_accel(config: &AppConfig, track: &TrackState) -> f32 {
    let n = config.guidance.navigation_constant;
    let v_c = config.guidance.closing_velocity;

    match lateral_accel(
        &config.guidance.law,
        n,
        v_c,
        track.los,
        track.los_rate,
    ) {
        Ok(a) => a,
        Err(err) => {
            tracing::warn!(law = %err.law, "unknown guidance law — zero lateral accel");
            0.0
        }
    }
}

fn log_track_frame(
    index: usize,
    path: &Path,
    centroid: Option<(f32, f32)>,
    track: Option<&TrackState>,
    elapsed_ms: f64,
) {
    if let Some(state) = track {
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
}

fn log_intercept_frame(
    index: usize,
    path: &Path,
    centroid: Option<(f32, f32)>,
    track: Option<&TrackState>,
    a_cmd: Option<f32>,
    miss: Option<f32>,
    elapsed_ms: f64,
) {
    if let Some(state) = track {
        tracing::info!(
            frame_index = index,
            path = %path.display(),
            track_id = state.track_id,
            los_rate = format!("{:.4}", state.los_rate),
            a_cmd = format!("{:.2}", a_cmd.unwrap_or(0.0)),
            miss = format!("{:.1}", miss.unwrap_or(f32::NAN)),
            elapsed_ms = format!("{elapsed_ms:.1}"),
            "intercept frame"
        );
    } else {
        tracing::info!(
            frame_index = index,
            path = %path.display(),
            has_centroid = centroid.is_some(),
            elapsed_ms = format!("{elapsed_ms:.1}"),
            "intercept frame (no active track)"
        );
    }
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

    #[test]
    fn intercept_motion_folder_writes_guidance_and_sim_csv() {
        let dir = temp_dir("intercept");
        write_dot_sequence(&dir, 20);
        let config = AppConfig::load().expect("config");

        let summary = intercept_motion_folder(&config, &dir).expect("intercept run");

        assert_eq!(summary.frame_count, 20);
        assert!(summary.tracks_csv.exists());
        assert!(summary.guidance_csv.exists());
        assert!(summary.sim_csv.exists());
        assert!(summary.trajectory_png.exists());
        assert!(fs::metadata(&summary.trajectory_png).unwrap().len() > 500);
        assert_eq!(summary.track_row_count, summary.guidance_row_count);
        assert_eq!(summary.track_row_count, summary.sim_row_count);
        assert!(
            summary.track_row_count >= 15,
            "expected many tracked frames, got {}",
            summary.track_row_count
        );

        let guidance_text = fs::read_to_string(&summary.guidance_csv).expect("guidance csv");
        assert!(guidance_text.contains("commanded_lateral_accel"));
        let law_tag = format!(",{},", config.guidance.law);
        assert!(
            guidance_text.contains(&law_tag),
            "expected law {} in guidance.csv",
            config.guidance.law
        );

        let sim_text = fs::read_to_string(&summary.sim_csv).expect("sim csv");
        assert!(sim_text.contains("miss_distance"));

        assert!(
            summary.frames.iter().any(|f| {
                f.commanded_lateral_accel
                    .is_some_and(|a| a.abs() > 0.0)
            }),
            "expected non-zero PN commands during track"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
