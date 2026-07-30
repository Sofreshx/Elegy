---
title: Adopt evidence-backed readiness and a system-adapter plugin boundary
status: accepted
owner: elegy-core
created: 2026-07-30
---

# Adopt evidence-backed readiness and a system-adapter plugin boundary

## Context

Elegy documentation and discovery previously treated several different facts as if they proved the same thing:

- source code existed;
- fixtures or schemas validated;
- a package could be assembled;
- a capability worked after a clean installation;
- a real consumer successfully used it.

They do not prove the same thing. This allowed agents to discover unfinished surfaces and encouraged product repositories to conform to abstractions that had not earned reuse.

The `plugin` label was also applied too broadly. Client Radar, AI Radar, and Question Studio are domain/business products. Their analysis and workflow logic may be valuable, but that does not make them reusable agent-to-system adapters.

## Decision

### Readiness stages

Every distributed surface owns one `elegy-readiness/v1` artifact:

| Stage | Meaning | Required evidence |
|---|---|---|
| `concept` | Design, scaffold, or blocked experiment. | No execution claim. |
| `implemented` | Executable source behavior exists and its source/package checks pass. | `source-tests` and `package-verification`. |
| `usable` | A clean packaged installation completes a non-fixture end-to-end task in a declared environment. | Implemented evidence plus `clean-install` and `real-task` with `nonFixture: true`. |
| `production` | A usable surface is released and relied upon by an identified real consumer. | Usable evidence plus `release` and `consumer`. |

Missing readiness remains readable for v1 compatibility, defaults to `implemented`, and is never agent-routable. New v2 manifests must reference readiness explicitly.

Compilation, fixtures, schema conformance, generated projections, and package verification cannot promote a surface to `usable`.

### Discovery

Default marketplace generation, listing, search, installation, Codex export, skills, and agent-facing recommendations expose only `usable` and `production` surfaces. Maintainers may inspect or export incubating surfaces only through an explicit override that preserves their non-routable label.

An empty default marketplace is valid. Catalog size is not a success metric.

### Plugin eligibility

An Elegy plugin is a reusable system adapter. It connects an agent to at least one concrete boundary:

- data source or database;
- external platform or API;
- local filesystem, operating system, or application;
- executable or CLI that exposes reusable operations.

Business rules, scoring, analysis, domain workflows, reports, and product orchestration belong in a library, application, Automation Pack, or product-local command. Packaging domain logic in a manifest does not turn it into a plugin.

Client Radar, AI Radar, and Question Studio are removed from Elegy plugin distribution. Their owning products may consume Elegy adapters or expose a qualifying adapter separately.

Codex uses the word plugin for an installable bundle that may contain only
skills, only an MCP server, or both. Elegy deliberately uses a narrower product
classification while remaining able to project into that native bundle:

- `surfaceClass: adapter-plugin` means a reusable external-system adapter;
- `surfaceClass: tool` means executable product or development behavior;
- `surfaceClass: skill` means instructions and supporting resources;
- `surfaceClass: host-adapter` means an optional protocol projection;
- `surfaceClass: host-extension` means host-specific behavior.

Only an active `adapter-plugin` may set `packaging: plugin`. It must declare a
typed capability catalog. Skills may guide use of that boundary, and MCP may
project it, but neither is executable product evidence by itself.

### Simplification and extraction

A generalized Elegy contract, adapter family, or projection may expand only after two independent working consumers demonstrate the same stable boundary. An extraction proposal must identify:

1. both existing consumers;
2. duplicated working behavior;
3. the smallest common contract;
4. behavior intentionally left product-local.

Possible future hosts, providers, or workflows are not consumer evidence. Until proof exists, use a concrete product-local command.

Generalized surfaces without two consumers are frozen to fixes, removal, or work that obtains concrete evidence. They gain no new schema versions, adapters, projections, or compatibility claims.

## Consequences

- The generated [ecosystem readiness matrix](../readiness.md) is the default human-facing inventory.
- [Deprecations and reclassification](../deprecations.md) records the removed
  plugin labels and the retained replacement surfaces.
- Accounts is the only retained active adapter plugin in the initial cleanup.
  Desktop and Observe are adapter candidates under rework.
- Elegy can honestly provide useful implemented libraries and development tools while exposing no default agent-routable plugins.
- Forge is described as an implemented development tool, not a production automation product.
- Care is described as an implemented diagnostic prototype, not a standalone support product.
- Agents must not introduce Elegy dependencies merely because a schema or wrapper exists.
- Promotion is evidence review, not documentation wording.

## Validation

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project . --check
cargo run -p elegy-documentation -- export readiness --project . --output docs/readiness.md
cargo run -p elegy-documentation -- check --project .
```
