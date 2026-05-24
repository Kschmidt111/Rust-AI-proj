# SeekerSim crate (Phase 2)

Library + binary layout; HTTP routes under `src/api/`, YOLO detection under `src/vision/`, shared types in `src/domain/`.

## Layout

```
src/
├── main.rs           # thin entry → cli.rs
├── cli.rs            # serve | detect subcommands
├── lib.rs            # run(), shared modules
├── config.rs         # server + vision config
├── domain/           # Detection, BBox, COCO labels, VisionError
├── vision/           # decode, preprocess, detector, postprocess
├── telemetry/        # logging init (metrics later)
└── api/
    └── routes/
        └── health.rs
```

## Setup (one-time)

From repo root:

```powershell
.\scripts\download-model.ps1      # models/yolov8n.onnx (~12 MB)
.\scripts\download-sample-image.ps1 # data/samples/test.jpg
```

## Run — HTTP server (Phase 1)

```powershell
cd crates/seeker-sim
cargo run
# or explicitly:
cargo run -- serve
```

Config: `config/default.toml` at repo root (`[server].bind`, `[vision].model_path`).

```powershell
curl.exe http://127.0.0.1:8080/health
```

## Run — single-image detection (Phase 2)

```powershell
cd crates/seeker-sim
cargo run -- detect --input ../../data/samples/test.jpg
```

Expect JSON with COCO class names and bounding boxes (e.g. person, bus on the sample image).

## Tests

```powershell
cargo test
```

Debug logs:

```powershell
$env:RUST_LOG = "seeker_sim=debug"
cargo run -- detect --input ../../data/samples/test.jpg
```
