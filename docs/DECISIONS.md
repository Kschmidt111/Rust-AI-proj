# Architecture Decision Records (ADRs)

Decisions for **SeekerSim**. Superseded Sentinel ISR ADRs are listed at the bottom.

---

## ADR-001: Project scope is track + guidance simulation

**Status:** Accepted  
**Date:** 2026-05-23

**Context:** Need a focused Rust + CV portfolio piece that demonstrates closed-loop “imagery → steering” without over-scoping.

**Decision:** Build **SeekerSim**: detect → track → Kalman → proportional navigation → 2D sim, then **Qdrant ingestion + Ollama RAG** (Phases 7–8) for the full local-ML platform story. Not a thin OpenAI wrapper or RAG-only demo.

**Consequences:** Qdrant/Ollama required for **full AI engineer portfolio** (Phases 7–8); CSV MVP sufficient through Phase 6.

---

## ADR-002: Rust for all orchestration and hot path

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** Rust owns ingest, inference scheduling, tracking, guidance, sim, API. Python allowed only for one-off ONNX export scripts in `scripts/`, not in runtime.

**Consequences:** Steeper learning curve; stronger systems interview story.

---

## ADR-003: YOLOv8n via ONNX Runtime (`ort`)

**Status:** Accepted (Phase 2 bootstrap; see ADR-017 for small-target path)  
**Date:** 2026-05-23

**Decision:** Use **YOLOv8n** exported to ONNX, inference via **`ort`**, to wire up the first detection pipeline in Rust.

**Alternatives:** PyTorch live, TensorFlow Lite, custom tiny CNN.

**Consequences:** Export step documented; GPU via CUDA execution provider when available. **Not sufficient alone** for small, fast movers at range—ADR-017 adds motion + ROI tracking and model upgrades for the intercept demo.

---

## ADR-004: Video via frame folders + ffmpeg first

**Status:** Accepted  
**Date:** 2026-05-23

**Context:** `opencv` Rust bindings are painful on Windows for beginners.

**Decision:** Phase 3 uses **ffmpeg-extracted PNG folders**; optional `opencv` `VideoCapture` in Phase 6.

**Consequences:** Extra disk for frames; simpler, reliable builds.

---

## ADR-005: Kalman + IoU association for tracking v1

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** **IoU associator** + **constant-velocity Kalman** on bbox center—not ByteTrack/DeepSORT in v1.

**Alternatives:** Full multi-object tracker, optical flow.

**Consequences:** Good enough for single-target demos; may upgrade in ADR-011 later.

---

## ADR-006: Proportional navigation as primary guidance law

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** Implement **PN** as primary; **pure pursuit** as teaching baseline.

**Consequences:** Must implement LOS rate from vision (`tracking/los.rs`) with tested convention.

---

## ADR-007: Simulation-only, no hardware

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** 2D kinematic sim in Rust; no claim of flight-test or weapon integration.

**Consequences:** Safe public GitHub; careful resume wording.

---

## ADR-008: Commented code for learning

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** All `pub fn` have doc comments; optional C# analogies.

**Consequences:** Enforced in [DEVELOPMENT.md](DEVELOPMENT.md) and `.cursor/rules/seeker-sim-development.mdc`.

---

## ADR-009: Telemetry as CSV/JSONL, not a database (MVP)

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** Per-run files under `data/output/{run_id}/`; no Qdrant in MVP.

**Consequences:** Easy inspection; add DB later if needed.

---

## ADR-010: Axum HTTP + shared pipeline for CLI

**Status:** Accepted  
**Date:** 2026-05-23

**Decision:** One `pipeline::run()` used by CLI and HTTP routes.

**Consequences:** No duplicated business logic.

---

## ADR-011: Dual portfolio — CV path + AI engineer platform path

**Status:** Accepted  
**Date:** 2026-05-23

**Context:** Founder guidance for 2026: local open models, vector DB ingestion, low-latency orchestration—not OpenAI wrappers. User also targets defense/perception and AI engineer roles.

**Decision:** One repo, two phased tracks in [PROJECT_BRIEF.md](PROJECT_BRIEF.md):
- **Phases 2–6:** ONNX vision, tracking, guidance, latency metrics (perception story).
- **Phases 7–8:** Qdrant continuous ingestion + Ollama RAG (AI engineer story).

**Consequences:** Phase 7–8 are portfolio requirements for the full AI narrative, not optional side quests. Vision path must work first.

---

## ADR-017: Hybrid perception for small moving targets

**Status:** Accepted  
**Date:** 2026-05-23

**Context:** Primary user goal: **track a moving object in video and intercept it in simulation**, including when the target is **small in frame** (few pixels to ~20 px). YOLOv8n on COCO at 640×640 is a poor fit for that alone—it is fast to integrate but weak on tiny objects and not trained for “missile against sky.”

**Decision:** Use a **hybrid perception stack**, not YOLO-only:

| Stage | Method | When |
|-------|--------|------|
| **Acquire** | Motion blob (frame differencing) *or* YOLO detection | First frames / re-acquisition after coast |
| **Track** | Kalman prediction + **ROI search** (centroid of motion in predicted window) | Every frame after lock |
| **Refine (optional)** | YOLO on cropped ROI only, or upgrade to **YOLOv8s** at **1280** input | Hard real-world clips |
| **Measure** | Bbox center or **point centroid** → LOS / LOS rate | Tracking + guidance |

**Demo guarantee:** Include a **synthetic small-target video** (bright dot on uniform background, moving) so the intercept story always works on a fresh clone. Real-world drone/ball clips are stretch goals with upgraded model + tuning.

**Alternatives rejected for v1:**
- YOLO-only every frame — too slow and misses small targets.
- ByteTrack/DeepSORT first — adds complexity before single-target Kalman works.
- OpenCV runtime in hot path on Windows — deferred; motion ROI implemented in Rust on `image` buffers first.

**Consequences:**
- Phase 2 YOLO code stays; it becomes one **acquisition** backend, not the whole seeker.
- New modules: `vision/motion.rs`, `tracking/roi_tracker.rs` (names TBD in ARCHITECTURE).
- Phase 4 “done when” requires stable track on **small synthetic target**, not only large COCO objects.
- Phase 5 “done when” requires **simulated intercept** (miss distance below configured threshold).
- Config gains `perception_mode` (`yolo` | `motion` | `hybrid`) when implemented—default `hybrid` for video runs.

---

## Open decisions

| ID | Question | Decide by |
|----|----------|-----------|
| ADR-012 | Upgrade to ByteTrack? | After Phase 4 single-target Kalman works |
| ADR-013 | `nalgebra` vs hand-rolled Kalman? | Start of Phase 4 |
| ADR-014 | Normalized vs pixel coords in guidance | Phase 4 |
| ADR-015 | Ollama embed vs ONNX embedder for Qdrant | Start of Phase 7 |
| ADR-016 | Ollama vs vLLM for LLM serving | Phase 8 |
| ADR-018 | Optical flow in ROI vs frame-diff only? | During Phase 4 if coasting is too fragile |

---

## Superseded (Sentinel ISR era)

| ADR | Was | Superseded by |
|-----|-----|----------------|
| Qdrant required | Vector DB for detections | ADR-009 (CSV MVP) |
| Ollama RAG core | Analyst queries | ADR-001 (optional Phase 7) |
| ISR framing | Surveillance RAG | SeekerSim brief |

---

## Template

```markdown
## ADR-NNN: Title
**Status:** Proposed | Accepted
**Date:** YYYY-MM-DD
**Context:** ...
**Decision:** ...
**Alternatives:** ...
**Consequences:** ...
```
