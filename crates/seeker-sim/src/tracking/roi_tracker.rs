//! ROI-local motion search around Kalman prediction (Phase 4).
//!
//! After track lock, diff only a window around the predicted center instead of
//! the full frame — reduces noise and matches ADR-017 hybrid perception.

use crate::vision::decode::RgbImageData;
use crate::vision::motion::motion_centroid;

/// Motion differencing with full-frame (acquire) or ROI (track) modes.
///
/// Maintains the previous frame luma buffer; call exactly **one** detect method
/// per frame.
///
/// # C# analogy
/// A tracker helper that switches between full-image motion and a cropped
/// search window around the predicted target.
#[derive(Debug, Clone)]
pub struct RoiMotionTracker {
    threshold: f32,
    min_pixels: u32,
    prev_luma: Option<Vec<u8>>,
    width: u32,
    height: u32,
}

impl RoiMotionTracker {
    /// Creates a tracker with motion threshold and minimum changed-pixel count.
    pub fn with_settings(threshold: f32, min_pixels: u32) -> Self {
        Self {
            threshold,
            min_pixels,
            prev_luma: None,
            width: 0,
            height: 0,
        }
    }

    /// Full-frame diff for acquisition / re-acquire (no active track yet).
    pub fn detect_global(&mut self, frame: &RgbImageData) -> Option<(f32, f32)> {
        self.detect(frame, None)
    }

    /// ROI diff centered at `(cx, cy)` with square half-width `half_size_px`.
    ///
    /// Use the Kalman **predicted** position after [`PointTracker::begin_frame`].
    pub fn detect_roi(
        &mut self,
        frame: &RgbImageData,
        cx: f32,
        cy: f32,
        half_size_px: u32,
    ) -> Option<(f32, f32)> {
        self.detect(frame, Some((cx, cy, half_size_px)))
    }

    fn detect(
        &mut self,
        frame: &RgbImageData,
        roi: Option<(f32, f32, u32)>,
    ) -> Option<(f32, f32)> {
        let luma = frame_to_luma(&frame.pixels);

        let result = if let Some(ref prev) = self.prev_luma {
            if prev.len() != luma.len() {
                None
            } else if let Some((cx, cy, half)) = roi {
                motion_centroid_roi(
                    prev,
                    &luma,
                    frame.width,
                    frame.height,
                    cx,
                    cy,
                    half,
                    self.threshold,
                    self.min_pixels,
                )
            } else {
                motion_centroid(
                    prev,
                    &luma,
                    frame.width,
                    frame.height,
                    self.threshold,
                    self.min_pixels,
                )
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

/// Motion centroid inside a square ROI; coordinates returned in **full image** space.
pub fn motion_centroid_roi(
    prev: &[u8],
    curr: &[u8],
    width: u32,
    height: u32,
    center_x: f32,
    center_y: f32,
    half_size_px: u32,
    threshold: f32,
    min_pixels: u32,
) -> Option<(f32, f32)> {
    if prev.len() != curr.len() || width == 0 || height == 0 {
        return None;
    }

    let half = half_size_px as i32;
    let cx = center_x.round() as i32;
    let cy = center_y.round() as i32;

    let x0 = (cx - half).clamp(0, width as i32 - 1) as u32;
    let y0 = (cy - half).clamp(0, height as i32 - 1) as u32;
    let x1 = (cx + half).clamp(0, width as i32 - 1) as u32;
    let y1 = (cy + half).clamp(0, height as i32 - 1) as u32;

    let step = (threshold * 255.0).max(1.0) as i16;
    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut count = 0_u32;

    for y in y0..=y1 {
        for x in x0..=x1 {
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

fn frame_to_luma(pixels: &image::RgbImage) -> Vec<u8> {
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

    fn frame_with_dot(cx: i32, cy: i32) -> RgbImageData {
        let mut pixels = RgbImage::from_pixel(640, 480, SKY);
        for y in 0..480_i32 {
            for x in 0..640_i32 {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= 36 {
                    pixels.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                }
            }
        }
        RgbImageData {
            width: 640,
            height: 480,
            pixels,
        }
    }

    #[test]
    fn roi_detects_dot_inside_window() {
        let mut tracker = RoiMotionTracker::with_settings(0.02, 4);
        let f0 = frame_with_dot(50, 120);
        let f1 = frame_with_dot(55, 123);

        tracker.detect_global(&f0);
        let c = tracker
            .detect_roi(&f1, 55.0, 123.0, 32)
            .expect("dot in roi");

        assert!((c.0 - 55.0).abs() < 4.0);
        assert!((c.1 - 123.0).abs() < 4.0);
    }

    #[test]
    fn roi_ignores_dot_outside_window() {
        let mut tracker = RoiMotionTracker::with_settings(0.02, 4);
        let f0 = frame_with_dot(50, 120);
        let f1 = frame_with_dot(55, 123);

        tracker.detect_global(&f0);
        let c = tracker.detect_roi(&f1, 400.0, 300.0, 16);

        assert!(c.is_none());
    }
}
