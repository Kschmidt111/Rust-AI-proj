# SeekerSim crate (Phase 1D)

Library + binary layout; HTTP routes under `src/api/`, config in `src/config.rs`, logging via `tracing`.

## Layout

```
src/
├── main.rs           # thin entry
├── lib.rs            # run(), shared modules
├── config.rs
├── telemetry/        # logging init (metrics later)
└── api/
    └── routes/
        └── health.rs
```

## Run

```powershell
cd crates/seeker-sim
cargo run
```

Config: `config/default.toml` at repo root (`[server].bind`).

```powershell
curl.exe http://127.0.0.1:8080/health
cargo test
```

Debug logs:

```powershell
$env:RUST_LOG = "seeker_sim=debug"
cargo run
```
