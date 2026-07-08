---
name: elegy-plugin-authoring
description: Use when creating, bootstrapping, scaffolding, adapting, or reviewing new Elegy plugins in this repo or downstream repositories, including skill-only plugins, Rust CLI-backed plugins, external/private marketplace wrappers, and downstream plugin repos.
---

# Elegy Plugin Authoring

Author Elegy plugins by reading the target repo first, then applying the smallest correct pattern. Do not use or recreate a rigid plugin generator.

## Workflow

1. Read the nearest repo instructions, then `docs/architecture/README.md`, `docs/architecture/skill-core-v1.md`, `docs/architecture/codex-plugin-projection.md`, `docs/specs/plugin-marketplace-v1.md`, and the current plugin examples that match the target lane.
2. Inspect existing manifests, `distribution/surfaces.json`, skills, runtime crates, wrappers, and marketplace state before deciding.
3. Choose one lane:
   - Skill-only plugin: portable agent instructions with no runtime binary.
   - Rust CLI-backed plugin: governed executable behavior with optional skills and capability catalog.
   - External/private marketplace wrapper: public metadata that points at a separately built archive.
   - Downstream plugin repo: external repo that owns its own runtime, tests, release archive, and sidecars.
4. Clarify only decisions that files cannot answer: plugin purpose, public/private source split, runtime surface, marketplace category, user-visible prompts, and acceptance evidence.
5. Read `references/snippets.md` after choosing the lane, then adapt the relevant snippets to the target repo.
6. Remove every placeholder before finishing. Do not leave toy commands, dummy tests, generic skill prose, or commented-out future code.
7. Validate with the narrowest relevant checks.

## Contracts

- `.elegy-plugin/plugin.json` is the plugin manifest authority.
- `distribution/surfaces.json` owns marketplace listing order, category, release routing, and wrapper artifact base URLs.
- `.elegy/marketplace.json` is generated. Do not edit it by hand.
- Codex plugin output is a derived projection. Put Codex-specific metadata under `extensions["codex.plugin/v1"]`.
- CLI invocation templates are the default integration contract. Use MCP only when the host specifically needs an MCP boundary.
- Profiles are allowlists, not approvals. Do not treat a profile as permission for side effects.

## Validation

Run the checks that match the touched surfaces:

```bash
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project .
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project . --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace validate --source .
cargo run -p elegy-documentation -- check --project .
```

For Rust-backed plugins, also run the relevant package tests, for example:

```bash
cargo test -p <plugin-crate>
```

## Release pipeline (private/external plugins)

When authoring a private or external plugin that ships archives to the Elegy
marketplace, read `references/private-plugin-release-pipeline.md` for the
full setup: fine-grained PAT creation, `ELEGY_RELEASE_TOKEN` secret setup,
token-based CI upload (preferred), local build + manual upload (fallback),
and the new-private-plugin checklist.

## References

- Snippets: `references/snippets.md`
- Release pipeline: `references/private-plugin-release-pipeline.md`
- Architecture entrypoint: `docs/architecture/README.md`
- Skill authority: `docs/architecture/skill-core-v1.md`
- Codex projection: `docs/architecture/codex-plugin-projection.md`
- Marketplace contract: `docs/specs/plugin-marketplace-v1.md`
