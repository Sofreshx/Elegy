# Plugin Wrapper Template

Use this template to register a plugin whose implementation ships outside this
repository. The wrapper is public metadata. Runtime behavior lives in compiled
artifacts or an external service.

## Usage

1. Copy this directory to `marketplace-wrappers/<your-plugin-name>/`
2. Edit `.elegy-plugin/plugin.json`
3. Add the plugin to `distribution/surfaces.json`
4. Regenerate `.elegy/marketplace.json`

Set `"marketplacePublished": false` until public archives and `.sha256`
sidecars exist. Draft wrappers stay in source, but they are omitted from the
generated installable marketplace.

## Directory structure

```
marketplace-wrappers/<your-plugin-name>/
  .elegy-plugin/
    plugin.json       # discovery metadata
  README.md           # local docs about this wrapper (optional)
```

No `src/` directory is required. Put implementation code in the external
repository, compiled archive, or hosted service.

## Manifest rules

- Use `license: "Proprietary"` when the implementation is closed source.
- Omit `repository` when the source location is private.
- Do not put secrets in manifests, skills, scripts, descriptors, or archives.
- Keep host-specific display metadata under `extensions`.

```json
{
  "schemaVersion": "elegy-plugin/v1",
  "name": "elegy-my-plugin",
  "version": "0.1.0",
  "description": "Short user-facing capability summary.",
  "author": {
    "name": "Example Publisher"
  },
  "license": "Proprietary",
  "extensions": {
    "elegy.marketplace-wrapper/v1": {
      "schemaVersion": "elegy.marketplace-wrapper/v1",
      "sourceRepository": "https://github.com/org/private-plugin"
    }
  }
}
```

## Marketplace entry source

Add one packaged surface to `distribution/surfaces.json`:

```json
{
  "name": "elegy-my-plugin",
  "kind": "external-plugin-wrapper",
  "packaging": "plugin",
  "pluginRoot": "marketplace-wrappers/my-plugin",
  "artifactBaseUrl": "https://github.com/org/private-plugin/releases/download",
  "marketplacePublished": false,
  "marketplaceCategory": "Developer Tools",
  "description": "Short user-facing capability summary."
}
```

Then run:

```bash
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project .
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace validate --source .
```

The generated marketplace lives at `.elegy/marketplace.json`. Hosts own install
state, authentication, approvals, and execution policy.
