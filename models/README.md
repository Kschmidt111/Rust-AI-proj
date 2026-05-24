# Models (gitignored weights)

ONNX files are **not committed**. Download after clone.

## Required for Phases 2–3

| File | Size | Purpose |
|------|------|---------|
| `yolov8n.onnx` | ~12 MB | YOLOv8n object detection via ONNX Runtime |

## Download

From repo root:

```powershell
.\scripts\download-model.ps1
```

Source: [Ultralytics assets v8.4.0](https://github.com/ultralytics/assets/releases/download/v8.4.0/yolov8n.onnx)

## License

Ultralytics YOLO is **AGPL-3.0**. Public OSS portfolio use is fine; note in interviews/docs. See [docs/PROJECT_BRIEF.md](../docs/PROJECT_BRIEF.md).

## Config

Path is set in `config/default.toml`:

```toml
[vision]
model_path = "models/yolov8n.onnx"
```

## Future upgrades

For better small-object recall (optional): `yolov8s.onnx` + higher `input_size` in config. Small-target tracking in Phase 4 also uses **motion detection**, not YOLO alone ([ADR-017](../docs/DECISIONS.md)).
