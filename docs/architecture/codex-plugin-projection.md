# Codex Plugin Projection

Codex export is a derived host projection over the `elegy-plugin/v1` plugin manifest.
The portable plugin archive (`.plugin.zip`) is the primary release contract; Codex
export generates a `.codex-plugin/plugin.json` and `skills/` directory from the
plugin manifest through `elegy-plugin-packaging export --host codex`.

Codex-specific metadata lives in the manifest's `extensions["codex.plugin/v1"]`
namespace. The base `elegy-plugin/v1` manifest is not widened with host-specific fields.

## Compatibility target

Current Codex plugin compatibility covers:

| Codex surface | Elegy authority |
|---|---|
| `.codex-plugin/plugin.json` identity and display metadata | `elegy-plugin/v1` plus `extensions["codex.plugin/v1"]` |
| `skills/` | base `skills` field |
| `.app.json` connector references | `extensions["codex.plugin/v1"].apps` |
| lifecycle hooks | `extensions["codex.plugin/v1"].hooks` or default `hooks/hooks.json` |
| plugin-bundled MCP config | `extensions["codex.plugin/v1"].mcpServers` |
| marketplace UI metadata | `extensions["codex.plugin/v1"].interface` |

Codex app connector files are local connector references:

```json
{
  "apps": {
    "github": {
      "id": "connector_...",
      "required": true
    }
  }
}
```

They are not an OAuth, token, provider-action, or approval-policy schema.
Hosts own connector authentication, tool approvals, and runtime sessions.

## SDK surface

The Codex plugin projection is implemented in `shared/plugin-sdk/src/lib.rs`:

- `CodexPluginManifest`
- `CodexPluginExtensionV1`
- `CodexPluginInterface`
- `CodexAppsFile`
- `CodexHooksConfig`
- `import_codex_plugin_v1`
- `export_plugin_v1`
