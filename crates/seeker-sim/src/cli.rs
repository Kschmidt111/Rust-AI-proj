//! CLI entrypoints: `serve` (HTTP), `detect`, `process`, `track`, `intercept`, `run` (Phase 6F).

use clap::{Parser, Subcommand};
use seeker_sim::{
    pipeline::{intercept_motion_folder, resolve_input_path, track_motion_folder},
    telemetry::{self, summarize_frame_latency},
    AppConfig, RunError,
};
use std::path::{Path, PathBuf};

/// SeekerSim — visual tracking and guidance simulation.
#[derive(Parser, Debug)]
#[command(name = "seeker-sim", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the HTTP API server (default).
    Serve,
    /// Portfolio batch demo: intercept (or track) + latency summary (Phase 6F).
    Run {
        /// Directory of PNG/JPEG frames (e.g. `data/frames/dot_run_001`).
        #[arg(long, short)]
        input: PathBuf,
        /// `intercept` (default) or `track`.
        #[arg(long, default_value = "intercept")]
        mode: String,
    },
    /// Run YOLO detection on a single image (Phase 2).
    Detect {
        /// Path to `.jpg` or `.png` (e.g. `data/samples/test.jpg`).
        #[arg(long, short)]
        input: PathBuf,
    },
    /// Process a folder of frames in order (Phase 3).
    Process {
        /// Directory of PNG/JPEG frames (e.g. `data/frames/run_001`).
        #[arg(long, short)]
        input: PathBuf,
    },
    /// Motion centroids via frame differencing (Phase 4D).
    Motion {
        /// Directory of PNG/JPEG frames (e.g. `data/frames/dot_run_001`).
        #[arg(long, short)]
        input: PathBuf,
    },
    /// Motion + Kalman track over a frame folder (Phase 4F).
    Track {
        /// Directory of PNG/JPEG frames (e.g. `data/frames/dot_run_001`).
        #[arg(long, short)]
        input: PathBuf,
    },
    /// Motion track + PN guidance + 2D sim (Phase 5C).
    Intercept {
        /// Directory of PNG/JPEG frames (e.g. `data/frames/dot_run_001`).
        #[arg(long, short)]
        input: PathBuf,
    },
}

/// Parses CLI args and runs the selected subcommand.
pub fn run() -> Result<(), i32> {
    let cli = Cli::parse();

    telemetry::init();

    let config = match AppConfig::load() {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(error = %err, "failed to load configuration");
            return Err(1);
        }
    };

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => run_serve(config),
        Commands::Run { input, mode } => run_demo(config, input, mode),
        Commands::Detect { input } => run_detect(config, input),
        Commands::Process { input } => run_process(config, input),
        Commands::Motion { input } => run_motion(input),
        Commands::Track { input } => run_track(config, input),
        Commands::Intercept { input } => run_intercept(config, input),
    }
}

fn run_serve(config: AppConfig) -> Result<(), i32> {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Ok(address) = config.server.socket_addr() {
        tracing::info!(
            %address,
            "UI: http://{address}/  ·  health: http://{address}/health  ·  replay + sim viewer on same host"
        );
    }

    rt.block_on(async {
        if let Err(err) = seeker_sim::run(config).await {
            log_run_error(&err);
            return Err(1);
        }
        Ok(())
    })
}

/// Batch demo: full pipeline on a frame folder + p50/p95 latency (browser replay is separate).
fn run_demo(config: AppConfig, input: PathBuf, mode: String) -> Result<(), i32> {
    let input_str = input.to_string_lossy().into_owned();
    let folder = match resolve_input_path(&input_str) {
        Ok(p) => p,
        Err(err) => {
            tracing::error!(error = %err, path = %input_str, "invalid input path");
            return Err(1);
        }
    };

    let mode_lc = mode.to_ascii_lowercase();
    match mode_lc.as_str() {
        "intercept" => run_demo_intercept(config, &folder),
        "track" => run_demo_track(config, &folder),
        _ => {
            tracing::error!(mode = %mode, "unknown mode — use 'intercept' or 'track'");
            Err(1)
        }
    }
}

fn run_demo_intercept(config: AppConfig, folder: &Path) -> Result<(), i32> {
    match intercept_motion_folder(&config, folder) {
        Ok(summary) => {
            let elapsed: Vec<f64> = summary.frames.iter().map(|f| f.elapsed_ms).collect();
            print_latency_block("Per-frame pipeline", &elapsed);

            println!(
                "\nRun complete · {} frames · run_id={}",
                summary.frame_count, summary.run_id
            );
            println!("  {}", summary.tracks_csv.display());
            println!("  {}", summary.guidance_csv.display());
            println!("  {}", summary.sim_csv.display());
            println!("  {}", summary.trajectory_png.display());
            if let Some(min_miss) = summary.min_miss_distance {
                println!("  min miss distance: {min_miss:.1} sim units");
            }

            if let Ok(addr) = config.server.socket_addr() {
                println!(
                    "\nBrowser replay: http://{addr}/ → Run replay → paste run_id `{}`",
                    summary.run_id
                );
            }
            Ok(())
        }
        Err(err) => {
            tracing::error!(error = %err, path = %folder.display(), "run failed");
            Err(1)
        }
    }
}

fn run_demo_track(config: AppConfig, folder: &Path) -> Result<(), i32> {
    match track_motion_folder(&config, folder) {
        Ok(summary) => {
            let elapsed: Vec<f64> = summary.frames.iter().map(|f| f.elapsed_ms).collect();
            print_latency_block("Per-frame pipeline", &elapsed);

            println!(
                "\nRun complete · {} frames · run_id={:?}",
                summary.frame_count, summary.track_id
            );
            println!(
                "  {} ({} rows)",
                summary.tracks_csv.display(),
                summary.track_row_count
            );
            Ok(())
        }
        Err(err) => {
            tracing::error!(error = %err, path = %folder.display(), "run failed");
            Err(1)
        }
    }
}

/// Prints p50 / p95 / max ms for a list of per-frame timings.
fn print_latency_block(label: &str, elapsed_ms: &[f64]) {
    match summarize_frame_latency(elapsed_ms) {
        Some(s) => {
            println!(
                "{label} latency (ms): p50={:.1}  p95={:.1}  max={:.1}  (n={})",
                s.p50_ms, s.p95_ms, s.max_ms, s.count
            );
        }
        None => println!("{label} latency: no frame timings"),
    }
}

fn run_detect(config: AppConfig, input: PathBuf) -> Result<(), i32> {
    match seeker_sim::vision::detect_on_image_json(&config, &input) {
        Ok(json) => {
            println!("{json}");
            Ok(())
        }
        Err(err) => {
            tracing::error!(error = %err, path = %input.display(), "detection failed");
            Err(1)
        }
    }
}

fn run_process(config: AppConfig, input: PathBuf) -> Result<(), i32> {
    match seeker_sim::pipeline::process_frame_folder(&config, &input) {
        Ok(summary) => {
            println!(
                "Processed {} frames from {} ({} total detections)",
                summary.frame_count,
                summary.folder.display(),
                summary.total_detections
            );
            for frame in &summary.frames {
                println!(
                    "  [{:04}] {} detections ({:.1} ms)",
                    frame.index,
                    frame.detection_count,
                    frame.elapsed_ms
                );
            }
            Ok(())
        }
        Err(err) => {
            tracing::error!(error = %err, path = %input.display(), "frame pipeline failed");
            Err(1)
        }
    }
}

fn run_motion(input: PathBuf) -> Result<(), i32> {
    use seeker_sim::ingest::{FrameSource, IngestError};
    use seeker_sim::vision::{decode, MotionDetector};

    let paths = match FrameSource::folder(&input).collect_paths() {
        Ok(p) => p,
        Err(IngestError::FolderNotFound { path }) => {
            tracing::error!(path = %path.display(), "folder not found");
            return Err(1);
        }
        Err(IngestError::EmptyFolder { path }) => {
            tracing::error!(path = %path.display(), "folder has no PNG/JPEG frames");
            return Err(1);
        }
        Err(other) => {
            tracing::error!(error = %other, "failed to read frame folder");
            return Err(1);
        }
    };

    let mut detector = MotionDetector::new();
    let mut hits = 0_usize;

    println!("Motion centroids for {} ({} frames)", input.display(), paths.len());

    for (index, path) in paths.iter().enumerate() {
        let rgb = match decode::load_rgb_image(path) {
            Ok(img) => img,
            Err(err) => {
                tracing::error!(error = %err, path = %path.display(), "decode failed");
                return Err(1);
            }
        };

        match detector.detect(&rgb) {
            Some((cx, cy)) => {
                hits += 1;
                println!("  [{index:04}] centroid ({cx:.1}, {cy:.1})");
            }
            None => {
                println!("  [{index:04}] —");
            }
        }
    }

    println!("Centroids on {hits}/{} frames (frame 0 always skipped)", paths.len());
    Ok(())
}

fn run_track(config: AppConfig, input: PathBuf) -> Result<(), i32> {
    match seeker_sim::pipeline::track_motion_folder(&config, &input) {
        Ok(summary) => {
            println!(
                "Tracked {} frames from {} (track_id={:?})",
                summary.frame_count,
                summary.folder.display(),
                summary.track_id
            );
            println!(
                "Wrote {} rows to {}",
                summary.track_row_count,
                summary.tracks_csv.display()
            );
            for frame in &summary.frames {
                match &frame.track {
                    Some(state) => println!(
                        "  [{:04}] pos ({:.1}, {:.1}) vel ({:.1}, {:.1}) px/s los={:.4} rad/s={:.4} coast={}",
                        frame.index,
                        state.position.0,
                        state.position.1,
                        state.velocity.0,
                        state.velocity.1,
                        state.los,
                        state.los_rate,
                        state.coast_count
                    ),
                    None => {
                        let cent = frame
                            .centroid
                            .map(|(x, y)| format!("centroid ({x:.1}, {y:.1})"))
                            .unwrap_or_else(|| "no centroid".into());
                        println!("  [{:04}] — ({cent})", frame.index);
                    }
                }
            }
            Ok(())
        }
        Err(err) => {
            tracing::error!(error = %err, path = %input.display(), "motion track failed");
            Err(1)
        }
    }
}

fn run_intercept(config: AppConfig, input: PathBuf) -> Result<(), i32> {
    match seeker_sim::pipeline::intercept_motion_folder(&config, &input) {
        Ok(summary) => {
            println!(
                "Intercept {} frames from {} (track_id={:?})",
                summary.frame_count,
                summary.folder.display(),
                summary.track_id
            );
            println!(
                "Wrote tracks={} guidance={} sim={} rows",
                summary.track_row_count,
                summary.guidance_row_count,
                summary.sim_row_count,
            );
            println!("  {}", summary.tracks_csv.display());
            println!("  {}", summary.guidance_csv.display());
            println!("  {}", summary.sim_csv.display());
            println!("  {}", summary.trajectory_png.display());
            if let Some(min_miss) = summary.min_miss_distance {
                println!("  min miss distance: {min_miss:.1} sim units");
            }
            Ok(())
        }
        Err(err) => {
            tracing::error!(error = %err, path = %input.display(), "intercept run failed");
            Err(1)
        }
    }
}

fn log_run_error(err: &RunError) {
    match err {
        RunError::Bind { address, source } if source.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                %address,
                "port already in use — stop the other seeker-sim instance or change [server].bind in config"
            );
        }
        other => tracing::error!(error = %other, "server failed"),
    }
}
