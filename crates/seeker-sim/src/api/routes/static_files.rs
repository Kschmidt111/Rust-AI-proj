//! Static web UI — `api/static/` served at site root (Phase 6A+).
//!
//! `index_handler` serves `index.html` with strict no-cache headers (local dev).
//! Other assets use `ServeDir` fallback in `routes/mod.rs`.

use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;

/// Absolute path to `crates/seeker-sim/src/api/static/`.
pub fn static_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/api/static")
}

/// Response headers that discourage browsers from caching the HTML shell.
fn no_cache_headers() -> [(header::HeaderName, HeaderValue); 3] {
    [
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        ),
        (header::PRAGMA, HeaderValue::from_static("no-cache")),
        (header::EXPIRES, HeaderValue::from_static("0")),
    ]
}

/// Serves `index.html` for `/` and `/index.html` (re-read from disk each request).
///
/// # Returns
/// HTML with `Cache-Control: no-store` so UI updates show without incognito.
pub async fn index_handler() -> Response {
    let path = static_root().join("index.html");
    match tokio::fs::read_to_string(&path).await {
        Ok(body) => (
            no_cache_headers(),
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
            body,
        )
            .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to read index.html: {err}"),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn root_serves_index_html() {
        let app = full_app();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn root_index_is_not_cached_by_server_headers() {
        let app = full_app();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let cache = response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            cache.contains("no-store"),
            "expected no-store Cache-Control, got {cache:?}"
        );
    }

    #[tokio::test]
    async fn health_still_works_with_static_fallback() {
        let app = full_app();
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

    #[tokio::test]
    async fn static_assets_exist_on_disk() {
        let root = static_root();
        assert!(root.join("index.html").is_file(), "missing index.html");
        assert!(root.join("style.css").is_file(), "missing style.css");
        assert!(root.join("app.js").is_file(), "missing app.js");
    }
}
