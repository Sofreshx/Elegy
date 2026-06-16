---
title: Block all crates.io publishing; keep dry-run smoke test
status: accepted
date: 2026-06-15
owner: Elegy
---

# Block all crates.io publishing; keep dry-run smoke test

## Context

Elegy distributes through GitHub Releases, binary artifacts, wrapper
surfaces, and agent-facing skill + MCP projections. The primary
consumers are AI-agent hosts, not Rust developers composing with
Elegy library crates.

Four crates did not set `publish = false`:
`elegy-memory`, `elegy-planning`, `elegy-skills`, `elegy-contracts`.
The `publish-crate.yml` workflow was wired to publish `elegy-memory`
to crates.io on release events.  This pipeline used nonexistent
action versions (`actions/checkout@v6.0.2`,
`actions/upload-artifact@v7.0.1`, `actions/download-artifact@v8.0.1`)
and would have failed regardless.

## Decision

1. **Block all 28 crates** from crates.io with `publish = false` in
   each `Cargo.toml`.  This is the default safety net — no crate can
   be published accidentally.

2. **Keep `publish-crate.yml` as a dry-run smoke test** (renamed
   `Crate Verify`).  The workflow runs `cargo verify-project`, `cargo
   test`, and `cargo publish --dry-run` on PRs and pushes to `main`.
   The dry-run step fails while `publish = false` is set — this is
   the intentional safety net.

3. **Preserve the publish gate** for future use.  The `Publish to
   crates.io` step remains, gated behind `github.event_name ==
   'release'`.  It will only succeed after someone consciously removes
   `publish = false` from a target crate and a GitHub Release is
   published.

4. **Dormant-by-design.**  The publish step is intentionally dormant.
   Its presence in the workflow is not a loophole or an oversight.
   Activation requires three independent conscious actions: removing
   `publish = false` from the crate's `Cargo.toml`, configuring the
   `CARGO_REGISTRY_TOKEN` secret, and publishing a GitHub Release.
   Without all three, the step is inert.  This is consistent with the
   "block by default" posture — the gate exists so the workflow
   definition does not need rewriting when publishing is eventually
   desired, but it must never fire accidentally.

## How to enable publishing for a crate

1. Remove `publish = false` from the crate's `Cargo.toml`.
2. Configure the `CARGO_REGISTRY_TOKEN` repository secret.
3. Publish a GitHub Release.  The `cargo publish --dry-run` step will
   pass, followed by the actual `cargo publish`.

Do not remove `publish = false` from crates whose primary
distribution path is the elegy umbrella CLI or GitHub Releases.

## Consequences

- crates.io publishing is blocked by default at the Cargo.toml level.
- The `Crate Verify` workflow serves as a smoke test (manifest
  validity + tests) and a dry-run readiness check for future
  publishing.
- All GitHub Actions workflows now use valid, pinned action versions
(`actions/checkout@v6.0.2`, `actions/upload-artifact@v4`,
   `actions/download-artifact@v4`).
