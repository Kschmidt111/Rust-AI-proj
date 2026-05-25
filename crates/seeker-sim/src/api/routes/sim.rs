//! `POST /v1/sim/run` — pure kinematic sim (Phase 6B).

use crate::api::state::AppState;
use crate::pipeline::{run_pure_sim, SimRunError, SimRunRequest, SimRunResponse};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::instrument;

/// JSON error body for failed sim runs.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

/// Handles `POST /v1/sim/run`.
///
/// # Returns
/// HTTP 200 with frame snapshots, or 400 when request parameters are invalid.
#[instrument(name = "http.sim_run", skip(state, request))]
pub async fn sim_run_handler(
    State(state): State<AppState>,
    Json(request): Json<SimRunRequest>,
) -> Result<Json<SimRunResponse>, SimRunHttpError> {
    tracing::debug!(
        law = %request.law,
        target_x = request.target_x,
        target_y = request.target_y,
        "sim run requested"
    );

    let response = run_pure_sim(&state.config, &request)?;
    tracing::info!(
        law = %response.law,
        frame_count = response.frame_count,
        min_miss = format!("{:.1}", response.min_miss_distance),
        "sim run complete"
    );
    Ok(Json(response))
}

/// HTTP-layer error for sim run validation failures.
#[derive(Debug)]
pub(crate) enum SimRunHttpError {
    BadRequest(String),
}

impl From<SimRunError> for SimRunHttpError {
    fn from(err: SimRunError) -> Self {
        SimRunHttpError::BadRequest(err.to_string())
    }
}

impl IntoResponse for SimRunHttpError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            SimRunHttpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use crate::api::router;
    use crate::config::AppConfig;
    use tower::ServiceExt;

    fn full_app() -> Router {
        router(AppConfig::load().expect("config"))
    }

    #[tokio::test]
    async fn sim_run_returns_frames_json() {
        let app = full_app();
        let body = r#"{
            "target_x": 400,
            "target_y": 80,
            "target_vx": 50,
            "target_vy": 10,
            "law": "pn"
        }"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sim/run")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn sim_run_rejects_unknown_law() {
        let app = full_app();
        let body = r#"{
            "target_x": 0,
            "target_y": 100,
            "target_vx": 0,
            "target_vy": 0,
            "law": "invalid"
        }"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/sim/run")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
