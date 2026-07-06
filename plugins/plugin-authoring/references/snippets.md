# Plugin Authoring Snippets

Use these snippets as starting points. Adapt names, descriptions, prompts, paths, and validation to the target repo. Delete fields that do not apply.

## Skill-Only Plugin

Layout:

```text
plugins/<plugin-name>/
  .elegy-plugin/plugin.json
  SKILL.md
  references/...
```

Manifest:

```json
{
  "schemaVersion": "elegy-plugin/v1",
  "name": "elegy-example",
  "version": "0.1.0",
  "description": "Specific task and audience.",
  "author": {"name": "Elegy Contributors"},
  "license": "Apache-2.0",
  "repository": "https://github.com/Sofreshx/Elegy",
  "skills": "./",
  "extensions": {
    "codex.plugin/v1": {
      "schemaVersion": "codex.plugin/v1",
      "interface": {
        "displayName": "Elegy Example",
        "shortDescription": "Do one concrete job",
        "longDescription": "Explain the concrete workflow this skill helps an agent execute.",
        "developerName": "Elegy Contributors",
        "category": "Developer Tools",
        "capabilities": ["Read"],
        "defaultPrompt": ["Use this skill for a concrete task."]
      }
    }
  }
}
```

Distribution entry:

```json
{ "name": "elegy-example", "kind": "skill-only", "description": "Specific task and audience." }
```

## Rust CLI-Backed Plugin

Layout:

```text
plugins/<plugin-name>/
  .elegy-plugin/plugin.json
  Cargo.toml
  DISTRIBUTION.md
  src/
  tests/
  skills/<skill-id>/SKILL.md
  schemas/ or fixtures/ when the plugin owns contracts
```

Manifest:

```json
{
  "schemaVersion": "elegy-plugin/v1",
  "name": "elegy-example",
  "version": "0.1.0",
  "description": "Governed CLI behavior.",
  "author": {"name": "Elegy Contributors"},
  "license": "Apache-2.0",
  "repository": "https://github.com/Sofreshx/Elegy",
  "skills": "./skills/",
  "capabilityCatalog": {
    "path": "./capability-catalog.json",
    "schemaVersion": "elegy-capability-catalog/v1",
    "readinessCommand": "elegy-example capabilities --detail"
  },
  "extensions": {
    "codex.plugin/v1": {
      "schemaVersion": "codex.plugin/v1",
      "interface": {
        "displayName": "Elegy Example",
        "shortDescription": "Run governed example workflows",
        "longDescription": "Describe the CLI-backed workflow and boundaries.",
        "developerName": "Elegy Contributors",
        "category": "Developer Tools",
        "capabilities": ["Read", "Write"],
        "defaultPrompt": ["Run the Elegy Example CLI for this task."]
      }
    }
  }
}
```

Distribution entry:

```json
{
  "name": "elegy-example",
  "kind": "cli",
  "packaging": "plugin",
  "pluginRoot": "plugins/example",
  "marketplaceCategory": "Developer Tools",
  "description": "Governed CLI behavior."
}
```

## External Or Private Marketplace Wrapper

Use this lane when the public repo owns marketplace metadata and the implementation lives in another repo or binary archive.

Layout:

```text
plugins/<plugin-name>/
  .elegy-plugin/plugin.json
  README.md
```

Manifest:

```json
{
  "schemaVersion": "elegy-plugin/v1",
  "name": "elegy-example",
  "version": "0.1.0",
  "description": "Public wrapper description.",
  "author": {"name": "Elegy Contributors"},
  "license": "Proprietary",
  "extensions": {
    "elegy.marketplace-wrapper/v1": {
      "schemaVersion": "elegy.marketplace-wrapper/v1",
      "sourceRepository": "https://github.com/example/private-or-external"
    },
    "codex.plugin/v1": {
      "schemaVersion": "codex.plugin/v1",
      "homepage": "https://github.com/example/private-or-external",
      "interface": {
        "displayName": "Example",
        "shortDescription": "Public wrapper for the shipped archive.",
        "longDescription": "State what the bundled artifact does without exposing secrets.",
        "developerName": "Elegy Contributors",
        "category": "Productivity",
        "capabilities": ["CLI"],
        "defaultPrompt": ["Use the bundled Example CLI."]
      }
    }
  }
}
```

Distribution entry:

```json
{
  "name": "elegy-example",
  "kind": "external-plugin",
  "packaging": "plugin",
  "pluginRoot": "plugins/example",
  "artifactBaseUrl": "https://github.com/Sofreshx/Elegy/releases/download",
  "marketplaceCategory": "Productivity",
  "description": "Public wrapper description."
}
```

## Downstream External Plugin Repo

Use this lane when the target repo owns the plugin source and release artifacts.

Minimum repo contract:

```text
.elegy-plugin/plugin.json
skills/ or SKILL.md when the plugin ships skills
src/ and tests/ when the plugin ships a runtime
release workflow that publishes <plugin-name>-plugin-<target>.zip
release workflow that publishes matching .sha256 sidecars
```

Required checks before publishing:

```bash
elegy-plugin-packaging verify --plugin .
elegy-plugin-packaging pack --plugin . --binary <compiled-file> --binary-name bin/<plugin-name>
```

If the plugin appears in the public Elegy marketplace, keep the wrapper manifest name and version aligned with the downstream archive manifest.
