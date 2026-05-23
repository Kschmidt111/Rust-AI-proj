//! Core types produced by the vision pipeline.

use serde::Serialize;

/// Axis-aligned bounding box in **original image pixel coordinates**.
///
/// # C# analogy
/// A small record/DTO like `record BBox(float X1, float Y1, float X2, float Y2)`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl BBox {
    /// Center point `(cx, cy)` — useful for tracking and guidance later.
    pub fn center(&self) -> (f32, f32) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }
}

/// One object detection from a single frame.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Detection {
    pub class_id: u32,
    pub class_name: String,
    pub confidence: f32,
    pub bbox: BBox,
}
