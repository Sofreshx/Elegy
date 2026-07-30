---
name: elegy-plugin-authoring
description: Use when creating or reviewing an Elegy system-adapter plugin that connects an agent to a data source, database, platform, API, local system, or executable boundary.
---

# Elegy Plugin Authoring

> **Readiness: concept; not agent-routable.** This authoring guidance is being proven under the evidence-backed readiness policy.

An Elegy plugin is a reusable system adapter: it connects an agent to a data source, database, platform, API, filesystem or operating-system boundary, or an executable/CLI. Business and domain logic belongs in a library, application, or product-local tool. Do not create a plugin manifest or marketplace wrapper merely to distribute domain logic.

## Workflow

1. Read the nearest repo instructions, then `docs/architecture/README.md`, `docs/architecture/skill-core-v1.md`, `docs/architecture/codex-plugin-projection.md`, `docs/specs/plugin-connections-v1.md`, `docs/specs/plugin-marketplace-v1.md`, and the current plugin examples that match the target lane.
2. Apply the eligibility gate before authoring anything:
   - Identify the external or local system boundary being adapted.
   - Identify reusable operations an agent can invoke across tasks.
   - Reject domain analysis, scoring, orchestration, or product workflow logic; keep it product-local.
3. Inspect existing manifests, `distribution/surfaces.json`, skills, runtime crates, wrappers, and marketplace state before deciding.
4. Choose the smallest adapter lane:
   - CLI-backed adapter: governed executable operations with a required typed capability catalog and optional skills/MCP projection.
   - Downstream adapter repository: external repo owning runtime, tests, release archive, and sidecars.
   - Instruction-only material: publish through the host's skill lane, not the Elegy plugin marketplace.
5. Do not extract a generalized Elegy contract until two independent working consumers exhibit the same stable boundary. Record what remains product-local.
6. Clarify only decisions that files cannot answer: plugin purpose, public/private source split, runtime surface, connection/authentication requirements, marketplace category, user-visible prompts, and acceptance evidence.
7. Read `references/snippets.md` after choosing the lane, then adapt the relevant snippets to the target repo.
8. Remove every placeholder before finishing. Do not leave toy commands, dummy tests, generic skill prose, or commented-out future code.
9. Validate with the narrowest relevant checks.

## Contracts

- `.elegy-plugin/plugin.json` is the plugin manifest authority.
- Every active Elegy adapter plugin declares a typed `capabilityCatalog`.
- New and publishable plugins use `elegy-plugin/v2` and explicitly declare
  `connections.requirements.mode` as `none` or `declared`.
- A service slug is not a Codex app ID. Connected Codex plugins require an
  explicit opaque `connectionBindings` mapping; the host owns OAuth and
  connection state.
- Never put credentials, tokens, authorization codes, or an authentication flow
  in skills, CLI arguments, or MCP tool inputs. Use a host connection or a
  credential-owning `elegy-connection-provider/v1`.
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

## References

- Snippets: `references/snippets.md`
- Architecture entrypoint: `docs/architecture/README.md`
- Skill authority: `docs/architecture/skill-core-v1.md`
- Codex projection: `docs/architecture/codex-plugin-projection.md`
- Marketplace contract: `docs/specs/plugin-marketplace-v1.md`
