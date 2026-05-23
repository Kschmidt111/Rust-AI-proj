//! SeekerSim process entry point (Phase 1D).
//!
//! Responsibilities: init logging, load config, delegate to [`seeker_sim::run`].

use seeker_sim::{telemetry, AppConfig, RunError};

#[tokio::main]
async fn main() {
    telemetry::init();

    let config = match AppConfig::load() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(error = %err, "failed to load configuration");
            std::process::exit(1);
        }
    };

    if let Ok(address) = config.server.socket_addr() {
        tracing::info!(%address, "try: curl http://{address}/health");
    }

    if let Err(err) = seeker_sim::run(config).await {
        log_run_error(&err);
        std::process::exit(1);
    }
}

/// Maps server errors to actionable log lines.
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
