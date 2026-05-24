//! # Step 3 — Post-processing: raw ONNX tensor → human-readable detections
//!
//! The neural network does **not** return `{ "class": "person", "bbox": [...] }`.
//! It returns a large raw float tensor of **candidate predictions**. Post-processing
//! turns that into a clean list of [`Detection`] values.
//!
//! ## YOLOv8 output shape
//!
//! Ultralytics YOLOv8n at 640×640 typically outputs:
//!
//! ```text
//! shape: [1, 84, 8400]
//!         │   │    └── 8400 anchor points across the image (multi-scale grid)
//!         │   └── 84 values per candidate:
//!         │         [cx, cy, w, h, score_class_0, score_class_1, ... score_class_79]
//!         │          ── box ──  ─────────── 80 COCO class scores ──────────────
//!         └── batch size 1
//! ```
//!
//! So **84 = 4 box parameters + 80 COCO classes** (person, car, bus, …).
//!
//! Each of the 8400 columns is one "maybe detection" — most are garbage (background).
//! We filter them in two passes:
//!
//! 1. **Confidence threshold** — drop candidates whose best class score < 0.5 (config)
//! 2. **NMS (Non-Maximum Suppression)** — when multiple boxes overlap the same object,
//!    keep only the highest-confidence one
//!
//! ## Coordinate spaces (critical!)
//!
//! ```text
//! Model outputs (cx, cy, w, h) in LETTERBOXED 640×640 space
//!         │
//!         ▼  letterbox_xywh_to_original()  — undo pad + scale
//! Original image pixels (x1, y1, x2, y2)  — what we return in JSON
//! ```
//!
//! Box format from YOLO is **center + size** (xywh); we convert to **corner** format
//! (x1,y1 top-left, x2,y2 bottom-right) for [`BBox`].
//!
//! # C# analogy
//! Like parsing raw `float[]` scores from ML.NET, applying a confidence cutoff,
//! then running overlap filtering before mapping to `List<DetectionDto>`.

use crate::config::VisionConfig;
use crate::domain::{coco::coco_class_name, BBox, Detection, VisionError};
use crate::vision::preprocess::LetterboxMeta;

/// COCO dataset has 80 object classes (person=0, bicycle=1, …, toothbrush=79).
const COCO_CLASS_COUNT: usize = 80;

/// Main entry: parse raw ONNX output and return filtered detections.
///
/// # Arguments
/// * `output` — flat f32 slice from ONNX Runtime
/// * `shape` — tensor dimensions, usually `[1, 84, 8400]` or transposed `[1, 8400, 84]`
/// * `meta` — letterbox parameters from pre-processing (for coordinate inverse transform)
/// * `config` — confidence and IoU thresholds from TOML
///
/// # Steps
/// 1. Parse layout (channel-major vs prediction-major)
/// 2. For each of N candidates: find best class score
/// 3. Skip if below `confidence_threshold`
/// 4. Convert box from letterbox xywh → original image corners
/// 5. Run NMS to remove duplicate overlapping boxes
pub fn decode_yolov8_output(
    output: &[f32],
    shape: &[usize],
    meta: &LetterboxMeta,
    config: &VisionConfig,
) -> Result<Vec<Detection>, VisionError> {
    // ONNX exporters may emit [1, 84, N] or [1, N, 84] — handle both.
    let (features, num_preds) = match shape {
        [1, 84, n] => (84, *n),
        [1, n, 84] => {
            return decode_transposed(output, *n, meta, config);
        }
        other => {
            return Err(VisionError::OutputShape(format!(
                "expected [1, 84, N] or [1, N, 84], got {other:?}"
            )));
        }
    };

    if features != 84 {
        return Err(VisionError::OutputShape(format!(
            "expected 84 features per prediction, got {features}"
        )));
    }

    let mut candidates = Vec::new();

    // Layout [1, 84, N]: row-major by feature, then prediction index.
    // output[feature_row * num_preds + prediction_index]
    for i in 0..num_preds {
        let cx = output[0 * num_preds + i];
        let cy = output[1 * num_preds + i];
        let w = output[2 * num_preds + i];
        let h = output[3 * num_preds + i];

        // Among 80 class scores, pick the highest (multi-class classification per anchor).
        let mut best_class = 0u32;
        let mut best_score = 0.0_f32;

        for c in 0..COCO_CLASS_COUNT {
            let score = output[(4 + c) * num_preds + i];
            if score > best_score {
                best_score = score;
                best_class = c as u32;
            }
        }

        // Config default 0.5 — suppresses low-confidence noise from empty regions.
        if best_score < config.confidence_threshold {
            continue;
        }

        let bbox = letterbox_xywh_to_original(cx, cy, w, h, meta);
        candidates.push(Detection {
            class_id: best_class,
            class_name: coco_class_name(best_class).to_string(),
            confidence: best_score,
            bbox,
        });
    }

    Ok(non_max_suppression(candidates, config.iou_threshold))
}

/// Same logic as [`decode_yolov8_output`] but for shape `[1, N, 84]` (prediction-major layout).
///
/// Some ONNX exports store all 84 features contiguously per prediction:
/// `output[i * 84 + feature_index]` instead of `output[feature * N + i]`.
fn decode_transposed(
    output: &[f32],
    num_preds: usize,
    meta: &LetterboxMeta,
    config: &VisionConfig,
) -> Result<Vec<Detection>, VisionError> {
    let mut candidates = Vec::new();

    for i in 0..num_preds {
        let base = i * 84;
        let cx = output[base];
        let cy = output[base + 1];
        let w = output[base + 2];
        let h = output[base + 3];

        let mut best_class = 0u32;
        let mut best_score = 0.0_f32;

        for c in 0..COCO_CLASS_COUNT {
            let score = output[base + 4 + c];
            if score > best_score {
                best_score = score;
                best_class = c as u32;
            }
        }

        if best_score < config.confidence_threshold {
            continue;
        }

        let bbox = letterbox_xywh_to_original(cx, cy, w, h, meta);
        candidates.push(Detection {
            class_id: best_class,
            class_name: coco_class_name(best_class).to_string(),
            confidence: best_score,
            bbox,
        });
    }

    Ok(non_max_suppression(candidates, config.iou_threshold))
}

/// Inverse letterbox transform: model xywh → original image corner bbox.
///
/// # Input (from YOLO)
/// * `cx, cy` — box center in **640×640 letterboxed** coordinates
/// * `w, h` — box width and height in the same space
///
/// # Inverse math (undo what preprocess did)
/// ```text
/// 1. xywh → corners in letterbox space:
///    x1 = cx - w/2,  y1 = cy - h/2
///
/// 2. Remove padding (image was centered with pad_x, pad_y):
///    x1' = x1 - pad_x
///
/// 3. Undo uniform scale (resize was multiplied by scale):
///    x1_orig = x1' / scale
/// ```
///
/// Finally clamp to `[0, orig_width]` so boxes never extend outside the photo.
fn letterbox_xywh_to_original(cx: f32, cy: f32, w: f32, h: f32, meta: &LetterboxMeta) -> BBox {
    let x1 = (cx - w / 2.0 - meta.pad_x) / meta.scale;
    let y1 = (cy - h / 2.0 - meta.pad_y) / meta.scale;
    let x2 = (cx + w / 2.0 - meta.pad_x) / meta.scale;
    let y2 = (cy + h / 2.0 - meta.pad_y) / meta.scale;

    BBox {
        x1: x1.clamp(0.0, meta.orig_width as f32),
        y1: y1.clamp(0.0, meta.orig_height as f32),
        x2: x2.clamp(0.0, meta.orig_width as f32),
        y2: y2.clamp(0.0, meta.orig_height as f32),
    }
}

/// **Non-Maximum Suppression (NMS)** — remove duplicate boxes on the same object.
///
/// YOLO emits many overlapping candidates per object (different anchor points).
/// NMS keeps the best one:
///
/// ```text
/// 1. Sort all detections by confidence (highest first)
/// 2. Take the best box, add to output
/// 3. Remove any remaining box with IoU > threshold (default 0.45) vs the best
/// 4. Repeat until no candidates left
/// ```
///
/// IoU = Intersection over Union — 1.0 = identical boxes, 0.0 = no overlap.
///
/// # C# analogy
/// Like deduplicating search results: "these two hits are the same thing, keep the stronger score."
fn non_max_suppression(mut detections: Vec<Detection>, iou_threshold: f32) -> Vec<Detection> {
    detections.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut kept = Vec::new();

    while let Some(best) = detections.first().cloned() {
        detections.remove(0);
        kept.push(best.clone());
        // Drop boxes that overlap too much with the one we just kept (same object).
        detections.retain(|d| iou(&best.bbox, &d.bbox) < iou_threshold);
    }

    kept
}

/// **Intersection over Union** between two axis-aligned rectangles.
///
/// ```text
///        ┌─── A ───┐
///        │  ┌─B──┐ │
///        │  │ ∩  │ │   IoU = area(∩) / area(A ∪ B)
///        └──┴────┴─┘
/// ```
///
/// Used by NMS to measure "how much do these two boxes refer to the same object?"
fn iou(a: &BBox, b: &BBox) -> f32 {
    // Intersection rectangle: max of left/top edges, min of right/bottom edges.
    let x1 = a.x1.max(b.x1);
    let y1 = a.y1.max(b.y1);
    let x2 = a.x2.min(b.x2);
    let y2 = a.y2.min(b.y2);

    let inter_w = (x2 - x1).max(0.0);
    let inter_h = (y2 - y1).max(0.0);
    let inter = inter_w * inter_h;

    let area_a = (a.x2 - a.x1).max(0.0) * (a.y2 - a.y1).max(0.0);
    let area_b = (b.x2 - b.x1).max(0.0) * (b.y2 - b.y1).max(0.0);
    let union = area_a + area_b - inter;

    if union <= 0.0 {
        return 0.0;
    }
    inter / union
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nms_removes_overlap() {
        let a = Detection {
            class_id: 0,
            class_name: "person".into(),
            confidence: 0.9,
            bbox: BBox {
                x1: 0.0,
                y1: 0.0,
                x2: 10.0,
                y2: 10.0,
            },
        };
        let b = Detection {
            class_id: 0,
            class_name: "person".into(),
            confidence: 0.8,
            bbox: BBox {
                x1: 1.0,
                y1: 1.0,
                x2: 9.0,
                y2: 9.0,
            },
        };
        let out = non_max_suppression(vec![a, b], 0.5);
        assert_eq!(out.len(), 1);
    }
}
