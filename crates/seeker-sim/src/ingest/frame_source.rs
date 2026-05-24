//! Discover and iterate sorted frame paths from a folder on disk.
//!
//! Phase 3 reads **pre-extracted PNG/JPEG sequences** (from ffmpeg or our dot generator).
//! Video decoding in-process is deferred to Phase 6 (optional OpenCV / direct ffmpeg).

use crate::ingest::IngestError;
use std::path::{Path, PathBuf};

/// Where frames come from. Phase 3 supports folder only; video/webcam come later.
///
/// # C# analogy
/// Like an `IEnumerable<IFrameSource>` strategy — today only `FolderFrameSource`.
#[derive(Debug, Clone)]
pub enum FrameSource {
    /// Sorted image files under a directory (e.g. `data/frames/run_001/`).
    Folder(PathBuf),
}

impl FrameSource {
    /// Creates a folder-based source from an on-disk path.
    pub fn folder(path: impl Into<PathBuf>) -> Self {
        Self::Folder(path.into())
    }

    /// Collects all frame file paths in **sorted order** (lexicographic).
    ///
    /// Uses `%04d.png`-style names so `0002` sorts before `0010`.
    /// Accepts `.png`, `.jpg`, `.jpeg` (case-insensitive).
    ///
    /// # Errors
    /// * [`IngestError::FolderNotFound`] — path does not exist
    /// * [`IngestError::EmptyFolder`] — folder exists but has no image frames
    /// * [`IngestError::ReadDir`] — OS permission or I/O failure
    pub fn collect_paths(&self) -> Result<Vec<PathBuf>, IngestError> {
        match self {
            FrameSource::Folder(dir) => collect_image_paths(dir),
        }
    }
}

/// Returns sorted paths to PNG/JPEG files directly under `dir` (non-recursive).
fn collect_image_paths(dir: &Path) -> Result<Vec<PathBuf>, IngestError> {
    if !dir.is_dir() {
        return Err(IngestError::FolderNotFound {
            path: dir.to_path_buf(),
        });
    }

    let mut paths = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(|source| IngestError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| IngestError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path.is_file() && is_frame_image(&path) {
            paths.push(path);
        }
    }

    if paths.is_empty() {
        return Err(IngestError::EmptyFolder {
            path: dir.to_path_buf(),
        });
    }

  // Lexicographic sort — correct for zero-padded ffmpeg output (`0001.png`, `0002.png`, …).
    paths.sort();

    Ok(paths)
}

/// True for common still-frame extensions used in this project.
fn is_frame_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("seeker_{prefix}_{nanos}"))
    }

    #[test]
    fn sorts_frames_lexicographically() {
        let dir = temp_dir("frames");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("0010.png"), b"x").unwrap();
        fs::write(dir.join("0002.png"), b"x").unwrap();
        fs::write(dir.join("0001.png"), b"x").unwrap();
        fs::write(dir.join("readme.txt"), b"skip").unwrap();

        let paths = collect_image_paths(&dir).unwrap();
        assert_eq!(paths.len(), 3);
        assert!(paths[0].ends_with("0001.png"));
        assert!(paths[1].ends_with("0002.png"));
        assert!(paths[2].ends_with("0010.png"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_folder_errors() {
        let dir = temp_dir("empty");
        fs::create_dir_all(&dir).unwrap();
        let err = collect_image_paths(&dir).unwrap_err();
        assert!(matches!(err, IngestError::EmptyFolder { .. }));
        let _ = fs::remove_dir_all(&dir);
    }
}
