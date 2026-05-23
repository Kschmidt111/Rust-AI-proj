//! CLI entrypoints: `serve` (HTTP) and `detect` (Phase 2 vision).

use clap::{Parser, Subcommand};
use seeker_sim::{telemetry, AppConfig, RunError};
use std::path::PathBuf;

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
    /// Run YOLO detection on a single image (Phase 2).
    Detect {
        /// Path to `.jpg` or `.png` (e.g. `data/samples/test.jpg`).
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
        Commands::Detect { input } => run_detect(config, input),
    }
}

fn run_serve(config: AppConfig) -> Result<(), i32> {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Ok(address) = config.server.socket_addr() {
        tracing::info!(%address, "try: curl http://{address}/health");
    }

    rt.block_on(async {
        if let Err(err) = seeker_sim::run(config).await {
            log_run_error(&err);
            return Err(1);
        }
        Ok(())
    })
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
