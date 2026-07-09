---
title: Agent-Ready Repo Pack Demo
status: active
owner: Elegy
doc_kind: generated
---

# Agent-Ready Repo Pack Demo - Elegy Repo

**Generated:** 2026-07-08
**Tool:** elegy-checks v0.1.0
**Repo:** Sofreshx/Elegy

## Before

- No `.elegy/checks.json` existed
- No local check registry
- No CI parity mapping
- No structured run evidence

## After

- `.elegy/checks.json` created with 5 checks from 4 packs
- Stacks detected: `agents-instructions`, `docs`, `github-actions`, `rust`, `security-basics`, `specs`
- 2 commit-profile checks passing (rust-format, rust-clippy)
- SQLite evidence store at `~/.elegy/repo-state/<repoId>/checks/checks.sqlite`

## Detected stacks

| Stack | Checks |
|---|---|
| rust | rust-format, rust-clippy, rust-test |
| github-actions | ci-map-pr |
| security-basics | npm-audit-advisory |
| docs | (available, not applied) |
| specs | (available, not applied) |
| agents-instructions | (available, not applied) |

## Applied checks

| Check | Pack | Gate | Profile | Status |
|---|---|---|---|---|
| rust-format | rust | blocking | commit | PASS |
| rust-clippy | rust | blocking | commit | PASS |
| rust-test | rust | blocking | commit | (not in commit profile) |
| ci-map-pr | github-actions | advisory | — | (CI map command) |
| npm-audit-advisory | security-basics | advisory | — | (requires npm) |

## Run result (commit profile)

```
overallPass: true
checksRun: 2
checksPassed: 2
checksFailed: 0
blockingFailures: []
```

## What this proves

1. **Stack detection works** — elegy-checks correctly identified Rust, GitHub Actions,
   security basics, docs, specs, and agent instructions in the Elegy repo.
2. **Check pack application works** — 5 checks applied from 4 detected packs.
3. **Check execution works** — rust-format and rust-clippy both pass on the Elegy repo.
4. **JSON output is stable** — all commands emit structured JSON for agent consumption.
5. **Evidence store works** — run results persisted to SQLite for history/debugging.

## Next steps for clients

A client receiving this pack would get:
1. `.elegy/checks.json` — their check configuration (repo-tracked)
2. This report — what was detected, applied, and verified
3. Instructions to run `elegy-checks run --profile commit` before commits
4. Instructions to run `elegy-checks ci-map --scope pr` for CI parity

## Available but not applied

Additional packs can be applied later:
- `docs` — documentation checks
- `specs` — spec validation checks
- `agents-instructions` — agent instruction surface checks
