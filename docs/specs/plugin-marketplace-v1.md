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
- A `distribution/surfaces.json` surface may set `artifactBaseUrl` to publish
  plugin archives from an external release repository.
- Skill-only plugin packages publish one `target: "any"` artifact named
  `<plugin-name>-plugin-any.zip`; the archive has no `bin/` directory.
- External wrapper surfaces with `marketplacePublished: false` are drafts. The
  generated marketplace omits them until the public archives exist.
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
elegy-plugin-packaging marketplace status --source . --plugin elegy-planning --json
elegy-plugin-packaging marketplace update elegy-planning --source . --json
elegy-plugin-packaging marketplace monitor --source . --plugin elegy-planning --jsonl
elegy-plugin-packaging marketplace export-codex --source . --target x86_64-pc-windows-msvc --output ./dist/codex
elegy-plugin-packaging marketplace export-codex --source . --target x86_64-pc-windows-msvc --output ./dist/codex --check
elegy-plugin-packaging marketplace export-codex --source . --target x86_64-pc-windows-msvc --artifact-dir ./artifacts/distribution --output ./dist/codex
elegy-plugin-packaging marketplace export-codex --source . --plugin elegy-opencode-workers --target x86_64-pc-windows-msvc --output ./dist/codex
```

`--source` accepts a local root or an HTTPS base URL. Remote roots must serve
`.elegy/marketplace.json` and the referenced plugin manifests at their relative
paths. The repository index defaults to the rolling `main-snapshot` release;
stable consumers can regenerate it with an explicit `--release-tag`.

Codex export resolves the selected target artifact, verifies and stages it, and
copies its binary into the derived plugin. Omit `--target` to use the current
supported host target. Use `--plugin <name>` to export one marketplace entry
without materializing unrelated plugin artifacts. Use `--artifact-dir <path>`
when release assets have already been downloaded by CI; exporter validation
stays strict and does not silently repair invalid artifacts.

Freshness status compares the selected marketplace artifact sidecar with the
installed plugin receipt and manifest. Agents should call it only for explicit
operator checks, monitors, or capability-preflight failures. Normal skill use
must not poll freshness every turn.

Freshness statuses:

| Status | Meaning | Action |
|---|---|---|
| `current` | Installed receipt, manifest, and selected artifact checksum match. | No repair. |
| `notInstalled` | Marketplace entry exists, but no installed receipt or manifest exists. | Install selected plugin. |
| `stale` | Installed plugin identity matches, but version, artifact checksum, or capability digest differs. | Update selected plugin. |
| `missingArtifact` | Marketplace entry points at an artifact or projection file that is unavailable. | Repair release/projection source, then rerun install/export. |
| `checksumUnavailable` | Artifact exists, but the required SHA-256 sidecar is missing or unreadable. | Publish or repair checksum sidecar before install/update. |
| `identityMismatch` | Artifact manifest name/version does not match the marketplace wrapper. | Refuse install/update until the release artifact is corrected. |
| `unsupportedTarget` | No exact target artifact and no `any` fallback exists for the requested target. | Publish a supported target artifact or choose a supported target. |
| `unknown` | Status could not be determined, usually because installed state or host listing failed. | Surface error to operator; retry explicit check/update after cause is fixed. |

Apps and operators may surface any non-`current` status as actionable. Strict
validation fails closed on missing artifacts, checksum failures, identity
mismatches, unsupported targets, or stale generated projections. Agents should
not poll freshness during normal turns.

## Closed-source wrappers

Public wrappers may use `license: "Proprietary"` and omit `repository`. Put
private behavior in compiled artifacts or a hosted service. Do not put secrets
in manifests, skills, scripts, app descriptors, or archives.

Discovery-only wrappers must declare:

```json
{
  "extensions": {
    "elegy.marketplace-wrapper/v1": {
      "schemaVersion": "elegy.marketplace-wrapper/v1"
    }
  }
}
```

This marker allows a wrapper manifest to omit `skills` and `mcpServers`. The
published plugin archive still supplies runtime files.

Wrapper fixups are explicit metadata, not exporter guesses:

| Field | Use |
|---|---|
| `windowsBinaryName` | Rename staged `bin/<name>` to `bin/<name>.exe` for Windows Codex projection. |
| `windowsMcpCommandRewrites` | Rewrite known `.mcp.json` command strings for Windows projection. |

External/private plugin publish contract:

| Owner | Contract |
|---|---|
| External plugin repo | Publishes `<plugin-name>-plugin-<target>.zip` and `.sha256` sidecars to the public marketplace release under the tag used by the marketplace. |
| Public Elegy repo | Stores wrapper metadata and generates `.elegy/marketplace.json` with `artifactBaseUrl`. |
| Installer/exporter | Downloads the artifact, verifies the sidecar checksum, and checks that the archive manifest matches the wrapper manifest. |

Private implementation source can stay private. The archive URL must be
reachable by the intended installer audience. For the public Elegy marketplace,
publish closed-source plugin archives to the public `Sofreshx/Elegy` release
referenced by `artifactBaseUrl`.

## Validation

```bash
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project . --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace validate --source .
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
```
