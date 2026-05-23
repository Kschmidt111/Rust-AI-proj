//! Letterbox resize and normalization for YOLO input tensors.

use crate::vision::decode::RgbImageData;

/// Metadata to map model coordinates back to the original image.
#[derive(Debug, Clone, Copy)]
pub struct LetterboxMeta {
    pub orig_width: u32,
    pub orig_height: u32,
    pub scale: f32,
    pub pad_x: f32,
    pub pad_y: f32,
    pub input_size: u32,
}

/// Result of preprocessing: flat CHW tensor + letterbox parameters.
pub struct PreprocessOutput {
    /// Normalized floats in NCHW order: `[1, 3, H, W]` flattened.
    pub tensor: Vec<f32>,
    pub meta: LetterboxMeta,
}

/// Converts an RGB image into a YOLO-style input tensor.
///
/// Steps:
/// 1. **Letterbox** — scale to fit inside `input_size`×`input_size` without distortion  
/// 2. **Pad** — center on square canvas (gray 114/255 typical; we use 0.0 here)  
/// 3. **Normalize** — divide RGB by 255.0  
/// 4. **Layout** — channel-first (CHW) for ONNX  
///
/// # C# analogy
/// Similar to ML.NET image featurization: resize, normalize, reorder dimensions.
pub fn letterbox_preprocess(image: &RgbImageData, input_size: u32) -> PreprocessOutput {
    let w = image.width as f32;
    let h = image.height as f32;
    let size = input_size as f32;

    let scale = (size / w).min(size / h);
    let new_w = (w * scale).round() as u32;
    let new_h = (h * scale).round() as u32;

    let resized = image::imageops::resize(
        &image.pixels,
        new_w,
        new_h,
        image::imageops::FilterType::Triangle,
    );

    let pad_x = ((input_size - new_w) / 2) as f32;
    let pad_y = ((input_size - new_h) / 2) as f32;

    let mut tensor = vec![0.0_f32; (3 * input_size * input_size) as usize];

    // CHW layout
    for y in 0..input_size {
        for x in 0..input_size {
            let rx = x as i32 - pad_x as i32;
            let ry = y as i32 - pad_y as i32;

            let (r, g, b) = if rx >= 0 && ry >= 0 && (rx as u32) < new_w && (ry as u32) < new_h {
                let p = resized.get_pixel(rx as u32, ry as u32);
                (p[0], p[1], p[2])
            } else {
                (0u8, 0u8, 0u8)
            };

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
        assert_eq!(out.tensor.len(), 1 * 3 * 640 * 640);
        assert_eq!(out.meta.input_size, 640);
    }
}
