---
title: Repository Layout
status: active
owner: elegy-core
doc_kind: reference
---

# Repository Layout

Elegy separates shipped surfaces by role. Directory placement is part of the
contract because release tooling, agent navigation, and validation all depend on
the same surface taxonomy.

## Directory Kinds

| Directory | Contract |
| --- | --- |
| `plugins/` | Bundled installable plugin packages. |
| `tools/` | Standalone CLI crates that are not plugin packages. |
| `hosts/` | Host adapters and transport servers. |
| `skills/` | Standalone skill-only packages. |
| `marketplace-wrappers/` | Public metadata wrappers for external or private plugin archives. |
| `shared/` | Reusable Rust libraries and platform tooling. |
| `distribution/` | Canonical release and surface catalog. |
| `docs/` | Architecture, ADRs, specs, governance, and operations docs. |
| `examples/` | Acceptance examples and golden fixtures. |

## Required Shape

| Kind | Root | Required files |
| --- | --- | --- |
| Bundled plugin | `plugins/{plugin-name}` | `.elegy-plugin/plugin.json`, `skills/elegy-{skill-id}/SKILL.md`, `DISTRIBUTION.md` |
| Standalone CLI | `tools/{tool-name}` | `Cargo.toml`, `src/`, `DISTRIBUTION.md` when shipped |
| Host adapter | `hosts/{host-name}` | `Cargo.toml`, `src/`, `DISTRIBUTION.md` when shipped |
| Skill package | `skills/elegy-{skill-id}` | `SKILL.md` |
| Marketplace wrapper | `marketplace-wrappers/{plugin-name}` | `.elegy-plugin/plugin.json` |
| Shared crate | `shared/{crate-name}` | `Cargo.toml`, `src/` |

`distribution/surfaces.json` is the release catalog. Every shipped CLI, plugin
archive, skill package, host adapter, and wrapper must have one catalog entry.

## Migration Rule

The repository uses this shape now. Run the shape checker in blocking mode
before merging layout, catalog, packaging, or artifact-hygiene changes:

```powershell
pwsh scripts/check-repo-shape.ps1 -Project . -FailOnIssues
```

## Anti-Patterns

| Pattern | Fix |
| --- | --- |
| Transport adapters under `plugins/` | Move to `hosts/` or an owning plugin adapter directory. |
| Standalone CLI crates under `plugins/` | Move to `tools/`. |
| External/private marketplace wrappers under `plugins/` | Move to `marketplace-wrappers/`. |
| Flat `SKILL.md` directly under `plugins/{name}` | Move to `skills/elegy-{skill-id}/SKILL.md` or make it a bundled plugin skill. |
| Active `.cargo/config.toml` with local paths | Keep only `.cargo/config.example.toml` in the repo. |
| Local database or agent state files | Ignore and keep outside version control. |

## Validation

Use the narrowest check for the changed surface:

```powershell
pwsh scripts/check-repo-shape.ps1 -Project . -Json
cargo metadata --format-version 1 --no-deps
cargo fmt --all --check
```

When package manifests, marketplace entries, or governed artifacts change, also
run the relevant packaging and documentation checks from `AGENTS.md`.
