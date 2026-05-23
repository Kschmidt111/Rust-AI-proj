//! Load image bytes from disk into an in-memory RGB buffer.

use crate::domain::VisionError;
use image::{ImageReader, RgbImage};
use std::path::Path;

/// Decoded RGB image ready for preprocessing.
pub struct RgbImageData {
    pub pixels: RgbImage,
    pub width: u32,
    pub height: u32,
}

/// Reads and decodes an image file to RGB8.
///
/// # Arguments
/// * `path` — Path to `.jpg`, `.jpeg`, or `.png`.
///
/// # C# analogy
/// `Image.FromFile(path)` then lock bits into a byte array.
pub fn load_rgb_image(path: &Path) -> Result<RgbImageData, VisionError> {
    let reader = ImageReader::open(path).map_err(|source| VisionError::ReadImage {
        path: path.to_path_buf(),
        source,
    })?;

    let img = reader
        .decode()
        .map_err(|e| VisionError::DecodeImage(e.to_string()))?
        .into_rgb8();

    let width = img.width();
    let height = img.height();

    Ok(RgbImageData {
        pixels: img,
        width,
        height,
    })
}
