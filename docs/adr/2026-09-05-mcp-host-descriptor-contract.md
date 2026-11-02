---
title: MCP host descriptor contract and degraded-mode startup
status: accepted
owner: Elegy
created: 2026-09-05
---

# MCP host descriptor contract and degraded-mode startup

## Context

No ADR or spec governed `elegy.toml`'s `[descriptors]` contract or the
`elegy-run` MCP host's startup behavior. The repo root's own `elegy.toml`
relied on the implicit default pattern (`elegy.resources.d/*.toml`) instead
of declaring it, and its only matching file,
`elegy.resources.d/planning.toml`, was a minimal placeholder
(`content = "Minimal descriptor for MCP host."`). Commit `e6c89c9` ("Remove
obsolete generated artifacts") deleted it because nothing declared it
load-bearing.

From that point, `elegy-run --project D:\Projets\Elegy` — the exact
invocation Claude Desktop's `elegy-planning` MCP registration used — failed
`compose_runtime_state` with `DESC-CONFIG-002` ("descriptor include patterns
did not match any files") before opening the MCP transport
(`hosts/host-mcp/src/host.rs`, `serve_stdio_with_options` composed runtime
state *before* calling `.serve(stdio())`). The host process exited before
answering `initialize`. Claude Desktop saw only "Connection closed" and
retried indefinitely — a multi-day silent reconnect loop with no actionable
diagnostic anywhere in the chain.

Nothing would have caught this: `shared/descriptor` had no `tests/`
directory at all; `hosts/host-mcp`'s tests only exercised `ElegyMcpHost` over
an in-process `tokio::io::duplex` transport, pointed at `examples/*` fixtures
that all declare descriptors explicitly, never at the repository root; no CI
workflow invoked `elegy-run`; and `.elegy/checks.json` had no check covering
MCP host startup.

## Decision

**The strict descriptor contract in `shared/descriptor` is preserved.** A
project whose `descriptors.include` pattern (explicit or the implicit
default `elegy.resources.d/*.toml`) matches zero files is still a hard error
(`DESC-CONFIG-002`) from `load_project_from_root_config`. We did not relax
this to make an empty project "legal" — a descriptor-less project is not a
supported state for `compose_runtime_state`/`compose_catalog`, and every
other `elegy.toml` in the repo already declares descriptors explicitly.

**The MCP host layer, not the descriptor contract, absorbs the failure.**
`elegy-run` now always completes the MCP `initialize` handshake:

- If `compose_runtime_state` succeeds, `ElegyMcpHost` serves the real
  resource catalog exactly as before.
- If it fails, `ElegyMcpHost` starts in a `Degraded` state: `initialize`
  still succeeds, `get_info().instructions` names the failing diagnostic
  code(s), `list_resources` returns exactly one entry
  (`elegy://host/diagnostics`), and reading that resource returns the full
  structured diagnostic list as JSON. A client that only checks "did the
  server start" (as Claude Desktop's MCP client does) gets a real answer
  instead of a bare transport close.
- `list_tools`/`call_tool` are unaffected either way — they were already
  unconditionally empty/erroring (MCP tool projection was removed in favor
  of direct CLI invocation), which the docs previously misstated.
- A `ServerInitializeError::ConnectionClosed` during `.serve()` (the client
  tearing down the transport mid-handshake — observed historically as
  `renderer released port`) is logged and treated as a clean exit, not
  propagated as a Debug-formatted error.

**A `--preflight` flag makes the failure checkable without a client.**
`elegy-run --project <path> --preflight` resolves and composes the project,
prints a human-readable summary or diagnostic list to stderr, and exits 0/1
without opening any transport. This is wired into `.elegy/checks.json` as a
blocking `commit`-profile check and into `rust-ci.yml`'s `test` job, so a
future deletion of a load-bearing descriptor fails a local commit or CI run
instead of only surfacing as a silent MCP disconnect loop days later.

**The repository root is a genuine descriptor-bearing MCP project.**
`elegy.toml` now declares `[descriptors]` explicitly, and
`elegy.resources.d/elegy-authority.toml` exposes real, small (well under the
1 MiB `policy.filesystem.max_file_size_bytes` default) authority documents —
`docs/readiness.md`, `docs/architecture/README.md`, `docs/docs-index.md`,
`distribution/surfaces.json` — as filesystem resources, scoped to
`policy.filesystem.roots = ["docs", "distribution"]`.
`elegy.resources.d/README.md` states explicitly that the directory is
hand-authored and load-bearing, specifically to prevent a repeat of
`e6c89c9`'s silent deletion.

Stderr-only `tracing` logging was added to `hosts/host-mcp` (mirroring
`hosts/memory-mcp/src/observability.rs`'s stderr-only pattern), since the
host previously had no logging at all and its only failure signal was Rust's
default `Error: {:?}` dump on stdout-adjacent stderr.

## Consequences

- A project with descriptors that fail to resolve is still rejected by
  `shared/descriptor`/`shared/core` for any caller that calls
  `compose_runtime_state`/`compose_catalog` directly (e.g. `elegy-contracts`,
  `elegy-mcp`); only the `elegy-run` MCP host degrades gracefully, because it
  is the layer with an MCP client depending on a completed handshake.
- `elegy-run`'s resource catalog now includes the repo's own authority docs.
  Regenerating `docs/readiness.md` or editing `distribution/surfaces.json`
  changes what this MCP surface serves; that's intended, not accidental
  scope creep — these are exactly the documents an agent host should be able
  to read via MCP without a CLI round-trip.
- `elegy.resources.d/` is now explicitly load-bearing repo content. A future
  "remove obsolete generated artifacts" pass must not delete it without
  checking `.elegy/checks.json`'s `elegy-run-preflight` and this ADR first.
- `--preflight` and the diagnostic resource are `elegy-run`-specific; other
  MCP hosts in this repo (`hosts/capability-mcp`, `hosts/memory-mcp`) have
  their own startup contracts and are not covered by this decision.
- `README.md` and `docs/agent-integration.md`'s MCP sections were corrected
  to show `--project`, to state that `elegy-run` serves resources only (not
  tools), and to document degraded mode — they previously showed a bare
  `elegy-run` invocation that fails from the repo root and implied tool
  support that has never existed in this host.
