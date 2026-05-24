//! Match measurements to an existing track prediction (Phase 4C).
//!
//! Point centroids use a **distance gate** (small moving targets).
//! Bounding boxes use **IoU** (YOLO detections).

use crate::domain::{BBox, Detection};

/// Result of associating one predicted track position to point candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointAssociation {
    /// Index into the candidate slice that matched.
    Matched(usize),
    /// No candidate fell within the distance gate.
    NoMatch,
}

/// Intersection-over-union of two axis-aligned boxes in pixel space.
///
/// # Arguments
/// * `a` — reference box (e.g. prior track bbox or predicted region).
/// * `b` — candidate box (e.g. new detection).
///
/// # Returns
/// IoU in `[0.0, 1.0]`; `0.0` when boxes do not overlap.
///
/// # C# analogy
/// A static geometry helper — like `BBoxExtensions.IoU(a, b)` on a utility class.
pub fn iou(a: &BBox, b: &BBox) -> f32 {
    let inter_x1 = a.x1.max(b.x1);
    let inter_y1 = a.y1.max(b.y1);
    let inter_x2 = a.x2.min(b.x2);
    let inter_y2 = a.y2.min(b.y2);

    let inter_w = (inter_x2 - inter_x1).max(0.0);
    let inter_h = (inter_y2 - inter_y1).max(0.0);
    let inter_area = inter_w * inter_h;

    if inter_area <= 0.0 {
        return 0.0;
    }

    let area_a = (a.x2 - a.x1).max(0.0) * (a.y2 - a.y1).max(0.0);
    let area_b = (b.x2 - b.x1).max(0.0) * (b.y2 - b.y1).max(0.0);
    let union = area_a + area_b - inter_area;

    if union <= 0.0 {
        return 0.0;
    }

    inter_area / union
}

/// Euclidean distance between two image points (pixels).
pub fn point_distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

/// Associates a predicted track center to the nearest point candidate within a gate.
///
/// Used for motion centroids and small dot targets where IoU is unreliable.
///
/// # Arguments
/// * `predicted` — Kalman-predicted `(x, y)` before the update step.
/// * `candidates` — centroids from motion blobbing or point detections this frame.
/// * `max_distance_px` — reject matches farther than this (pixels).
///
/// # Returns
/// [`PointAssociation::Matched`] with the **closest** in-gate index, or [`PointAssociation::NoMatch`].
///
/// # C# analogy
/// ```csharp
/// var hit = candidates
///     .Select((c, i) => (i, Dist(predicted, c)))
///     .Where(x => x.Dist <= maxDistance)
///     .OrderBy(x => x.Dist)
///     .FirstOrDefault();
/// ```
pub fn associate_nearest_point(
    predicted: (f32, f32),
    candidates: &[(f32, f32)],
    max_distance_px: f32,
) -> PointAssociation {
    let mut best: Option<(usize, f32)> = None;

    for (index, &candidate) in candidates.iter().enumerate() {
        let dist = point_distance(predicted, candidate);
        if dist > max_distance_px {
            continue;
        }
        if best.is_none_or(|(_, best_dist)| dist < best_dist) {
            best = Some((index, dist));
        }
    }

    match best {
        Some((index, _)) => PointAssociation::Matched(index),
        None => PointAssociation::NoMatch,
    }
}

/// Associates a reference bbox to the best detection by IoU (YOLO path).
///
/// # Arguments
/// * `reference` — prior track bbox or search window.
/// * `detections` — candidates from the detector this frame.
/// * `iou_threshold` — minimum IoU to accept a match (config `[tracking].iou_match_threshold`).
///
/// # Returns
/// Index of the best detection at or above threshold, or `None`.
pub fn associate_best_iou(
    reference: &BBox,
    detections: &[Detection],
    iou_threshold: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;

    for (index, detection) in detections.iter().enumerate() {
        let score = iou(reference, &detection.bbox);
        if score < iou_threshold {
            continue;
        }
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((index, score));
        }
    }

    best.map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(x1: f32, y1: f32, x2: f32, y2: f32) -> BBox {
        BBox { x1, y1, x2, y2 }
    }

    fn detection(bbox: BBox) -> Detection {
        Detection {
            class_id: 0,
            class_name: "object".into(),
            confidence: 0.9,
            bbox,
        }
    }

    #[test]
    fn iou_identical_boxes_is_one() {
        let b = bbox(10.0, 10.0, 30.0, 30.0);
        assert!((iou(&b, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn iou_non_overlapping_is_zero() {
        let a = bbox(0.0, 0.0, 10.0, 10.0);
        let b = bbox(20.0, 20.0, 30.0, 30.0);
        assert_eq!(iou(&a, &b), 0.0);
    }

    #[test]
    fn associate_nearest_point_picks_closest_in_gate() {
        let predicted = (100.0, 100.0);
        let candidates = [(105.0, 105.0), (101.0, 100.0), (200.0, 200.0)];

        let result = associate_nearest_point(predicted, &candidates, 10.0);

        assert_eq!(result, PointAssociation::Matched(1));
    }

    #[test]
    fn associate_nearest_point_rejects_outside_gate() {
        let predicted = (0.0, 0.0);
        let candidates = [(50.0, 0.0), (0.0, 50.0)];

        let result = associate_nearest_point(predicted, &candidates, 20.0);

        assert_eq!(result, PointAssociation::NoMatch);
    }

    #[test]
    fn associate_best_iou_picks_highest_overlap() {
        let reference = bbox(0.0, 0.0, 20.0, 20.0);
        let detections = vec![
            detection(bbox(18.0, 18.0, 38.0, 38.0)),
            detection(bbox(5.0, 5.0, 25.0, 25.0)),
        ];

        let index = associate_best_iou(&reference, &detections, 0.1).expect("match");

        assert_eq!(index, 1);
    }

    #[test]
    fn associate_best_iou_returns_none_below_threshold() {
        let reference = bbox(0.0, 0.0, 10.0, 10.0);
        let detections = vec![detection(bbox(100.0, 100.0, 110.0, 110.0))];

        assert!(associate_best_iou(&reference, &detections, 0.3).is_none());
    }
}
