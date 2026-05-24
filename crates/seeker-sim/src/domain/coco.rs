//! COCO dataset class names (80 classes) for YOLOv8 pretrained exports.

/// Returns the COCO label for `class_id`, or `"unknown"` if out of range.
///
/// # C# analogy
/// Like a static `ReadOnlyDictionary<int, string>` of label mappings.
pub fn coco_class_name(class_id: u32) -> &'static str {
    COCO_CLASSES
        .get(class_id as usize)
        .copied()
        .unwrap_or("unknown")
}

/// YOLOv8n pretrained on COCO uses these 80 class names (index = class id).
const COCO_CLASSES: [&str; 80] = [
    "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck", "boat",
    "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat",
    "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack",
    "umbrella", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball",
    "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket",
    "bottle", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple",
    "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair",
    "couch", "potted plant", "bed", "dining table", "toilet", "tv", "laptop", "mouse",
    "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
    "refrigerator", "book", "clock", "vase", "scissors", "teddy bear", "hair drier",
    "toothbrush",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_class_names() {
        assert_eq!(coco_class_name(0), "person");
        assert_eq!(coco_class_name(5), "bus");
    }
}
