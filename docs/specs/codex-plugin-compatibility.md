---
title: Codex Plugin Compatibility
status: active
owner: Elegy
---

# Codex Plugin Compatibility

## Contract

Elegy exports Codex plugins as a host projection. `elegy-plugin/v1` remains the
portable manifest. Codex-only fields stay under `extensions["codex.plugin/v1"]`.

## Field mapping

| Elegy source | Codex output |
|---|---|
| `name`, `version`, `description`, `author`, `license`, `repository` | `.codex-plugin/plugin.json` |
| `skills` | copied to `skills/` and emitted as `"skills": "./skills"` |
| `extensions["codex.plugin/v1"].homepage` | `homepage` |
| `extensions["codex.plugin/v1"].keywords` | `keywords` |
| `extensions["codex.plugin/v1"].interface` | `interface` |
| `extensions["codex.plugin/v1"].apps` | `apps` path plus copied `.app.json` |
| `extensions["codex.plugin/v1"].hooks` | `hooks` path plus copied hooks file |
| default `hooks/hooks.json` | copied and emitted when no explicit hooks path exists |
| `extensions["codex.plugin/v1"].mcpServers` | `mcpServers` path plus copied file or directory |
| `extensions["codex.plugin/v1"].assets` | copied asset files or directories |

## Import behavior

`import_codex_plugin_v1` reads `.codex-plugin/plugin.json`, maps portable fields
to `ElegyPluginV1`, and preserves Codex-only fields under
`extensions["codex.plugin/v1"]`. Unknown Codex fields stay in the extension's
`extra` map.

## Non-goals

- Do not model OAuth scopes, app actions, token storage, or connector approval
  policy in the Elegy manifest.
- Do not widen `ElegyPluginV1` with Codex-only fields.
- Do not treat Codex projections as the source of truth for portable archives.

## Validation

Use the narrowest checks for changed boundaries:

```bash
cargo test -p elegy-plugin-sdk
cargo test -p elegy-skills
cargo run -p elegy-documentation -- check --project .
```
