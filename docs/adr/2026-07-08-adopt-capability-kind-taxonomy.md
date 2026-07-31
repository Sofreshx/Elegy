---
title: Adopt Capability-Kind Taxonomy
status: accepted
owner: elegy-core
---

# Adopt Capability-Kind Taxonomy

## Status

Accepted.

## 2026-07-31 E1 authority amendment

The v1 taxonomy is retained as a compatibility wire shape, but it is no
longer the current authoring model. `elegy-capability-catalog/v2` gives every
capability exactly one concrete interface kind:

| Kind | Interface |
|---|---|
| `cli` | Local executable invocation template |
| `mcp-resource` | Addressable MCP resource |
| `mcp-tool` | Typed MCP tool call |

CLI, MCP resource, and MCP tool are first-class interfaces. They are not
fallbacks or projections of one another. If the same behavior is intentionally
published twice, it uses separate capability IDs and separate evidence.

The v1 `mcp` kind is migration-only and must be classified as a resource or
tool. `app-binding` is legacy compatibility metadata and is only meaningful
when a native Codex app connection binding exists in the host projection. A
portable service slug is not a Codex app ID, and an unbound app-binding entry
is not routable. Connection requirements and opaque host app IDs remain owned
by the host connection boundary.

The v1 `fallback` field has no active runtime consumer. It may be retained in
legacy artifacts as descriptive migration guidance, but it does not create a
second kind, select an invocation, or establish readiness. New runtime code
must not branch on fallback metadata.

Readiness evidence is explicit: `implemented` means source behavior and local
tests; `conformance` means governed contract or fixture interpretation;
`live-proof` means a clean installed non-fixture task; and `routable` means a
validated `usable` or `production` surface. Only the last stage is agent
routable by default.

## 2026-07-30 readiness and eligibility amendment

Capability kinds classify invocation shape; they do not establish that a
surface is a qualifying plugin or a usable product. Default discovery joins the
catalog with `elegy-readiness/v1` and exposes only `usable` or `production`
surfaces.

New adapter kinds and generalized catalog fields are frozen until two
independent working consumers demonstrate the same stable boundary. Domain and
business logic does not become a plugin by declaring a capability kind.

## 2026-07-23 amendment

The catalog's `app-binding` shape remains a v1 compatibility contract, but it
is no longer connection or authentication authority. Treating a portable
service slug such as `github` as a Codex connector ID was incorrect: Codex app
IDs are registered, opaque host identifiers, and connection lifecycle belongs
to the host.

`elegy-plugin/v3` therefore keeps requirements under its `elegy` governance
namespace and preserves native Codex apps. The v3 exporter never derives a
Codex app ID from catalog slugs. Marketplace publication requires v3. See
[Plugin Connections V1](../specs/plugin-connections-v1.md).

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
| `mcp` (v1) | Typed MCP surface; migrate to `mcp-resource` or `mcp-tool`. | `.mcp.json` |
| `app-binding` (v1) | Legacy host-authenticated metadata; requires a native Codex connection binding. | Derived `.app.json` only when bound |

`provider-adapter` (for AI provider calls) is deferred until a real
AI-provider consumer exists, per the substrate-governance public-surface
graduation rule.

### 3. Add a `fallback` mechanism

A v1 capability can declare fallback metadata for migration guidance. Elegy has
no active runtime consumer for it; it does not make the Codex export prefer a
connector or execute another interface.

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

The capability catalog records only legacy portable app-binding metadata. A
native host connection binding is required before a Codex exporter may derive
`.app.json`; the host-owned binding, not the service slug, is the connection
authority. Hand-authored `.app.json` remains a host projection, never the
portable capability authority.

Transition rule: if `codex.plugin/v1.apps` path exists and the catalog has no
`app-binding` capabilities, the exporter keeps copying the hand-authored file
(backward compat). If both exist, catalog wins.

### 5. Portable/Codex split

`kind` and the legacy `appBinding.connector` (an external-service identity like
`github`) are portable and host-neutral. A Codex exporter may emit `.app.json`
only from an explicit native connection binding; it never derives an opaque
Codex app ID from the connector slug. No Codex-only fields are added to the
base manifest — they stay in host connection and projection layers.

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

The current contract authority and migration rules are maintained in
[`capability-catalog-v2.md`](../specs/capability-catalog-v2.md). The commands
below validate the existing v1 compatibility implementation while v2 code is
introduced in a later phase.

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --write
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
cargo test -p elegy-planning
cargo run -p elegy-core --bin elegy-contracts -- --project . contracts validate
cargo run -p elegy-documentation -- check --project .
```
