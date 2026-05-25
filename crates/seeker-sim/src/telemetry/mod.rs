//! Logging bootstrap and run telemetry export (CSV).

pub mod latency;
pub mod plot;
pub mod record;
pub mod writer;

pub use latency::{summarize_frame_latency, FrameLatencySummary};

pub use plot::write_trajectory_png;
pub use record::{GuidanceRecord, SimRecord, TrackRecord};
pub use writer::{new_run_id, write_intercept_csvs, write_tracks_csv, InterceptCsvPaths, TelemetryError, TracksCsvWriter};

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initializes the global `tracing` subscriber (call once at process startup).
///
/// Log level defaults to `info`. Override with the `RUST_LOG` environment variable,
/// e.g. `RUST_LOG=debug` or `RUST_LOG=seeker_sim=debug,tower_http=info`.
///
/// # C# analogy
/// `builder.Logging.AddConsole()` + log level from `appsettings.Development.json`.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(filter)
        .init();
}
