//! HTTP API surface — routes only, no business logic.
//!
//! Each submodule under `routes/` owns one resource (`/health`, future `/v1/runs`, etc.).

mod routes;

use axum::Router;

/// Builds the complete Axum router for the service.
///
/// # C# analogy
/// `app.MapControllers()` or grouping minimal API endpoints.
pub fn router() -> Router {
    routes::router()
}
