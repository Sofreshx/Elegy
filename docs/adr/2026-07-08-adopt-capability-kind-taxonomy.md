---
title: Adopt Capability-Kind Taxonomy
status: accepted
owner: elegy-core
---

# Adopt Capability-Kind Taxonomy

## Status

Accepted.

## Context

Elegy's `elegy-capability-catalog/v1` currently models every capability as a
CLI invocation (`executable` + `command`). There is no way to declare that a
capability is an MCP tool, a Codex app binding, or a provider adapter. Codex
app connectors (`.app.json`) are hand-authored connector-reference files that
are disconnected from the capability catalog — there is no link between a
capability's identity and the Codex connector it maps to.

The capability catalog is also not a shared governed Rust type in
`shared/plugin-sdk`. The only Rust type is `ElegyPluginCapabilityCatalog`, which
is a path reference (`{path, schemaVersion, readinessCommand}`). The catalog
contents are modeled only in plugin-local code, despite being referenced by the
portable `elegy-plugin/v1` manifest.

Codex plugins support skills, app integrations, MCP servers, and hooks. Elegy
needs a clear taxonomy that maps each capability kind to the correct Codex
export surface while keeping the portable manifest host-neutral.

## Decision

### 1. Promote the capability catalog to a shared governed Rust contract

`elegy-capability-catalog/v1` becomes a typed Rust contract in
`shared/plugin-sdk` with generated JSON schema and shared validation. It is
part of the portable manifest surface (referenced by `capabilityCatalog` in
`elegy-plugin/v1`) and should be governed as a shared contract, not
plugin-local.

### 2. Introduce a capability `kind` taxonomy

Each capability in the catalog declares a `kind`:

| Kind | Description | Codex export surface |
|---|---|---|
| `cli` | Executable deterministic or controlled commands. Invoked via `elegy-*` binaries. | Invoked by skills or MCP server. |
| `mcp` | Typed agent-facing tool server. | `.mcp.json` |
| `app-binding` | Host-authenticated external-service connector. Maps to a Codex app connector. | `.app.json` |

`provider-adapter` (for AI provider calls) is deferred until a real
AI-provider consumer exists, per the substrate-governance public-surface
graduation rule.

### 3. Add a `fallback` mechanism

A capability can declare a fallback surface. This lets the Codex export prefer
the host-native connector while other hosts use the fallback (typically a CLI).

```json
{
  "id": "github.pr-triage",
  "kind": "app-binding",
  "appBinding": { "connector": "github", "category": "Developer Tools" },
  "fallback": {
    "kind": "cli",
    "invocation": { "executable": "gh", "command": ["pr", "list"] }
  }
}
```

### 4. App bindings declared in the catalog; `.app.json` becomes derived

The capability catalog is the single source of truth for app bindings. The
Codex exporter generates `.app.json` from `app-binding` capabilities in the
catalog. Hand-authored `.app.json` files are no longer the authority.

Transition rule: if `codex.plugin/v1.apps` path exists and the catalog has no
`app-binding` capabilities, the exporter keeps copying the hand-authored file
(backward compat). If both exist, catalog wins.

### 5. Portable/Codex split

`kind`, `fallback`, and the `appBinding.connector` (external-service identity
like `github`) are portable and host-neutral. The Codex exporter emits the
connector as the `id` in `.app.json`. No Codex-only fields are added to the
base `ElegyPluginV1` manifest — they stay in the catalog and projection layers.

### 6. Backward compatibility

When deserializing a catalog that omits `kind`, default to `cli`. This
preserves compatibility with existing catalog files. Authored output must
include `kind` explicitly.

## Consequences

- `shared/plugin-sdk` gains `ElegyCapabilityCatalogV1`, `ElegyCapability`,
  `ElegyCapabilityKind`, `ElegyCapabilityFallback`, `ElegyAppBinding` types
  with generated JSON schema and validation.
- The Codex exporter gains a catalog-driven `.app.json` generation path.
- Existing catalog files (currently only `plugins/planning`) need `kind: cli`
  added to each capability. The defaulting rule prevents breakage during
  transition.
- Plugin-local catalog deserialization can migrate to the shared type.
- The capability catalog schema becomes a governed artifact under
  `shared/plugin-sdk/schemas/`.
- A fixture proves the app-binding → `.app.json` export path.

## Validation

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --write
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
cargo test -p elegy-planning
cargo run -p elegy-core --bin elegy-contracts -- --project . contracts validate
cargo run -p elegy-documentation -- check --project .
```
