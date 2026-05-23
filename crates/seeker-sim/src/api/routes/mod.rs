//! Route modules aggregated into one router.

mod health;

use axum::Router;

/// Combines all HTTP routes. Add `.merge(...)` here as new endpoints appear.
pub fn router() -> Router {
    Router::new().merge(health::router())
}
