//! Frame differencing for small bright movers (Phase 4D).
//!
//! Compares consecutive grayscale frames and returns the centroid of pixels
//! that **brightened** since the previous frame — suited to the white-dot-on-blue
//! synthetic video from `scripts/generate-dot-video.ps1`.

use crate::vision::decode::RgbImageData;

/// Stateful motion detector: holds the previous frame's luma buffer.
///
/// Call [`Self::detect`] once per frame in order. The first frame always
/// returns `None` (nothing to diff against).
///
/// # C# analogy
/// A class with `_previousFrame` updated each tick — like a background-subtraction
/// helper in a game loop before you pass centroids to the tracker.
#[derive(Debug, Clone)]
pub struct MotionDetector {
    threshold: f32,
    min_pixels: u32,
    prev_luma: Option<Vec<u8>>,
    width: u32,
    height: u32,
}

impl MotionDetector {
    /// Creates a detector with default threshold `0.02` (≈5/255 luma step).
    pub fn new() -> Self {
        Self::with_threshold(0.02)
    }

    /// Creates a detector with an explicit brightness-change threshold.
    ///
    /// # Arguments
    /// * `threshold` — fraction of full scale `[0.0, 1.0]`; pixel must brighten by
    ///   at least `threshold * 255` vs the previous frame.
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            threshold,
            min_pixels: 4,
            prev_luma: None,
            width: 0,
            height: 0,
        }
    }

    /// Minimum changed pixels required before returning a centroid (noise guard).
    pub fn with_min_pixels(mut self, min_pixels: u32) -> Self {
        self.min_pixels = min_pixels;
        self
    }

    /// Diffs against the previous frame and returns the motion centroid, if any.
    ///
    /// # Arguments
    /// * `frame` — current RGB frame (same size as previous when diffing).
    ///
    /// # Returns
    /// `Some((cx, cy))` in image pixels (sub-pixel float from pixel average),
    /// or `None` on the first frame, when nothing moved enough, or on size change.
    pub fn detect(&mut self, frame: &RgbImageData) -> Option<(f32, f32)> {
        let luma = rgb_to_luma(&frame.pixels);

        let result = if let Some(ref prev) = self.prev_luma {
            if prev.len() != luma.len() {
                None
            } else {
                motion_centroid(prev, &luma, frame.width, frame.height, self.threshold, self.min_pixels)
            }
        } else {
            None
        };

        self.prev_luma = Some(luma);
        self.width = frame.width;
        self.height = frame.height;

        result
    }
}

impl Default for MotionDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Centroid of pixels that brightened between two equal-length luma buffers.
///
/// Uses **positive diff only** so a white dot on a dark sky produces one blob
/// at the new dot position instead of ghosts at both old and new locations.
///
/// # Arguments
/// * `prev` / `curr` — row-major luma `0..=255`, length `width * height`.
/// * `threshold` — brighten-by fraction of 255 (same as [`MotionDetector`]).
/// * `min_pixels` — reject tiny noise specks below this count.
pub fn motion_centroid(
    prev: &[u8],
    curr: &[u8],
    width: u32,
    height: u32,
    threshold: f32,
    min_pixels: u32,
) -> Option<(f32, f32)> {
    if prev.len() != curr.len() || width == 0 || height == 0 {
        return None;
    }

    let step = (threshold * 255.0).max(1.0) as i16;
    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut count = 0_u32;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let delta = curr[idx] as i16 - prev[idx] as i16;
            if delta <= step {
                continue;
            }
            sum_x += f64::from(x);
            sum_y += f64::from(y);
            count += 1;
        }
    }

    if count < min_pixels {
        return None;
    }

    let n = f64::from(count);
    Some(((sum_x / n) as f32, (sum_y / n) as f32))
}

// Converts RGB8 to luma (ITU-R BT.601 weights).
fn rgb_to_luma(pixels: &image::RgbImage) -> Vec<u8> {
    pixels
        .pixels()
        .map(|rgb| {
            let r = f32::from(rgb[0]);
            let g = f32::from(rgb[1]);
            let b = f32::from(rgb[2]);
            (0.299 * r + 0.587 * g + 0.114 * b).round() as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    const SKY: Rgb<u8> = Rgb([26, 58, 92]);

    fn solid_frame(width: u32, height: u32, color: Rgb<u8>) -> RgbImageData {
        let pixels = RgbImage::from_pixel(width, height, color);
        RgbImageData {
            width,
            height,
            pixels,
        }
    }

    fn frame_with_dot(
        width: u32,
        height: u32,
        bg: Rgb<u8>,
        cx: i32,
        cy: i32,
        radius: i32,
    ) -> RgbImageData {
        let mut pixels = RgbImage::from_pixel(width, height, bg);
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= radius * radius {
                    pixels.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                }
            }
        }
        RgbImageData {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn first_frame_returns_none() {
        let mut detector = MotionDetector::new();
        let frame = solid_frame(64, 48, SKY);
        assert!(detector.detect(&frame).is_none());
    }

    #[test]
    fn static_scene_returns_none_after_warmup() {
        let mut detector = MotionDetector::new();
        let frame = solid_frame(64, 48, SKY);
        detector.detect(&frame);
        assert!(detector.detect(&frame).is_none());
    }

    #[test]
    fn moved_dot_produces_centroid_near_new_position() {
        let mut detector = MotionDetector::new();
        let bg = SKY;
        let frame0 = frame_with_dot(640, 480, bg, 50, 120, 6);
        let frame1 = frame_with_dot(640, 480, bg, 55, 123, 6);

        detector.detect(&frame0);
        let centroid = detector.detect(&frame1).expect("dot moved");

        assert!((centroid.0 - 55.0).abs() < 3.0, "cx={}", centroid.0);
        assert!((centroid.1 - 123.0).abs() < 3.0, "cy={}", centroid.1);
    }

    #[test]
    fn motion_centroid_rejects_noise_specks() {
        let w = 4_u32;
        let h = 4_u32;
        let prev = vec![0_u8; 16];
        let mut curr = vec![0_u8; 16];
        curr[5] = 40; // single pixel

        assert!(motion_centroid(&prev, &curr, w, h, 0.02, 4).is_none());
    }
}
