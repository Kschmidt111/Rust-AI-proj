# Data directory (gitignored contents)

Runtime data lives here. **Do not commit** large videos, frame sequences, or model outputs.

## Layout

| Path | Purpose | How to create |
|------|---------|----------------|
| `samples/` | Single test images | `.\scripts\download-sample-image.ps1` → `test.jpg` |
| `videos/` | Input MP4 (optional) | Copy your own file |
| `frames/` | PNG sequences for pipeline | `generate-dot-video.ps1` or `extract-frames.ps1` |
| `frames/dot_run_001/` | 100 synthetic dot frames (Phase 3 demo) | `.\scripts\generate-dot-video.ps1` |
| `frames/run_001/` | Frames from ffmpeg | `.\scripts\extract-frames.ps1` |
| `output/{run_id}/` | Per-run CSV, plots (Phase 4+) | Created by pipeline |

## Example commands

```powershell
# Sample still image (Phase 2 detect)
.\scripts\download-sample-image.ps1
cd crates\seeker-sim
cargo run -- detect --input ../../data/samples/test.jpg

# Synthetic moving dot sequence (Phase 3 process)
.\scripts\generate-dot-video.ps1
cargo run -- process --input ../../data/frames/dot_run_001
```

## Git

Only this `README.md` is tracked. Everything else under `data/` is ignored via root `.gitignore`.

Architecture: [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md).
