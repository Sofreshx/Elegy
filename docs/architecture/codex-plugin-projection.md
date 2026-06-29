# Codex Plugin Projection

Codex export is a derived host projection over the `elegy-plugin/v1` plugin manifest.
The portable plugin archive (`.plugin.zip`) is the primary release contract; Codex
export generates a `.codex-plugin/plugin.json` and `skills/` directory from the
plugin manifest through `elegy-plugin-packaging export --host codex`.

Codex-specific metadata lives in the manifest's `extensions["codex.plugin/v1"]`
namespace. The base `elegy-plugin/v1` manifest is not widened with host-specific fields.

The Codex plugin projection is documented in `shared/plugin-sdk/src/lib.rs`
(`CodexPluginExtensionV1`, `export_plugin_v1`).
