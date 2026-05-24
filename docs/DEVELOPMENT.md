# Development guidelines — SeekerSim

**Single reference for how we write code, learn Rust, and keep the codebase professional as it grows.**

Audience: you (C# SWE learning Rust) and the AI assistant. Read this before starting a new phase or opening a PR.

Related docs: [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md) · [ARCHITECTURE.md](ARCHITECTURE.md) · [DECISIONS.md](DECISIONS.md) · [rules.md](../rules.md) (secrets only)

---

## 1. Core principles

**Portfolio north star:** [PROJECT_BRIEF.md § Design north star](PROJECT_BRIEF.md#design-north-star--ai-engineer-positioning-2026) — local models, vector DB ingestion, low-latency orchestration (not an OpenAI wrapper).

| Principle | Meaning |
|-----------|---------|
| **Go slowly** | One phase (or sub-step) at a time. Each step must compile, run, and have a clear test before moving on. |
| **Learn in the code** | Heavily commented `pub fn`; C# analogies where they help. Prefer clarity over cleverness. |
| **Professional layout early** | lib+bin, modules per concern, typed errors, `tracing` — even when the feature is small. |
| **Minimal diffs** | Only change what the current phase needs. No drive-by refactors or speculative features. |
| **Architecture is the contract** | New code goes where [ARCHITECTURE.md](ARCHITECTURE.md) says it goes. Update the doc if layout changes. |

---

## 2. Pacing — how we work phase by phase

Follow [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md). Within each phase:

1. **Define the smallest testable slice** (e.g. 1A → 1B → 1C, not “all of Phase 1 at once”).
2. **Implement** with comments on every public function.
3. **Verify** — command you can run (`cargo test`, `curl`, sample image, etc.).
4. **Commit on `dev`** with a message that names the phase.
5. **Open PR to `master`** when the phase (or sub-phase) is demo-ready.

Do **not** skip ahead (e.g. Phase 4 tracking before Phase 2 detection works on one image).

Suggested side-project pace: ~1 phase per week (see roadmap table).

---

## 3. Code comment standard (required)

Every **`pub fn`**, **`pub async fn`**, and **`pub struct`** used across modules gets a doc comment:

```rust
/// One-sentence summary.
///
/// # Arguments
/// * `name` — what it means
///
/// # Returns
/// What the caller gets; mention errors if non-obvious.
///
/// # C# analogy
/// Optional: how this maps from C# / .NET.
pub async fn example(path: &Path) -> Result<Foo, MyError> {
```

- Private helpers: at least a one-line `//` summary above the function.
- Non-obvious blocks (tensor prep, Kalman math, PN): short inline comments.
- Do **not** comment every line of boilerplate.

Formal decision: [DECISIONS.md ADR-008](DECISIONS.md).

---

## 4. Repository & module layout

### 4.1 Top level

```
Rust-AI-proj/
├── config/default.toml     # committed defaults, no secrets
├── crates/seeker-sim/      # all application Rust code
├── docs/                   # architecture & guidelines
├── scripts/                # ffmpeg, model download, pre-push-check
├── data/                   # gitignored runtime data
└── models/                 # gitignored weights
```

### 4.2 Crate layout (scale here)

```
crates/seeker-sim/src/
├── main.rs           # thin: telemetry::init, config load, seeker_sim::run
├── lib.rs            # pub mod …; run()
├── config.rs
├── telemetry/
├── api/routes/       # HTTP only — no ML
├── domain/           # types + errors (Phase 2+)
├── ingest/           # Phase 3+
├── vision/           # Phase 2+
├── tracking/         # Phase 4+
├── guidance/         # Phase 5+
├── sim/              # Phase 5+
└── pipeline/         # Phase 3+ orchestration
```

**Rule:** add new phases as **new modules**, not new logic in `main.rs`.

### 4.3 Module dependency rule (strict)

```text
api  →  pipeline  →  { ingest, vision, tracking, guidance, sim, telemetry }
                         ↓
                      domain
```

| Module | May depend on | Must not depend on |
|--------|---------------|---------------------|
| **domain** | std, serde | axum, ort, tokio HTTP, api |
| **vision** | domain, ort (in `detector.rs` only) | api |
| **api** | pipeline or handlers that call services | ort internals directly |
| **pipeline** | all service modules | — |

Violating this creates circular deps and makes testing impossible.

---

## 5. Rust / industry conventions we follow

| Topic | Convention |
|-------|------------|
| **Errors** | `thiserror` enums per module; `Result` at boundaries; avoid `unwrap()` in library code except startup in `main` |
| **Logging** | `tracing` + `tracing-subscriber`; no `println!` in library paths |
| **Config** | TOML in `config/default.toml`; secrets in `.env` (gitignored) later — never commit tokens |
| **Async** | Tokio + Axum for HTTP; CPU-heavy ONNX in `spawn_blocking` when added (Phase 2+) |
| **Tests** | Unit tests next to logic (`#[cfg(test)]`); route tests with `tower`; integration tests in `tests/` when needed |
| **Naming** | `snake_case` functions/modules, `PascalCase` types, crate `seeker-sim` / lib `seeker_sim` |
| **Public API** | Expose only what other modules need (`pub` sparingly) |

---

## 6. Adding a new feature (checklist)

Use this for Phase 2 onward:

- [ ] Phase listed in [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md) — am I on the right one?
- [ ] Module path exists in [ARCHITECTURE.md](ARCHITECTURE.md) file tree
- [ ] New dependency justified in [TOOLS.md](TOOLS.md) or new ADR in [DECISIONS.md](DECISIONS.md)
- [ ] Types live in `domain/` if shared
- [ ] HTTP surface only in `api/routes/<resource>.rs`
- [ ] Doc comments + at least one test or manual verification step documented
- [ ] No secrets, personal data, or large binaries ([rules.md](../rules.md))
- [ ] `cargo fmt`, `cargo clippy`, `cargo test` pass

---

## 7. Git workflow

| Branch | Use |
|--------|-----|
| **`dev`** | All daily commits |
| **`master`** | Stable; merge via PR only |

Before push:

```powershell
.\scripts\pre-push-check.ps1
```

See [GITHUB_SETUP.md](GITHUB_SETUP.md).

---

## 8. AI assistant obligations

When implementing or reviewing code, the assistant must:

1. Read this file and the current phase in [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md).
2. Propose the **smallest testable step** before large implementations.
3. Match existing module layout and naming in `crates/seeker-sim/`.
4. Add doc comments and C# analogies on new `pub` items.
5. Avoid scope creep (no Phase 3 pipeline while doing Phase 2 detect).
6. Run `cargo test` / `cargo build` after Rust changes when possible.
7. Follow [rules.md](../rules.md) before any push.

Cursor rule: [.cursor/rules/seeker-sim-development.mdc](../.cursor/rules/seeker-sim-development.mdc).

---

## 9. What “done” looks like per phase

| Phase | Verification |
|-------|----------------|
| 1 | `curl.exe http://127.0.0.1:8080/health` + `cargo test` |
| 2 | One image → JSON detections printed or returned |
| 3 | Frame folder processed; per-frame log lines |
| 4 | `tracks.csv` with stable `track_id` |
| 5 | `guidance.csv` + sim plot |
| 6 | README demo reproducible in &lt; 30 min |

---

## 10. When to update documentation

| Change | Update |
|--------|--------|
| New module or folder | [ARCHITECTURE.md](ARCHITECTURE.md) §10 + change log |
| New tool / crate dep | [TOOLS.md](TOOLS.md) |
| Design choice | [DECISIONS.md](DECISIONS.md) new ADR |
| New phase complete | [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md) checkbox / README status |

---

## Change log

| Date | Change |
|------|--------|
| 2026-05-23 | Initial development guidelines (consolidated learning + architecture rules) |
