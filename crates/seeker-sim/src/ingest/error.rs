//! Errors while discovering frame files on disk.

use thiserror::Error;

/// Errors from [`super::frame_source`] when building a frame list.
#[derive(Debug, Error)]
pub enum IngestError {
    #[error("frame folder not found: '{path}'")]
    FolderNotFound { path: std::path::PathBuf },

    #[error("no PNG/JPEG frames found in '{path}'")]
    EmptyFolder { path: std::path::PathBuf },

    #[error("failed to read directory '{path}': {source}")]
    ReadDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}
