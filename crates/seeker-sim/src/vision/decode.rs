//! # Step 0 — Decode image file to RGB pixels
//!
//! Before any ML happens, we need a uniform in-memory representation: an RGB pixel
//! buffer with known width and height.
//!
//! ## Why RGB?
//!
//! - JPEG and PNG on disk are **compressed** (JPEG = lossy DCT, PNG = deflate).
//! - The `image` crate decompresses them into raw **8-bit RGB** triplets per pixel.
//! - YOLO was trained on RGB (not BGR like old OpenCV defaults).
//!
//! ## Pixel layout in memory
//!
//! ```text
//! RgbImage stores pixels row-by-row, top-left origin:
//!
//!   (0,0) ── x increases ──>
//!     │
//!     y
//!     │
//!     ▼
//!
//! Each pixel = [R, G, B] as u8 values 0–255.
//! ```
//!
//! This matches how monitors and most web images work: origin top-left, x right, y down.
//! Guidance math later (line-of-sight angle) will use the same convention.
//!
//! # C# analogy
//! `Image.FromFile(path)` → lock bits → copy into a `byte[]` or `Bitmap` with known width/height.

use crate::domain::VisionError;
use image::{ImageReader, RgbImage};
use std::path::Path;

/// Decoded RGB image ready for preprocessing.
///
/// Wraps the `image` crate's [`RgbImage`] plus explicit width/height so callers
/// don't have to query the inner buffer repeatedly.
pub struct RgbImageData {
    /// Raw pixel buffer (owned by Rust — no GC, no unmanaged pointer lifetime issues).
    pub pixels: RgbImage,
    /// Image width in pixels (columns).
    pub width: u32,
    /// Image height in pixels (rows).
    pub height: u32,
}

/// Reads and decodes an image file to RGB8.
///
/// Supports common formats the `image` crate handles: `.jpg`, `.jpeg`, `.png`, etc.
///
/// # Arguments
/// * `path` — Path to the image file on disk.
///
/// # Errors
/// * [`VisionError::ReadImage`] — file missing or unreadable
/// * [`VisionError::DecodeImage`] — file exists but is corrupt or unsupported format
///
/// # C# analogy
/// ```csharp
/// using var bitmap = new Bitmap(path);
/// // then extract RGB bytes from bitmap.GetPixel or LockBits
/// ```
pub fn load_rgb_image(path: &Path) -> Result<RgbImageData, VisionError> {
    // ImageReader detects format from extension or magic bytes.
    let reader = ImageReader::open(path).map_err(|source| VisionError::ReadImage {
        path: path.to_path_buf(),
        source,
    })?;

    // decode() decompresses; into_rgb8() converts palette/grayscale/RGBA → RGB.
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
