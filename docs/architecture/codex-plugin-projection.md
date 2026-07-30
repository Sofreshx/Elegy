# Codex Plugin Projection

Codex export is a derived host projection over the `elegy-plugin/v2` plugin manifest.
The portable adapter archive format is the primary package contract. Release
archives use `<surface>-plugin-<target>.zip`;
Codex export generates a `.codex-plugin/plugin.json` and `skills/` directory
from the plugin manifest through `elegy-plugin-packaging export --host codex`.

Codex-specific metadata lives in the manifest's `extensions["codex.plugin/v1"]`
namespace. The base manifest owns portable connection requirements, while the
Codex extension owns opaque registered app bindings.

Required authority and output flow:

```text
ElegyPluginV1 Rust types
  -> generated Elegy schemas
    -> Codex projection
      -> .codex-plugin/plugin.json and companion files
```

Import can preserve unknown Codex fields for round trips. Preservation does not
make a field part of current-compatible default export. The compatibility spec
owns the field-level support matrix. Rust types and generated schemas are the
machine-readable manifest authority.

## Compatibility target

Current Codex plugin compatibility covers:

| Codex surface | Elegy authority |
|---|---|
| `.codex-plugin/plugin.json` identity and display metadata | `elegy-plugin/v2` plus `extensions["codex.plugin/v1"]` |
| `skills/` | base `skills` field |
| `.app.json` app references | `connections` plus `extensions["codex.plugin/v1"].connectionBindings` |
| lifecycle hooks | `extensions["codex.plugin/v1"].hooks` or default `hooks/hooks.json` |
| plugin-bundled MCP config | `extensions["codex.plugin/v1"].mcpServers` |
| marketplace UI metadata | `extensions["codex.plugin/v1"].interface` |

Default export targets the installed Codex plugin validator. Documented fields
that the validator rejects remain explicit experimental projections. They do
not widen the base manifest.

`elegy-plugin-packaging export --host codex` selects current-compatible output.
Add `--experimental-codex` only when the caller explicitly accepts fields that
the installed validator rejects. Current mode copies lifecycle hooks to
`hooks/hooks.json` for default discovery and omits the rejected manifest field.

Rust-backed plugins include a compiled executable explicitly:

```text
elegy-plugin-packaging export --host codex \
  --binary <compiled-file> \
  --binary-name bin/<plugin-name>
```

The manifest does not infer a build artifact. The caller selects the target
binary and its host-relative path.

Every Codex, OpenCode, and Claude export also retains a stripped
`.elegy-plugin/plugin.json` plus its capability catalog, readiness/evidence,
connection descriptors, skills, portable MCP descriptors, and selected binary.
The stripped manifest omits host extensions so another harness never inherits
Codex-only asset claims. Native host manifests remain projections over that
same portable core.

Codex app files are generated from declared connection requirements. Each
portable requirement ID must have an explicit `connectionBindings` entry with
the opaque ID of a registered Codex app. Service names are never inferred as
Codex IDs.

```json
{
  "apps": {
    "github-main": {
      "id": "connector_76869538009648d5b282a4bb21c3d157",
      "required": true
    }
  }
}
```

Plugin install does not imply connection. Codex owns app authentication,
credential storage, tool approvals, and runtime connection state. Legacy v1
catalog inference remains read/export compatible but is disabled for v2.

## SDK surface

The Codex plugin projection is implemented in `shared/plugin-sdk/src/lib.rs`:

- `CodexPluginManifest`
- `CodexPluginExtensionV1`
- `CodexPluginInterface`
- `CodexAppsFile`
- `CodexHooksConfig`
- `import_codex_plugin_v1`
- `export_plugin_v1`
- `export_plugin_v1_with_codex_mode_and_binary`

Field status, failure behavior, and companion-file contracts are defined in
[`docs/specs/codex-plugin-compatibility.md`](../specs/codex-plugin-compatibility.md).
