//! Decode YOLO ONNX output tensors into [`Detection`] values.

use crate::config::VisionConfig;
use crate::domain::{coco::coco_class_name, BBox, Detection, VisionError};
use crate::vision::preprocess::LetterboxMeta;

const COCO_CLASS_COUNT: usize = 80;

/// Parses raw ONNX output and applies confidence filter + NMS.
///
/// Expects Ultralytics YOLOv8 export shape `[1, 84, N]` where each column is:
/// `[cx, cy, w, h, class_scores...]`.
pub fn decode_yolov8_output(
    output: &[f32],
    shape: &[usize],
    meta: &LetterboxMeta,
    config: &VisionConfig,
) -> Result<Vec<Detection>, VisionError> {
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

    for i in 0..num_preds {
        let cx = output[0 * num_preds + i];
        let cy = output[1 * num_preds + i];
        let w = output[2 * num_preds + i];
        let h = output[3 * num_preds + i];

        let mut best_class = 0u32;
        let mut best_score = 0.0_f32;

        for c in 0..COCO_CLASS_COUNT {
            let score = output[(4 + c) * num_preds + i];
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

/// Maps center+size in letterboxed 640-space back to original image pixels.
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

/// Greedy non-maximum suppression on overlapping boxes.
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
        detections.retain(|d| iou(&best.bbox, &d.bbox) < iou_threshold);
    }

    kept
}

fn iou(a: &BBox, b: &BBox) -> f32 {
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
