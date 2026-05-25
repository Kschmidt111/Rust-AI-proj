//! Route modules aggregated into one router.

mod health;
mod runs;
mod sim;
mod static_files;

use crate::api::state::AppState;
use axum::http::{header, HeaderValue};
use axum::{routing::{get, post}, Router};
use static_files::{index_handler, static_root};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

/// UI static asset version — bump in `index.html` query strings when HTML/JS/CSS change.
pub const STATIC_ASSET_VERSION: &str = "6e-replay-1";

/// Combines all HTTP routes with shared application state.
pub fn router(state: AppState) -> Router {
    // No `append_index_html` — `/` is served by `index_handler` with no-cache headers.
    let static_service = ServeDir::new(static_root());

    Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .route("/health", get(health::health_handler))
        .route("/v1/sim/run", post(sim::sim_run_handler))
        .route("/v1/runs", post(runs::create_run_handler))
        .route("/v1/runs/{run_id}", get(runs::get_run_handler))
        .route(
            "/v1/runs/{run_id}/artifacts/{file_name}",
            get(runs::get_artifact_handler),
        )
        .fallback_service(static_service)
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        ))
        .with_state(state)
}
