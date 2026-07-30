# Codex plugin projection

```text
elegy-plugin/v3
  |-- Codex-native package fields --------------------+
  `-- elegy governance                                |
             |                                        |
             `-- validate auth/readiness/files         |
                                                      v
                              remove schemaVersion + elegy only
                                                      |
                                                      v
                                      .codex-plugin/plugin.json
```

Codex is the reference package model. Elegy adds portable governance without
wrapping or translating native fields. Codex export therefore requires
structural preservation of every declared native field and package
verification of every referenced known path. This is not a claim that Codex
runtime behavior has been exercised.

MCP OAuth remains host/server behavior. Codex receives the native MCP
configuration and performs supported login itself; Elegy never receives its
tokens during projection. A host that cannot represent a declared package or
authentication boundary receives an error, not a degraded plugin.

The Rust authority is `ElegyPluginV3`, `import_codex_plugin_v3`,
`project_codex_plugin_v3`, and `export_plugin_with_policy` in
`shared/plugin-sdk`. Generated schemas and host bundles are derived outputs.

Unknown future native fields survive the manifest projection, but Elegy does
not infer their semantics or referenced assets. Runtime parity requires a
separate clean Codex receipt.

The checked compatibility status and exclusions are in
[the compatibility spec](../specs/codex-plugin-compatibility.md).
