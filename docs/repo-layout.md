---
title: Repository Layout
status: active
owner: elegy-core
doc_kind: reference
---

# Repository Layout

Elegy records every distributed surface's role in
`distribution/surfaces.json`. Directory names help navigation but are not
classification: several tools retain historical paths under `plugins/` until a
separate low-risk source move is justified.

## Directory Kinds

| Directory | Contract |
| --- | --- |
| `plugins/` | Historical runtime root containing the Accounts adapter, tools, and adapter candidates. |
| `tools/` | Standalone CLI crates that are not plugin packages. |
| `hosts/` | Host adapters and transport servers. |
| `skills/` | Standalone skill-only packages. |
| `marketplace-wrappers/` | Historical or blocked external integration metadata. |
| `shared/` | Reusable Rust libraries and platform tooling. |
| `distribution/` | Canonical release and surface catalog. |
| `docs/` | Architecture, ADRs, specs, governance, and operations docs. |
| `examples/` | Acceptance examples and golden fixtures. |

## Required Shape

| Kind | Root | Required files |
| --- | --- | --- |
| Active adapter plugin | catalog-declared `pluginRoot` (currently `plugins/accounts`) | `.elegy-plugin/plugin.json`, typed capability catalog, readiness artifact, optional skills |
| Standalone tool | catalog-declared `crateRoot` | `Cargo.toml`, `src/`, readiness artifact, and `DISTRIBUTION.md` when shipped |
| Host adapter | `hosts/{host-name}` | `Cargo.toml`, `src/`, `DISTRIBUTION.md` when shipped |
| Skill package | `skills/elegy-{skill-id}` | `SKILL.md` |
| Historical integration metadata | `marketplace-wrappers/{name}` | readiness and limitation documentation; no active plugin manifest unless independently requalified |
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
| Inferring plugin status from a `plugins/` path | Read `surfaceClass`, `lifecycle`, and `packaging` from the catalog. |
| Adding a new standalone tool under `plugins/` | Use `tools/`; historical paths are not precedent. |
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
