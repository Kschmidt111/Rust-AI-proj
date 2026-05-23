//! SeekerSim entry point (Phase 1B).
//!
//! Starts a small HTTP server with a `/health` endpoint so we can verify
//! the Rust toolchain and Axum wiring before adding vision or guidance code.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;

/// Default listen address until Phase 1C loads `config/default.toml`.
const DEFAULT_BIND: &str = "127.0.0.1:8080";

/// JSON body returned by `GET /health`.
///
/// # C# analogy
/// Similar to a small DTO/record returned from an ASP.NET Core health endpoint.
#[derive(Debug, Serialize)]
struct HealthResponse {
    /// Service status string for load balancers and humans.
    status: &'static str,
    /// Crate name so clients can confirm they hit SeekerSim.
    service: &'static str,
}

/// Builds the Axum router with all HTTP routes for this phase.
///
/// # Returns
/// A `Router` ready to pass to `axum::serve`.
///
/// # C# analogy
/// Like `WebApplication` route registration (`MapGet`, etc.) before `Run()`.
fn build_router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// Handles `GET /health` — confirms the process is up and responding.
///
/// # Returns
/// HTTP 200 with JSON `{ "status": "ok", "service": "seeker-sim" }`.
///
/// # C# analogy
/// An async minimal API delegate: `() => Results.Ok(new { status = "ok" })`.
async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "seeker-sim",
    })
}

/// Parses the bind address string into a `SocketAddr`.
///
/// # Arguments
/// * `bind` — Address in `host:port` form, e.g. `127.0.0.1:8080`.
///
/// # Returns
/// Parsed socket address, or panic with a clear message if invalid.
///
/// # C# analogy
/// Like `IPEndPoint.Parse(...)` when configuring Kestrel URLs.
fn parse_bind_address(bind: &str) -> SocketAddr {
    bind.parse()
        .unwrap_or_else(|_| panic!("invalid bind address '{bind}' — expected host:port"))
}

/// Program entry: build router, bind TCP, serve forever.
///
/// # C# analogy
/// `Host.CreateDefaultBuilder` → configure pipeline → `app.Run()`.
#[tokio::main]
async fn main() {
  let addr = parse_bind_address(DEFAULT_BIND);
  let router = build_router();

  // `TcpListener` is the Rust equivalent of opening a listening socket before accept loops.
  let listener = tokio::net::TcpListener::bind(addr)
      .await
      .unwrap_or_else(|err| panic!("failed to bind {addr}: {err}"));

  println!("seeker-sim listening on http://{addr}");
  println!("try: curl http://{addr}/health");

  // `serve` runs until the process is stopped (Ctrl+C).
  axum::serve(listener, router)
      .await
      .expect("HTTP server exited with an error");
}
