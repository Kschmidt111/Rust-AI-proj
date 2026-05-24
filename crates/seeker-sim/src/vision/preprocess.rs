//! # Step 1 — Pre-processing: RGB image → ONNX input tensor
//!
//! YOLOv8 expects a very specific input:
//!
//! - **Shape:** `[1, 3, 640, 640]` — batch 1, 3 color channels, 640×640 pixels
//! - **Type:** `float32` values in range **0.0 to 1.0** (divide uint8 by 255)
//! - **Layout:** **NCHW** (channels first), not HWC like a typical pixel buffer
//!
//! Your photo is almost never exactly 640×640, so we use **letterboxing**.
//!
//! ## Letterboxing (why not stretch?)
//!
//! Naive stretch distorts aspect ratio — circles become ellipses, and the model
//! was trained on undistorted letterboxed images.
//!
//! ```text
//! Original 810×1080 (portrait)          Letterboxed 640×640
//! ┌────────────────┐                    ┌────────────────┐
//! │                │                    │████ padding ███│  ← black bars (we use 0.0)
//! │     photo      │   ──scale──>       │┌──────────────┐│
//! │                │                    ││ scaled photo ││  ← scaled to fit inside 640
//! │                │                    │└──────────────┘│
//! └────────────────┘                    │████ padding ███│
//!                                         └────────────────┘
//!
//! scale = min(640/810, 640/1080)  →  use the tighter constraint so image fits entirely
//! pad_x, pad_y = center the scaled image on the square canvas
//! ```
//!
//! We save `scale`, `pad_x`, `pad_y` in [`LetterboxMeta`] because post-processing must
//! **invert** this transform to draw boxes on the original photo.
//!
//! ## NCHW vs HWC (channel order)
//!
//! ```text
//! HWC (typical image memory):  pixel = [R, G, B, R, G, B, ...]  per row
//! CHW (ONNX expects):          all R plane, then all G, then all B
//!
//! tensor[idx(c,y,x)] = channel c at pixel (x,y)
//! ```
//!
//! # C# analogy
//! Like ML.NET `ImageFeaturizer`: resize → normalize → transpose dimensions before `Predict()`.

use crate::vision::decode::RgbImageData;

/// Metadata needed to map model output coordinates back to the **original** image.
///
/// YOLO outputs box centers in **letterboxed 640×640 space**. Post-processing
/// subtracts padding and divides by scale to get original pixel coordinates.
#[derive(Debug, Clone, Copy)]
pub struct LetterboxMeta {
    /// Original image width before letterbox (pixels).
    pub orig_width: u32,
    /// Original image height before letterbox (pixels).
    pub orig_height: u32,
    /// Uniform scale factor applied during resize (same for x and y).
    pub scale: f32,
    /// Horizontal padding added on the left of the scaled image (pixels in 640-space).
    pub pad_x: f32,
    /// Vertical padding added on the top of the scaled image (pixels in 640-space).
    pub pad_y: f32,
    /// Model input side length (usually 640 from config).
    pub input_size: u32,
}

/// Result of preprocessing: the float tensor ONNX expects + letterbox parameters.
pub struct PreprocessOutput {
    /// Flattened NCHW tensor: length = `3 * input_size * input_size`.
    /// Values are RGB / 255.0, range [0.0, 1.0]. Padding pixels are 0.0 (black).
    pub tensor: Vec<f32>,
    /// Letterbox parameters — pass this to post-processing for coordinate inverse transform.
    pub meta: LetterboxMeta,
}

/// Converts an RGB image into a YOLO-style input tensor.
///
/// # Algorithm
/// 1. **Compute scale** — `min(input_size/w, input_size/h)` so the whole image fits
/// 2. **Resize** — bilinear-style resize (`FilterType::Triangle`) to `(new_w, new_h)`
/// 3. **Embed in square canvas** — iterate every `(x,y)` in 640×640; sample resized
///    image or write padding (0) if outside the scaled region
/// 4. **Normalize** — `pixel / 255.0`
/// 5. **Reorder to CHW** — store channel 0 (R) plane, then G, then B
///
/// # Arguments
/// * `image` — decoded RGB from [`super::decode::load_rgb_image`]
/// * `input_size` — model square side (640 from `config/default.toml`)
///
/// # C# analogy
/// ```csharp
/// // Pseudocode equivalent
/// float scale = Math.Min(640f / width, 640f / height);
/// var resized = ResizeImage(image, (int)(width * scale), (int)(height * scale));
/// var tensor = new float[1 * 3 * 640 * 640];
/// // nested loops: pad, normalize, CHW layout...
/// ```
pub fn letterbox_preprocess(image: &RgbImageData, input_size: u32) -> PreprocessOutput {
    let w = image.width as f32;
    let h = image.height as f32;
    let size = input_size as f32;

    // Pick the smaller scale factor so neither dimension exceeds input_size.
    // Example: 810×1080 → scale = min(640/810, 640/1080) ≈ 0.593 → new size 480×640
    let scale = (size / w).min(size / h);
    let new_w = (w * scale).round() as u32;
    let new_h = (h * scale).round() as u32;

    // Resize the actual image content (aspect ratio preserved).
    let resized = image::imageops::resize(
        &image.pixels,
        new_w,
        new_h,
        image::imageops::FilterType::Triangle,
    );

    // Integer division centers the content; stored as f32 for sub-pixel math in postprocess.
    let pad_x = ((input_size - new_w) / 2) as f32;
    let pad_y = ((input_size - new_h) / 2) as f32;

    // Pre-allocate: 3 channels × H × W floats, initialized to 0.0 (padding color).
    let mut tensor = vec![0.0_f32; (3 * input_size * input_size) as usize];

    // Walk every pixel of the 640×640 model input canvas.
    for y in 0..input_size {
        for x in 0..input_size {
            // Map canvas (x,y) back to coordinates in the *resized* image.
            // Subtract pad because the resized image is centered inside the canvas.
            let rx = x as i32 - pad_x as i32;
            let ry = y as i32 - pad_y as i32;

            // Inside the resized image region → sample RGB; outside → padding (black).
            let (r, g, b) = if rx >= 0 && ry >= 0 && (rx as u32) < new_w && (ry as u32) < new_h {
                let p = resized.get_pixel(rx as u32, ry as u32);
                (p[0], p[1], p[2])
            } else {
                (0u8, 0u8, 0u8)
            };

            // CHW indexing: channel c owns a contiguous H×W slice.
            // idx(0) = all reds, idx(1) = all greens, idx(2) = all blues.
            let idx = |c: u32| (c * input_size * input_size + y * input_size + x) as usize;
            tensor[idx(0)] = r as f32 / 255.0;
            tensor[idx(1)] = g as f32 / 255.0;
            tensor[idx(2)] = b as f32 / 255.0;
        }
    }

    PreprocessOutput {
        tensor,
        meta: LetterboxMeta {
            orig_width: image.width,
            orig_height: image.height,
            scale,
            pad_x,
            pad_y,
            input_size,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::decode::RgbImageData;
    use image::RgbImage;

    #[test]
    fn tensor_length_matches_nchw() {
        let img = RgbImageData {
            pixels: RgbImage::new(100, 50),
            width: 100,
            height: 50,
        };
        let out = letterbox_preprocess(&img, 640);
        // batch=1 (implicit), C=3, H=640, W=640 → 1*3*640*640 floats
        assert_eq!(out.tensor.len(), 1 * 3 * 640 * 640);
        assert_eq!(out.meta.input_size, 640);
    }
}
