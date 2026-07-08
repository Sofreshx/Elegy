---
title: Capability Catalog V1
status: active
owner: Elegy
---

# Capability Catalog V1

## Contract

`elegy-capability-catalog/v1` is the shared governed contract for plugin
capability catalogs. It is referenced by the portable `elegy-plugin/v1` manifest
via `capabilityCatalog.path`.

Authority:

```text
Rust types in shared/plugin-sdk
  -> generated Elegy schemas
    -> capability-catalog.json files
```

The catalog is a portable, host-neutral artifact. Codex-specific projection
(such as `.app.json` connector files) is derived by the host exporter, not
authored into the catalog.

## Schema shape

```json
{
  "schemaVersion": "elegy-capability-catalog/v1",
  "plugin": "elegy-planning",
  "pluginVersion": "0.1.0",
  "generatedAt": "2026-07-08T00:00:00Z",
  "digest": "sha256:...",
  "capabilities": [ ... ]
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `schemaVersion` | string | yes | Must be `elegy-capability-catalog/v1`. |
| `plugin` | string | yes | Plugin name (kebab-case). |
| `pluginVersion` | string | yes | Plugin version (SemVer). |
| `generatedAt` | string | no | ISO 8601 generation timestamp. |
| `digest` | string | no | Content digest for integrity. |
| `capabilities` | array | yes | Non-empty array of capabilities. |

## Capability shape

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique capability ID (kebab-case with dots for namespacing). |
| `kind` | string | yes | One of `cli`, `mcp`, `app-binding`. Defaults to `cli` on read when absent (backward compat). |
| `sideEffectClass` | string | yes | One of `pure`, `query`, `mutation`, `fenced-mutation`. |
| `contractVersion` | string | yes | Contract version for the capability. |
| `description` | string | yes | Human-readable description. |
| `invocation` | object | conditional | Required for `cli` and `mcp` kinds. See Invocation. |
| `inputSchema` | object | no | JSON Schema for the capability's input. |
| `outputSchema` | object | no | JSON Schema for the capability's output. |
| `fallback` | object | no | Fallback surface. See Fallback. |
| `appBinding` | object | conditional | Required for `app-binding` kind. See App Binding. |

## Capability kinds

### `cli`

Executable deterministic or controlled commands. Invoked via `elegy-*` binaries.

```json
{
  "id": "project-run.claim.v2",
  "kind": "cli",
  "sideEffectClass": "mutation",
  "contractVersion": "v2",
  "description": "Claim a project run for execution tracking.",
  "invocation": {
    "executable": "elegy-planning",
    "command": ["project-run", "claim"],
    "requiredArgs": ["goal-id", "roadmap-id", "work-point-id"],
    "optionalArgs": ["id", "correlation-id"]
  },
  "inputSchema": { ... },
  "outputSchema": { ... }
}
```

### `mcp`

Typed agent-facing tool server. The invocation points to an MCP server
descriptor.

```json
{
  "id": "memory.search",
  "kind": "mcp",
  "sideEffectClass": "query",
  "contractVersion": "v1",
  "description": "Search local memory entries.",
  "invocation": {
    "executable": "elegy-memory",
    "command": ["mcp-server"],
    "toolName": "memory_search"
  },
  "inputSchema": { ... },
  "outputSchema": { ... }
}
```

### `app-binding`

Host-authenticated external-service connector. Maps to a Codex app connector.
The `appBinding` field declares the portable external-service identity.

```json
{
  "id": "github.pr-triage",
  "kind": "app-binding",
  "sideEffectClass": "query",
  "contractVersion": "v1",
  "description": "Triage GitHub pull requests.",
  "appBinding": {
    "connector": "github",
    "category": "Developer Tools"
  },
  "fallback": {
    "kind": "cli",
    "invocation": {
      "executable": "gh",
      "command": ["pr", "list"]
    }
  },
  "inputSchema": { ... },
  "outputSchema": { ... }
}
```

## Invocation

| Field | Type | Required | Description |
|---|---|---|---|
| `executable` | string | yes | Binary or server name. |
| `command` | array of string | yes | Command segments. |
| `requiredArgs` | array of string | no | Required positional arguments. |
| `optionalArgs` | array of string | no | Optional arguments. |
| `toolName` | string | no | MCP tool name (for `mcp` kind). |

## App binding

| Field | Type | Required | Description |
|---|---|---|---|
| `connector` | string | yes | External-service identity (e.g. `github`, `gmail`, `slack`). Portable and host-neutral. |
| `category` | string | no | Display category for the connector (e.g. `Developer Tools`). |

The Codex exporter emits `connector` as the `id` in `.app.json` and
`category` as the `category`. The connector identity is portable — other hosts
may map it to their own connector system.

## Fallback

| Field | Type | Required | Description |
|---|---|---|---|
| `kind` | string | yes | One of `cli`, `mcp`. |
| `invocation` | object | yes | Invocation for the fallback surface. |

A fallback declares an alternative surface for hosts that do not support the
primary kind. For example, an `app-binding` capability can fall back to a `cli`
invocation.

## Decision rule

Use this to determine the correct kind for a capability:

```text
Can it run locally and deterministically?
  -> cli

Does the host need typed repeated calls into an MCP tool server?
  -> mcp

Does it need host-authenticated external-service integration (GitHub, Gmail, Slack)?
  -> app-binding
```

A plugin without `app-binding` capabilities is still a real plugin. It is
streamlined packaging over skills + CLI/MCP. App bindings are what make a plugin
cross into host-authenticated external-service integration.

## Validation

Capabilities are validated by kind:

- All kinds require `id`, `kind`, `sideEffectClass`, `contractVersion`, `description`.
- `cli` and `mcp` require `invocation`.
- `app-binding` requires `appBinding.connector`.
- `fallback.kind` must be `cli` or `mcp`; fallback requires `invocation`.

## Codex export mapping

| Elegy capability kind | Codex export surface |
|---|---|
| `cli` | Invoked by skills or MCP server. No dedicated Codex file. |
| `mcp` | `.mcp.json` |
| `app-binding` | `.app.json` (generated from catalog `appBinding` fields) |

## Schema generation

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --write
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
```

## Validation commands

```bash
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
cargo run -p elegy-documentation -- check --project .
```
