---
title: Plugin marketplace v1
status: active
owner: Elegy
---

# Plugin marketplace v1

## Contract

The marketplace root contains `.elegy/marketplace.json`.

```json
{
  "schemaVersion": "elegy-marketplace/v1",
  "name": "elegy",
  "interface": { "displayName": "Elegy" },
  "plugins": [
    {
      "name": "elegy-planning",
      "source": { "source": "local", "path": "./plugins/planning" },
      "category": "Productivity",
      "artifacts": [
        {
          "target": "x86_64-pc-windows-msvc",
          "url": "https://example.invalid/plugin.zip",
          "checksumUrl": "https://example.invalid/plugin.zip.sha256"
        }
      ]
    }
  ]
}
```

Rules:

- Plugin order is presentation order.
- `source.path` stays inside the marketplace root and resolves to a directory
  containing `.elegy-plugin/plugin.json`.
- Entry and plugin manifest names must match.
- Artifact selection prefers the exact host target, then `any`.
- Remote installation requires an HTTPS artifact and SHA-256 sidecar.
- The archive manifest name and version must match the public wrapper manifest.
- Installation normalizes legacy `skills/` and `mcpServers` paths to `./` form;
  authoring and generated manifests remain strict.
- Extraction stages files before atomically publishing the install directory.

## Commands

```bash
elegy-plugin-packaging marketplace generate --project .
elegy-plugin-packaging marketplace validate --source .
elegy-plugin-packaging marketplace list --source . --json
elegy-plugin-packaging marketplace search planning --source . --json
elegy-plugin-packaging marketplace install elegy-planning --source .
elegy-plugin-packaging marketplace export-codex --source . --target x86_64-pc-windows-msvc --output ./dist/codex
```

`--source` accepts a local root or an HTTPS base URL. Remote roots must serve
`.elegy/marketplace.json` and the referenced plugin manifests at their relative
paths. The repository index defaults to the rolling `main-snapshot` release;
stable consumers can regenerate it with an explicit `--release-tag`.

Codex export resolves the selected target artifact, verifies and stages it, and
copies its binary into the derived plugin. Omit `--target` to use the current
supported host target.

## Closed-source wrappers

Public wrappers may use `license: "Proprietary"` and omit `repository`. Put
private behavior in compiled artifacts or a hosted service. Do not put secrets
in manifests, skills, scripts, app descriptors, or archives.

## Validation

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project . --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace validate --source .
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
```
