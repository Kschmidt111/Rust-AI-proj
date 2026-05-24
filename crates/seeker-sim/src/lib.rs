//! SeekerSim library root.
//!
//! Binary entry (`main.rs`) is thin; reusable logic lives here so later phases
//! (pipeline, vision, tests) can import modules without starting HTTP.

pub mod api;
pub mod config;
pub mod domain;
pub mod guidance;
pub mod ingest;
pub mod pipeline;
pub mod sim;
pub mod telemetry;
pub mod tracking;
pub mod vision;

pub use config::AppConfig;

use std::net::{AddrParseError, SocketAddr};
use thiserror::Error;

/// Errors that can occur while starting or running the HTTP server.
#[derive(Debug, Error)]
pub enum RunError {
    /// `[server].bind` in config is not a valid `host:port`.
    #[error("invalid bind address '{bind}': {source}")]
    InvalidBind {
        bind: String,
        source: AddrParseError,
    },

    /// TCP bind failed (port in use, permission denied, etc.).
    #[error("failed to bind {address}: {source}")]
    Bind {
        address: SocketAddr,
        source: std::io::Error,
    },

    /// Axum server exited unexpectedly.
    #[error("HTTP server error: {0}")]
    Serve(#[from] std::io::Error),
}

/// Starts the HTTP server using the supplied configuration.
///
/// Call [`telemetry::init`] once before invoking this function.
///
/// # Arguments
/// * `config` — Parsed application config (bind address, future sections).
///
/// # Returns
/// Runs until shutdown; normally only returns on error.
///
/// # C# analogy
/// `IHost.RunAsync()` — blocks the async runtime serving requests.
pub async fn run(config: AppConfig) -> Result<(), RunError> {
    let address = config.server.socket_addr().map_err(|source| RunError::InvalidBind {
        bind: config.server.bind.clone(),
        source,
    })?;
    let router = api::router();

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| RunError::Bind { address, source })?;

    tracing::info!(
        %address,
        bind = %config.server.bind,
        "seeker-sim listening"
    );

    axum::serve(listener, router).await?;

    Ok(())
}
