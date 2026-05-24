# Scripts

PowerShell helpers for setup, data prep, and security. Run from **repo root** unless noted.

| Script | Phase | Purpose |
|--------|-------|---------|
| `pre-push-check.ps1` | — | Scan staged files for secrets before `git push` ([rules.md](../rules.md)) |
| `download-model.ps1` | 2 | Download `yolov8n.onnx` → `models/` |
| `download-sample-image.ps1` | 2 | Download Ultralytics bus sample → `data/samples/test.jpg` |
| `extract-frames.ps1` | 3 | ffmpeg: MP4 → `data/frames/.../%04d.png` |
| `generate-dot-video.ps1` | 3 | Synthetic small white dot on blue sky → `data/frames/dot_run_001/` |

Tool rationale: [docs/TOOLS.md](../docs/TOOLS.md).

---

## Usage examples

### Download ONNX model (required once)

```powershell
.\scripts\download-model.ps1
```

### Download test image

```powershell
.\scripts\download-sample-image.ps1
```

### Extract frames from video (requires ffmpeg on PATH)

```powershell
.\scripts\extract-frames.ps1 `
  -InputVideo "data\videos\sample.mp4" `
  -OutputDir "data\frames\run_001"
```

### Generate synthetic dot sequence (no ffmpeg)

```powershell
.\scripts\generate-dot-video.ps1
# optional: -FrameCount 100 -OutputDir "data\frames\dot_run_001"
```

Then process:

```powershell
cd crates\seeker-sim
cargo run -- process --input ../../data/frames/dot_run_001
```

### Pre-push security check

```powershell
.\scripts\pre-push-check.ps1
```
