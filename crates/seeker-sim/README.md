# SeekerSim crate (Phase 1B)

Minimal HTTP server with `GET /health`.

## Run

```powershell
cd crates/seeker-sim
cargo run
```

In another terminal:

```powershell
curl http://127.0.0.1:8080/health
```

Expected: `{"status":"ok","service":"seeker-sim"}`

See [docs/LEARNING_ROADMAP.md](../../docs/LEARNING_ROADMAP.md) for next steps (Phase 1C: config).
