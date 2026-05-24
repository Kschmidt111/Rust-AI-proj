# Learning Roadmap — SeekerSim

Phased plan for a **C# developer (~4 years)** learning **Rust** while building visual tracking + guidance simulation. Each phase adds working software; every `pub fn` stays fully commented.

**Architecture reference:** [ARCHITECTURE.md](ARCHITECTURE.md)  
**Tools reference:** [TOOLS.md](TOOLS.md)  
**AI engineer positioning (founder north star):** [PROJECT_BRIEF.md § Design north star](PROJECT_BRIEF.md#design-north-star--ai-engineer-positioning-2026)

---

## Phase overview

| Phase | Goal | Rust concepts | New modules |
|-------|------|---------------|-------------|
| **0** | Docs complete | — | — |
| **1** | HTTP `/health` + config | cargo, modules, `Result`, `tokio`, Axum | `main`, `config`, `api` |
| **2** | Detect on one image | `Vec`, traits, `ort`, errors | `vision/*`, `domain` |
| **3** | Process frame folder | iterators, pipeline loop | `ingest/*`, `pipeline` |
| **4** | Track + Kalman + CSV | structs, ownership, unit tests | `tracking/*`, `telemetry` |
| **5** | PN guidance + 2D sim + plot | f32 math, modules | `guidance/*`, `sim/*` |
| **6** | CLI + metrics + demo video | `clap`, docs | `cli.rs` |
| **7** | Qdrant ingestion pipeline | embeddings, `reqwest`, Qdrant client | `memory/` or `clients/qdrant` |
| **8** | Ollama RAG + latency story | RAG prompts, local LLM | `rag/`, `clients/ollama` |

---

## Phase 0 — Foundation (current)

**Deliverables**

- [x] PROJECT_BRIEF, ARCHITECTURE, TOOLS, this roadmap
- [ ] Push to GitHub when ready ([GITHUB_SETUP.md](GITHUB_SETUP.md))

**Reading**

- Rust Book Ch. 1–6 (ownership, structs, enums, `Result`)

---

## Phase 1 — Skeleton service

**Goal:** `cargo run` → server on `127.0.0.1:8080` → `GET /health` → `{"status":"ok"}`.

**Steps**

1. `cargo new seeker-sim` under `crates/` (or workspace root—match ARCHITECTURE).
2. Dependencies: `tokio`, `axum`, `serde`, `serde_json`, `tracing`, `tracing-subscriber`, `toml`.
3. Load `config/default.toml`.
4. Comment every function.

**C# analogy:** Minimal ASP.NET Core host + `MapGet("/health")`.

**Done when:** `curl http://127.0.0.1:8080/health` succeeds.

---

## Phase 2 — Single-frame detection ✅

**Goal:** CLI or test loads `data/samples/test.jpg` → prints detections JSON.

**Steps**

1. [x] Add `ort`, `image`.
2. [x] Download `yolov8n.onnx` to `models/`.
3. [x] Implement `vision/preprocess`, `detector`, `postprocess`.
4. [x] Define `Detection`, `BBox` in `domain/types.rs`.

**Rust concepts:** `Arc` for ONNX session, `?` propagation, `thiserror`.

**Done when:** Boxes print with class + confidence for a sample image.

**Verify:**

```powershell
cd crates/seeker-sim
cargo run -- detect --input ../../data/samples/test.jpg
```

---

## Phase 3 — Frame sequence pipeline ✅

**Goal:** Process `data/frames/run_001/*.png` in order; log detection count per frame.

**Steps**

1. [x] `ingest/frame_source.rs` — iterate sorted PNG paths.
2. [x] `pipeline/mod.rs` — `for frame in source { ... }`.
3. [x] `scripts/extract-frames.ps1` — ffmpeg from MP4.
4. [x] `scripts/generate-dot-video.ps1` — synthetic **small moving target** on uniform background (intercept demo input).

**Rust concepts:** `Iterator`, `PathBuf`, enum `FrameSource { Folder, ... }`.

**Done when:** 100-frame folder processes without panic; `tracing` shows per-frame ms.

**Verify:**

```powershell
.\scripts\generate-dot-video.ps1
cd crates/seeker-sim
$env:RUST_LOG = "seeker_sim=info"
cargo run -- process --input ../../data/frames/dot_run_001
```

---

## Phase 4 — Tracking & telemetry (small-target focus) ✅

**Goal:** One dominant track on a **small moving target** with Kalman-smoothed velocity; write `tracks.csv`.

**Steps**

1. [x] `vision/motion.rs` — frame differencing → centroid candidate (acquisition / re-acquire).
2. [x] `tracking/roi_tracker.rs` — search predicted ROI around Kalman state (track without full-frame YOLO every frame).
3. [x] `associator.rs` — match measurement to track (IoU or distance gate for point targets).
4. [x] `kalman.rs` — 4-state CV filter on center x,y.
5. [x] `los.rs` — bearing + finite-difference LOS rate.
6. [x] `telemetry/writer.rs` — CSV rows.

**Rust concepts:** Unit tests in `#[cfg(test)]`, borrowing across frames, optional `#[serde(default)]` for config enums.

**Done when:** CSV shows stable `track_id` and velocity on **synthetic dot video** (target ~5–15 px). YOLO-only path remains for large COCO objects (Phase 2).

---

## Phase 5 — Guidance & simulation (intercept)

**Goal:** `guidance.csv` + `sim.csv` + `trajectory.png`; **simulated interceptor intercepts tracked target**.

**Steps**

1. `pure_pursuit.rs` — baseline (optional compare).
2. `pn.rs` — proportional navigation from LOS rate.
3. `sim/engine.rs` — integrate interceptor toward target; map image bearing → sim plane.
4. `plotters` — 2D path and LOS rate chart.

**Rust concepts:** Pure functions for laws; no hidden global state.

**Done when:** On synthetic dot video, **miss distance < threshold** (e.g. 10 m in sim units) with PN; improvement vs pure pursuit documented in README.

---

## Phase 6 — Portfolio polish

**Goal:** One-command demo: **video in → track + intercept plot out**.

**Steps**

1. `cli.rs`: `seeker-sim run --input data/videos/demo.mp4` (or frame folder).
2. Document p50/p95 ms per stage from `tracing` spans.
3. 90s screen recording: input video (small target) + output plot showing intercept.

**Done when:** README “Demo” section is reproducible on a fresh clone (minus model download); **synthetic dot intercept** is the guaranteed path; real-world clips documented as optional tuning.

---

## Phase 7 — Vector DB ingestion (AI engineer track)

**Goal:** After each run, detection/run summaries are **embedded and upserted** to **Qdrant**; semantic search returns past events.

**Why (2026 portfolio):** Custom **continuous ingestion** into a vector DB—not a one-off script. See [PROJECT_BRIEF.md](PROJECT_BRIEF.md#design-north-star--ai-engineer-positioning-2026).

**Steps**

1. Docker Compose: Qdrant local.
2. `clients/qdrant.rs` — create collection, upsert points with payload (run_id, frame, class, bbox text summary).
3. Embed summaries via Ollama embedding model or small ONNX embedder (ADR when implementing).
4. Hook pipeline end: each run → batch upsert (continuous path for later: per-frame upsert).

**Done when:** Query “white ball near center” returns hits from ingested runs.

---

## Phase 8 — Local LLM RAG + latency (AI engineer track)

**Goal:** `POST /v1/query` — question → Qdrant retrieve → **Ollama (Llama 3 class)** answer with **citations**; README documents **p50/p95** for detect and RAG paths.

**Why (2026 portfolio):** **Host open models locally** (Ollama), orchestrate full flow in Rust, optimize for **low latency**—not an OpenAI wrapper.

**Steps**

1. `clients/ollama.rs` — generate + optional embed API.
2. `rag/` — prompt template with grounded CONTEXT block; return `sources[]`.
3. Aggregate `tracing` spans → p50/p95 table in README.
4. (Stretch) Document path to vLLM swap behind same trait.

**Done when:** Natural-language question about an ingested run is answerable with citations; latency numbers in README.

**Compliance note:** Use Llama only for **generic log/run Q&A** framing; review [Meta Llama Acceptable Use Policy](https://www.llama.com/llama3/use-policy/) — see future `docs/COMPLIANCE.md` if added.

---

## Comment template (required)

```rust
/// One-sentence summary.
///
/// # Arguments
/// * `name` — meaning
///
/// # Returns
/// What the caller gets.
///
/// # C# analogy
/// Optional comparison.
pub fn example() -> Result<()> {
```

---

## C# ↔ Rust quick reference (this project)

| C# | Rust in SeekerSim |
|----|-------------------|
| `Program.cs` | `main.rs` |
| `IOptions<T>` | `AppConfig` from TOML |
| `ILogger` | `tracing::info!` |
| `Task<T>` | `async fn` + `.await` |
| `List<T>` | `Vec<T>` |
| `null` | `Option<T>` |
| Exception | `Result<T, SeekerError>` |

---

## Weekly pace (side project)

| Week | Phase |
|------|-------|
| 1 | 1 |
| 2 | 2 |
| 3 | 3 |
| 4 | 4 |
| 5–6 | 5–6 |
| 7–8 | 7–8 (AI engineer portfolio — after 2–6 solid) |

Adjust as needed; finish Phase 4 before Phase 5 (guidance needs velocity).
