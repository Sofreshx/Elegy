---
title: Codex Plugin Compatibility
status: active
owner: Elegy
---

# Codex Plugin Compatibility

## Contract

Elegy exports Codex plugins as a host projection. `elegy-plugin/v1` remains the
portable manifest. Codex-only fields stay under `extensions["codex.plugin/v1"]`.

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
| `schemaVersion` | `ElegyPluginV1` | parser, validator, inspect | Keep exact `elegy-plugin/v1`. |
| `name` | `ElegyPluginV1` | validator, inspect, export, archive identity | Keep required kebab-case. |
| `version` | `ElegyPluginV1` | validator, inspect, export, archive identity | Keep required SemVer. |
| `description` | `ElegyPluginV1` | validator, inspect, export | Keep required non-blank text. |
| `author` | `ElegyPluginV1Author` | validator, inspect, export/import | Keep portable publisher metadata. |
| `license` | `ElegyPluginV1` | export/import | Keep portable package metadata. |
| `repository` | `ElegyPluginV1` | validator, export/import | Keep portable package metadata. |
| `skills` | `ElegyPluginV1` | verifier, inspect, export, pack, scaffold | Keep portable component path. |
| `mcpServers` | `ElegyPluginV1` | verifier, inspect, Claude export, pack | Keep portable descriptor path. Do not reuse it for Codex runtime config. |
| `extensions` | `ElegyPluginV1` | extension validator and host adapters | Keep optional. Omit empty maps and empty host extensions. |

## Codex extension ownership

Current-compatible means accepted by the installed Codex plugin validator.
Experimental means documented or preserved for round-trip import but excluded
from default export until validator evidence changes.

| Elegy source | Codex output | State | Evidence and behavior |
|---|---|---|---|
| `schemaVersion` | none | Elegy contract | Require exact `codex.plugin/v1`; never emit the extension version. |
| `homepage`, `keywords` | same field | Current-compatible | Typed import/export; accepted by the installed validator. |
| `interface` | `interface` | Current-compatible | Current export requires validator-required fields and supports `logoDark`. |
| `apps` | `apps` path plus `.app.json` | Current-compatible | Installed validator accepts connector `id` plus optional `category`. |
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
| `.app.json` | connector references with `id` and optional `category` | Remove Elegy's unsupported `required` field. Do not add OAuth, token, action, or approval policy. |
| `.mcp.json` | `mcpServers` object | Validate the companion file. v1 stores its path and does not model inline objects. |
| hooks | command handlers in `hooks/hooks.json` | Treat manifest `hooks` as experimental while retaining default-file discovery. |
| interface assets | files under the plugin archive | Validate `composerIcon`, `logo`, `logoDark`, and PNG screenshots. |

## Audit findings

- `validate_plugin_mcp_tool_references` is a public no-op with no consumers.
- All seven bundled Elegy manifests declare an otherwise empty
  `codex.plugin/v1` extension.
- Scaffolding always emits an empty `extensions` map and a null `mcpServers`.
- MCP verification counts JSON files but does not parse descriptors.
- Default Codex export re-emits unknown fields and fields rejected by current
  validation.
- Archive and host-export binary inclusion use explicit CLI arguments, not the
  extension's `binary` field.

## Import behavior

`import_codex_plugin_v1` reads `.codex-plugin/plugin.json`, maps portable fields
to `ElegyPluginV1`, and preserves Codex-only fields under
`extensions["codex.plugin/v1"]`. Unknown Codex fields stay in the extension's
`extra` map. Import preservation does not imply default-export support.

## Non-goals

- Do not model OAuth scopes, app actions, token storage, or connector approval
  policy in the Elegy manifest.
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
