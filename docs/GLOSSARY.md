# Glossary — SeekerSim

Terms used in code comments, docs, and interviews.

---

## Guidance & control

| Term | Meaning |
|------|---------|
| **Seeker** | Notional sensor platform that produces imagery (here: camera / video). |
| **Line of sight (LOS)** | Direction from seeker to target, usually an angle. |
| **LOS rate** | Time derivative of LOS; key input to proportional navigation. |
| **Pure pursuit** | Steer toward where the target is now. |
| **Proportional navigation (PN)** | Commanded acceleration proportional to LOS rate; classic intercept teaching law. |
| **Navigation constant (N)** | Gain in PN (typically 3–5 in textbooks). |
| **Closing velocity** | Rate at which seeker and target range decreases. |
| **Software-in-the-loop (SIL)** | Control laws run in software against modeled dynamics—not hardware. |

---

## Tracking & estimation

| Term | Meaning |
|------|---------|
| **Detection** | One box in one frame from the neural network. |
| **Track** | Same logical object linked across frames (`track_id`). |
| **Association** | Matching a detection to an existing track (we use IoU). |
| **Kalman filter** | Recursive estimator for position/velocity from noisy measurements. |
| **Coast** | Predict without measurement when detector misses a frame. |
| **IoU** | Intersection-over-union — overlap score between two boxes. |
| **NMS** | Non-maximum suppression — remove duplicate detections. |

---

## Computer vision & ML

| Term | Meaning |
|------|---------|
| **YOLO** | You Only Look Once — one-stage object detector family. |
| **ONNX** | Portable model format consumed by ONNX Runtime. |
| **Bounding box** | Rectangle around detected object. |
| **Confidence** | Model score for a detection (0–1). |
| **Letterboxing** | Resize preserving aspect ratio with padding. |
| **Execution provider** | ONNX backend: CPU, CUDA, etc. |

---

## Simulation

| Term | Meaning |
|------|---------|
| **Interceptor** | Simulated pursuer whose motion we command. |
| **Target** | Simulated object being tracked (motion may be kinematic from vision or simple model). |
| **Miss distance** | Closest approach range between interceptor and target. |
| **dt** | Simulation time step per frame. |

---

## Rust & systems

| Term | Meaning |
|------|---------|
| **Ownership** | Each value has one owner; prevents data races. |
| **`Result<T,E>`** | Ok or error return type. |
| **`Option<T>`** | Some or None. |
| **`Arc`** | Shared ownership for ONNX session across tasks. |
| **`tokio`** | Async runtime. |

---

## Project-specific

| Term | Meaning |
|------|---------|
| **SeekerSim** | This repository’s product name. |
| **Run** | One end-to-end processing of an input → output folder. |
| **Pipeline** | `process_frame` loop orchestrating all stages. |
| **Telemetry** | CSV/JSONL written per run. |
