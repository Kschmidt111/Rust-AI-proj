//! `POST /v1/runs`, `GET /v1/runs/{id}`, artifact download (Phase 6D).

use crate::api::state::AppState;
use crate::pipeline::{
    load_run_status, resolve_artifact_path, resolve_input_path, run_folder_pipeline,
    CreateRunRequest, FolderRunError, RunStatusResponse,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::instrument;

/// JSON error body for run API failures.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

/// Handles `POST /v1/runs` — runs track or intercept pipeline on a frame folder.
#[instrument(name = "http.create_run", skip(state, request))]
pub async fn create_run_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<RunStatusResponse>), RunsHttpError> {
    let folder = resolve_input_path(&request.input_path)?;
    let config = state.config.clone();
    let mode = request.mode.clone();

    tracing::info!(
        input = %folder.display(),
        mode = %mode,
        "pipeline run requested"
    );

    let status = tokio::task::spawn_blocking(move || run_folder_pipeline(&config, &folder, &mode))
        .await
        .map_err(|e| RunsHttpError::Internal(e.to_string()))?
        .map_err(RunsHttpError::from)?;

    tracing::info!(
        run_id = %status.run_id,
        frames = ?status.frame_count,
        "pipeline run complete"
    );

    Ok((StatusCode::CREATED, Json(status)))
}

/// Handles `GET /v1/runs/{run_id}`.
#[instrument(name = "http.get_run", skip(state))]
pub async fn get_run_handler(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<RunStatusResponse>, RunsHttpError> {
    let status = load_run_status(&state.config, &run_id)?;
    Ok(Json(status))
}

/// Handles `GET /v1/runs/{run_id}/artifacts/{file_name}`.
#[instrument(name = "http.get_artifact", skip(state))]
pub async fn get_artifact_handler(
    State(state): State<AppState>,
    Path((run_id, file_name)): Path<(String, String)>,
) -> Result<Response, RunsHttpError> {
    let path = resolve_artifact_path(&state.config, &run_id, &file_name)?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| RunsHttpError::Internal(e.to_string()))?;

    let content_type = match file_name.as_str() {
        "trajectory.png" => "image/png",
        _ => "text/csv",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .expect("valid response"))
}

/// HTTP-layer errors for run routes.
#[derive(Debug)]
pub(crate) enum RunsHttpError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl From<FolderRunError> for RunsHttpError {
    fn from(err: FolderRunError) -> Self {
        match err {
            FolderRunError::InvalidInputPath(msg)
            | FolderRunError::UnknownMode(msg)
            | FolderRunError::InvalidRunId(msg)
            | FolderRunError::InvalidArtifact(msg) => RunsHttpError::BadRequest(msg),
            FolderRunError::RunNotFound(id) => RunsHttpError::NotFound(id),
            FolderRunError::Pipeline(e) => RunsHttpError::Internal(e.to_string()),
            FolderRunError::Io(e) => RunsHttpError::Internal(e.to_string()),
        }
    }
}

impl IntoResponse for RunsHttpError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            RunsHttpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            RunsHttpError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            RunsHttpError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
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
    use crate::pipeline::{repo_root, RunStatusResponse};
    use image::{Rgb, RgbImage};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("seeker_{prefix}_{nanos}"))
    }

    fn write_dot_sequence(dir: &PathBuf, count: usize) {
        fs::create_dir_all(dir).unwrap();
        let bg = Rgb([26, 58, 92]);
        for i in 0..count {
            let mut pixels = RgbImage::from_pixel(640, 480, bg);
            let cx = 40 + i as i32 * 5;
            let cy = 120 + i as i32 * 3;
            for y in 0..480_i32 {
                for x in 0..640_i32 {
                    let dx = x - cx;
                    let dy = y - cy;
                    if dx * dx + dy * dy <= 36 {
                        pixels.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                    }
                }
            }
            pixels.save(dir.join(format!("{:04}.png", i + 1))).unwrap();
        }
    }

    fn full_app() -> Router {
        router(AppConfig::load().expect("config"))
    }

    #[tokio::test]
    async fn post_runs_rejects_bad_mode() {
        let app = full_app();
        let body = r#"{"input_path":"data/frames/x","mode":"nope"}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_run_not_found() {
        let app = full_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/runs/run_9999999999999999999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn post_and_get_run_round_trip() {
        let frames_dir = repo_root().join("data/frames/http_run_test");
        if frames_dir.exists() {
            let _ = fs::remove_dir_all(&frames_dir);
        }
        write_dot_sequence(&frames_dir, 10);

        let rel = "data/frames/http_run_test";
        let app = full_app();
        let post_body = format!(r#"{{"input_path":"{rel}","mode":"track"}}"#);

        let post = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(post_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(post.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(post.into_body(), usize::MAX)
            .await
            .unwrap();
        let created: RunStatusResponse = serde_json::from_slice(&body).unwrap();

        let get = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/runs/{}", created.run_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get.status(), StatusCode::OK);

        let _ = fs::remove_dir_all(&frames_dir);
    }
}
