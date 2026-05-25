# seeker-sim crate

Main Rust application: HTTP API, ONNX vision, and frame-sequence pipeline.

**Phases implemented:** 1 (server) · 2 (detect) · 3 (process) · 4 (track) · 5 (intercept) · 6 (web UI + `run`)

---

## Demo flow (browser first)

```powershell
# From repo root
.\scripts\generate-dot-video.ps1
cd crates\seeker-sim
$env:RUST_LOG = "seeker_sim=info"

# CLI batch → artifacts + latency line
cargo run -- run --input ../../data/frames/dot_run_001

# Browser replay + pure sim
cargo run
# http://127.0.0.1:8080/  → Run replay (paste run_id) or Sim viewer → Go
```

## Source layout

```
src/
├── main.rs           # entry → cli::run()
├── cli.rs            # serve | run | detect | process | track | intercept
├── lib.rs            # pub mod …; HTTP run()
├── config.rs         # TOML → AppConfig
├── domain/           # Detection, BBox, VisionError, COCO labels
├── ingest/           # FrameSource — sorted PNG/JPEG paths
├── pipeline/         # process, track, intercept, sim_run, run_api
├── sim/              # 2D kinematic engine
├── guidance/         # PN, pure pursuit
├── vision/           # decode, YOLO, motion (frame diff)
├── tracking/         # Kalman, associator, ROI, LOS, PointTracker
├── telemetry/        # CSV writer, plot, latency summary
└── api/              # routes, static UI
```

---

## One-time setup

From **repo root**:

```powershell
.\scripts\download-model.ps1        # → models/yolov8n.onnx (~12 MB)
.\scripts\download-sample-image.ps1 # → data/samples/test.jpg
```

Config file: `../../config/default.toml` (relative to this crate).

---

## Commands

Run from `crates/seeker-sim`:

### Phase 1 — HTTP server

```powershell
cargo run
# or: cargo run -- serve
curl.exe http://127.0.0.1:8080/health
```

Expected: `{"status":"ok","service":"seeker-sim"}`

### Phase 2 — single image

```powershell
cargo run -- detect --input ../../data/samples/test.jpg
```

Prints pretty JSON: class names, confidence, bounding boxes in **original image pixels**.

### Phase 3 — frame folder

Generate synthetic frames (repo root):

```powershell
.\scripts\generate-dot-video.ps1
```

Process folder (YOLO session loaded **once**; logs each frame):

```powershell
$env:RUST_LOG = "seeker_sim=info"
cargo run -- process --input ../../data/frames/dot_run_001
```

Or from MP4 via ffmpeg (repo root):

```powershell
.\scripts\extract-frames.ps1 -InputVideo data\videos\sample.mp4 -OutputDir data\frames\run_001
cargo run -- process --input ../../data/frames/run_001
```

**Note:** Synthetic dot frames often show **0 YOLO detections** — use `track` (Phase 4) for small movers.

### Phase 4 — motion track + CSV

```powershell
.\scripts\generate-dot-video.ps1
cargo run -- track --input ../../data/frames/dot_run_001
```

Writes `data/output/run_<id>/tracks.csv` with stable `track_id`, velocity, and LOS columns.

### Phase 5–6 — intercept + web UI

```powershell
cargo run -- intercept --input ../../data/frames/dot_run_001
# or portfolio demo with latency:
cargo run -- run --input ../../data/frames/dot_run_001
```

Artifacts: `tracks.csv`, `guidance.csv`, `sim.csv`, `trajectory.png`.

```powershell
cargo run
# Sim viewer + run replay at http://127.0.0.1:8080/
```

---

## Call chain (for learning)

| Command | Driver | Vision path |
|---------|--------|-------------|
| `detect` | `cli` → `vision::detect_on_image` | decode → YOLO → postprocess |
| `process` | `cli` → `pipeline::process_frame_folder` | same per frame, model loaded once |
| `track` | `cli` → `pipeline::track_motion_folder` | motion/ROI → Kalman → tracks.csv |
| `intercept` / `run` | `cli` → `pipeline::intercept_motion_folder` | track + PN/PP + sim CSVs |
| `serve` | `cli` → `lib::run` → Axum + static UI | JSON APIs + browser |

---

## Tests

```powershell
cargo test
```

Includes tracking Kalman/associator/LOS tests, motion ROI, pipeline integration, health route.

Debug vision logs:

```powershell
$env:RUST_LOG = "seeker_sim=debug"
cargo run -- detect --input ../../data/samples/test.jpg
```

Roadmap: [docs/LEARNING_ROADMAP.md](../../docs/LEARNING_ROADMAP.md) — **Next:** Phase 7 Qdrant ingestion.
