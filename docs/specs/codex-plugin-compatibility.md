---
title: Codex Plugin Compatibility
status: active
owner: Elegy
---

# Codex Plugin Compatibility

## Contract

Elegy exports Codex plugins as a host projection. `elegy-plugin/v2` is the
publishable portable manifest; v1 remains install/read compatible. Codex-only
fields stay under `extensions["codex.plugin/v1"]`.

Authority:

```text
Rust types in shared/plugin-sdk
  -> generated Elegy schemas
    -> host exporter
      -> generated .codex-plugin/plugin.json
```

Generated schemas and Codex manifests are derived outputs. They do not define
the portable contract.

## Base field ownership

| Field | Owner | Consumers | Decision |
|---|---|---|---|
| `schemaVersion` | `ElegyPluginV2` | parser, validator, inspect | Publish `elegy-plugin/v2`; retain v1 compatibility. |
| `name` | `ElegyPluginV1` | validator, inspect, export, archive identity | Keep required kebab-case. |
| `version` | `ElegyPluginV1` | validator, inspect, export, archive identity | Keep required SemVer. |
| `description` | `ElegyPluginV1` | validator, inspect, export | Keep required non-blank text. |
| `author` | `ElegyPluginV1Author` | validator, inspect, export/import | Keep portable publisher metadata. |
| `license` | `ElegyPluginV1` | export/import | Keep portable package metadata. |
| `repository` | `ElegyPluginV1` | validator, export/import | Keep portable package metadata. |
| `skills` | `ElegyPluginV1` | verifier, inspect, export, pack | Keep portable component path. |
| `mcpServers` | `ElegyPluginV1` | verifier, inspect, Claude export, pack | Keep portable descriptor path. Do not reuse it for Codex runtime config. |
| `capabilityCatalog` | `ElegyPluginV2` | verifier, inspect, every host export, pack | Required in v2 as typed executable discovery authority. |
| `connections` | `ElegyPluginV2` | verifier, inspect, host export | Required in v2. Declares `none` or a governed requirements file, plus an optional provider descriptor. |
| `readiness` | `ElegyPluginV2` | verifier, discovery, every host export, pack | Required in v2. Evidence controls routing and ships with the package. |
| `extensions` | `ElegyPluginV1` | extension validator and host adapters | Keep optional. Omit empty maps and empty host extensions. |

## Codex extension ownership

Current-compatible means accepted by the installed Codex plugin validator.
Experimental means documented or preserved for round-trip import but excluded
from default export until validator evidence changes.

| Elegy source | Codex output | State | Evidence and behavior |
|---|---|---|---|
| `schemaVersion` | none | Elegy contract | Require exact `codex.plugin/v1`; never emit the extension version. |
| `homepage`, `keywords` | same field | Current-compatible | Typed import/export; accepted by the installed validator. |
| base `version` | `version` with `+codex.<projectionDigest12>` | Current-compatible | Export keeps the Elegy SemVer prefix and adds deterministic build metadata so Codex can pick up fast-moving projection changes. |
| `interface` | `interface` | Current-compatible | Current export requires validator-required fields and supports `logoDark`. |
| `connectionBindings` | `.app.json` | Current-compatible | Explicitly maps portable requirement IDs to registered opaque Codex app IDs. The exporter carries the requirement's `required` flag. |
| `apps` | `apps` path plus `.app.json` | v1 compatibility | Hand-authored/catalog-derived app files remain supported only for legacy v1 plugins. |
| `hooks` | default `hooks/hooks.json` | Current-compatible file discovery | Current export copies the file without a manifest field. |
| `hooks` | `hooks` manifest field | Experimental | Emitted only with explicit experimental mode; installed validator rejects it. |
| `mcpServers` | `mcpServers` | Current-compatible | Companion file is parsed and statically validated before export. |
| `assets` | copied files only | Elegy packaging metadata | Never emitted into Codex `plugin.json`. |
| unknown fields | same field | Experimental | Preserved on import and emitted only in explicit experimental mode. |
| `bundledContentVariant`, `binary` | none by default | Unsupported | Retained only as unknown imported data; no typed Elegy abstraction. |

## Export modes

| Mode | CLI | Contract |
|---|---|---|
| Current | default | Reject missing required publisher/interface metadata; omit manifest hooks and unknown fields; pass the installed validator. |
| Experimental | `export --experimental-codex` | Preserve documented experimental hooks and unknown imported fields; caller accepts validator incompatibility. |

## Companion contracts

| Surface | Current contract | Known correction |
|---|---|---|
| `skills` | `./skills/` | Require `./`-prefixed portable paths. |
| `.app.json` | app references with registered opaque `id`, `required`, and optional `category` | In v2, generated only from declared connection requirements plus explicit Codex bindings. OAuth, token storage, and runtime connection state remain host-owned. |
| `.mcp.json` | `mcpServers` object | Validate the companion file. v1 stores its path and does not model inline objects. Target-specific archives may use target-specific command paths. Windows `bin/` commands must reference a Windows-runnable file such as `.exe`, `.cmd`, `.bat`, or `.ps1`. |
| hooks | command handlers in `hooks/hooks.json` | Treat manifest `hooks` as experimental while retaining default-file discovery. |
| interface assets | files under the plugin archive | Validate `composerIcon`, `logo`, `logoDark`, and PNG screenshots. |

## Capability-kind mapping

The `elegy-capability-catalog/v1` catalog declares each capability's execution
kind. Authentication requirements are separately owned by
`elegy-plugin-connections/v1`.

| Catalog `kind` | Codex export | Authority |
|---|---|---|
| `cli` | Invoked by skills or MCP server. No dedicated Codex file. | Catalog `invocation` field. |
| `mcp` | `.mcp.json` | Catalog `invocation` field with `toolName`. |
| `app-binding` | v1 compatibility projection | Catalog service slug is legacy metadata and is never used as a v2 Codex app ID. |

For v2, `.app.json` is generated only when every declared connection has an
explicit `connectionBindings` entry. Codex then owns the OAuth/connection flow
and shows whether the registered app is connected. Catalog inference and
hand-authored app copying remain v1 compatibility behavior.

A capability may declare a `fallback` surface (typically `cli`) for hosts that
do not support the primary kind. The Codex exporter does not emit fallback into
the Codex plugin — it is host-neutral guidance.

See [capability-catalog-v1 spec](capability-catalog-v1.md) for the full
capability shape.

## Audit findings

- The active adapter manifest keeps Codex-only data under
  `extensions["codex.plugin/v1"]`; empty extension maps are omitted.
- Every active Elegy adapter plugin has a capability catalog. Skill-only
  bundles may be valid native Codex plugins, but they are not Elegy adapter
  plugins and are outside Elegy marketplace projection.
- `.app.json` is connection-declaration-driven for v2. The SDK retains a
  legacy v1 fixture for catalog inference and proves that v2 cannot use it.
- MCP companion files are parsed and validated during export/verify.
- Default Codex export omits validator-rejected manifest hooks and unknown
  fields unless explicit experimental export is requested.
- Marketplace Codex export accepts target-specific adapter archives.
- Archive and host-export binary inclusion use explicit CLI arguments, not an
  extension `binary` field.

## Import behavior

`import_codex_plugin_v1` reads `.codex-plugin/plugin.json`, maps portable fields
to `ElegyPluginV1`, and preserves Codex-only fields under
`extensions["codex.plugin/v1"]`. Unknown Codex fields stay in the extension's
`extra` map. Import preservation does not imply default-export support.

## Marketplace projection

`elegy-plugin-packaging marketplace export-codex` converts an
`elegy-marketplace/v1` root into a Codex marketplace tree. It exports each local
wrapper under `plugins/<name>/`, preserves entry order and category, defaults
Codex policy to `AVAILABLE` and `ON_INSTALL`, resolves the selected target's
verified binary when an archive contains one, and omits Elegy artifact fields
from the generated index. The Codex index is derived output. For Windows
targets, export rejects `.mcp.json`
commands under `bin/` when they omit a Windows-runnable extension or point at a
missing file.

Use `marketplace export-codex --check` to compare an existing generated tree
against current projection output without rewriting it.

Generated Codex plugin versions use:

```text
<elegy-version>+codex.<projectionDigest12>
```

`projectionDigest12` must change when exported manifest content, skills,
catalog metadata, companion files, or bundled binary checksums change. Generated
control files are excluded from digest inputs to avoid self-referential churn,
including install receipts and generated `.elegy-plugin/`, `.codex-plugin/`,
and `.claude-plugin/` manifest copies inside staged projection trees.

Release CI publishes one Codex marketplace projection per supported target:

```text
elegy-codex-marketplace-<target>.zip
elegy-codex-marketplace-<target>.zip.sha256
```

Each archive contains `.agents/plugins/marketplace.json`, generated
`.codex-plugin/plugin.json` files, skills, companion files, and target binaries.

## Non-goals

- Do model the need for a connection and its host binding. Do not place OAuth
  secrets, tokens, authorization codes, or connector runtime state in the
  Elegy manifest.
- Do not widen `ElegyPluginV1` with Codex-only fields.
- Do not treat Codex projections as the source of truth for portable archives.
- Do not emit unknown imported Codex fields in current-compatible mode.

## Validation

Use the narrowest checks for changed boundaries:

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
cargo run -p elegy-documentation -- check --project .
```

Regenerate checked-in schemas only from the canonical Rust types:

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --write
```
