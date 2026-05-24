# Pre-push rules (SeekerSim)

**Purpose:** Keep personal data, credentials, and sensitive material off GitHub.  
**Audience:** You and the AI assistant — review this before every `git push`.

**For coding standards, learning pace, and architecture:** see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

---

## Golden rule

> If you would not paste it in a public Discord channel, do not commit it.

When in doubt, **leave it out** or put it in a gitignored path (`data/`, `models/`, `.env`).

---

## Never commit

| Category | Examples |
|----------|----------|
| **Secrets & tokens** | API keys, GitHub PATs (`ghp_…`), OpenAI keys (`sk-…`), AWS keys (`AKIA…`), JWTs, passwords, connection strings with credentials |
| **Environment files** | `.env`, `.env.local`, `secrets.toml`, `credentials.json`, `*.pem`, `*.pfx`, `id_rsa`, `*.key` |
| **Personal identity** | Home address, phone, email, SSN, passport, clearance details, employee IDs |
| **Private paths (when avoidable)** | Windows user folders in docs if they expose your username — prefer generic paths like `C:\Projects\seeker-sim` in examples |
| **Classified / restricted data** | Military imagery, ITAR-controlled files, employer-internal docs, proprietary datasets |
| **Large personal media** | Home security camera footage, private photos, videos with faces/license plates you do not have rights to share |
| **Built artifacts with debug info** | `target/`, `*.pdb`, local crash dumps |
| **Model weights (large)** | `*.onnx`, `*.pt`, `*.bin` — use `models/` (gitignored) |

---

## Allowed on GitHub (this project)

| Item | Notes |
|------|--------|
| **Public / synthetic sample data** | Tiny synthetic frames only if explicitly licensed or generated |
| **`config/default.toml`** | Must contain **no secrets** — only localhost bind, public paths, thresholds |
| **Architecture & learning docs** | No real operational details |
| **`Cargo.lock`** | Normal for Rust applications |
| **Code & comments** | No embedded API keys; use env vars or config (gitignored) for secrets later |

---

## Pre-push checklist (human + AI)

Run through this **before every push**:

### 1. Inspect what will leave your machine

```powershell
cd C:\Projects\Rust-AI-proj   # your local clone
git status
git diff
git diff --cached
```

### 2. Scan staged files for common secret patterns

```powershell
git diff --cached | Select-String -Pattern 'ghp_|gho_|github_pat_|sk-[a-zA-Z0-9]{10,}|AKIA[0-9A-Z]{16}|api[_-]?key|secret|password\s*=|token\s*=|Bearer\s+[a-zA-Z0-9\-_.]+' -AllMatches
```

If anything matches, **stop** — remove the secret, rotate the credential if it was ever committed, and use `.env` (gitignored) instead.

### 3. Confirm gitignore is doing its job

These must **not** appear in `git status` as staged:

- `.env` / `.env.*`
- `data/` contents (except `data/README.md`)
- `models/*.onnx`
- `target/`

### 4. Review new files by name

Reject or gitignore if you see:

`*.pem`, `*.key`, `credentials*`, `secrets*`, `.env*`, `*.pfx`, `id_rsa*`

### 5. Branch hygiene

- Day-to-day work on **`dev`**
- Merge to **`master`** via pull request only
- Do **not** force-push to `master` unless you explicitly intend to and understand the risk

### 6. Optional automated script

```powershell
.\scripts\pre-push-check.ps1
```

---

## If a secret was committed (recovery)

1. **Rotate/revoke** the credential immediately (GitHub, OpenAI, etc.).
2. Remove from the repo history — deleting the file in a new commit is **not enough** if it was pushed; use [GitHub secret scanning](https://docs.github.com/en/code-security/secret-scanning) guidance or `git filter-repo` / BFG for history rewrite.
3. Notify no one publicly with the secret value in the issue description.

---

## AI assistant obligations

Before suggesting or running `git push`, the assistant must:

1. Run `git status` and `git diff` (staged + unstaged).
2. Flag any file matching the **Never commit** table.
3. Flag diffs matching secret regex patterns above.
4. **Refuse to push** if secrets or obvious personal data are present until removed.
5. **Never** commit `.env`, keys, or real imagery paths without explicit user approval.
6. Prefer generic paths in documentation examples.

---

## Safe patterns for later phases

| Need | Do this | Not this |
|------|---------|----------|
| API key for Ollama (future) | `OLLAMA_HOST` in `.env` (gitignored) | Key in `config/default.toml` |
| Model path | `models/yolov8n.onnx` in gitignored folder | Commit the `.onnx` file |
| Sample video | `data/videos/` gitignored | Commit MP4 to repo |
| Local bind | `127.0.0.1:8080` in committed TOML | Production URLs with auth tokens |

---

## Related files

- [.gitignore](.gitignore) — ignored paths
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — branch workflow (§7)
- [.cursor/rules/pre-push-security.mdc](.cursor/rules/pre-push-security.mdc) — Cursor agent rule (auto-reminder)

---

## Change log

| Date | Change |
|------|--------|
| 2026-05-23 | Initial pre-push security rules |
