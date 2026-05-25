//! Shared HTTP handler state (config loaded once at startup).

use crate::AppConfig;

/// Application state injected into Axum handlers.
///
/// # C# analogy
/// `IOptions<AppSettings>` or a scoped service registered in DI.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Parsed TOML configuration.
    pub config: AppConfig,
}

impl AppState {
    /// Wraps loaded config for route handlers.
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}
