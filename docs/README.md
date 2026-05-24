# Documentation index

Single source of truth while building **SeekerSim**.

## Implementation status

| Phase | Topic | Status |
|-------|--------|--------|
| 0 | Docs foundation | Done |
| 1 | HTTP `/health`, config, layout | Done |
| 2 | YOLO single-image `detect` | Done |
| 3 | Frame folder `process` | Done |
| 4 | Tracking + Kalman + CSV | Next |
| 5 | PN guidance + 2D sim | Planned |
| 6 | One-command demo | Planned |
| 7–8 | Qdrant + Ollama RAG | Planned |

Details per phase: [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md).

---

## Start here

0. **[DEVELOPMENT.md](DEVELOPMENT.md)** — how we code, learn Rust, scale the repo  
1. [TOOLS.md](TOOLS.md) — tools/libraries and rationale  
2. [ARCHITECTURE.md](ARCHITECTURE.md) — diagrams, flows, types, file structure  
3. [LEARNING_ROADMAP.md](LEARNING_ROADMAP.md) — what to build each phase  

## Reference

- [DECISIONS.md](DECISIONS.md) — ADRs (e.g. ADR-017 hybrid perception for small targets)  
- [GLOSSARY.md](GLOSSARY.md) — terminology  

## Data flow (current + planned)

```
Frames on disk → ingest → vision (YOLO) → [tracking] → [guidance] → [sim] → CSV/plots
                                              ↘ Qdrant → Ollama RAG   (Phases 7–8)
```

Phases in brackets are not implemented yet. Per-frame loop detail: [ARCHITECTURE.md §2](ARCHITECTURE.md#2-architectural-flow-per-frame).
