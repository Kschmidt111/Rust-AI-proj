//! `GET /health` — liveness probe for orchestrators and local dev.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use tracing::instrument;

/// JSON body returned by `GET /health`.
///
/// # C# analogy
/// A small response DTO / record type.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}

/// Registers health routes on the supplied router chain.
pub fn router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// Handles `GET /health`.
///
/// # Returns
/// HTTP 200 with `{ "status": "ok", "service": "seeker-sim" }`.
#[instrument(name = "http.health", skip_all)]
pub async fn health_handler() -> Json<HealthResponse> {
    tracing::debug!("health check requested");

    Json(HealthResponse {
        status: "ok",
        service: "seeker-sim",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_ok_json() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
