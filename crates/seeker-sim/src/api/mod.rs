//! HTTP API surface — routes only, no business logic.
//!
//! Each submodule under `routes/` owns one resource (`/health`, `/v1/sim/run`, etc.).

mod routes;
pub mod state;

use crate::AppConfig;
use axum::Router;
pub use state::AppState;

/// Builds the complete Axum router for the service.
///
/// # Arguments
/// * `config` — Loaded application config (injected into handlers via [`AppState`]).
///
/// # C# analogy
/// `app.MapControllers()` or grouping minimal API endpoints.
pub fn router(config: AppConfig) -> Router {
    routes::router(AppState::new(config))
}
