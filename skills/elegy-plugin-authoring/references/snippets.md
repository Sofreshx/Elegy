# Adapter plugin authoring snippets

These snippets apply only after the eligibility gate identifies a reusable
data-source, database, platform, API, local-system, or executable boundary.

## Active adapter layout

```text
plugins/<adapter-name>/
  .elegy-plugin/plugin.json
  capability-catalog.json
  readiness.json
  evidence/
  skills/                       # optional workflow guidance
  .mcp.json or mcp-servers/     # optional host projection
  src/ and tests/               # when implemented in this repository
```

## Portable manifest

```json
{
  "schemaVersion": "elegy-plugin/v3",
  "name": "elegy-example-adapter",
  "version": "0.1.0",
  "description": "Adapt a concrete external system for typed agent use.",
  "skills": "./skills/",
  "elegy": {
    "surfaceClass": "adapter-plugin",
    "capabilityCatalog": {
      "path": "./capability-catalog.json",
      "schemaVersion": "elegy-capability-catalog/v1",
      "readinessCommand": "elegy-example-adapter status --json"
    },
    "connections": {
      "requirements": {
        "mode": "none"
      }
    },
    "readiness": {
      "stage": "implemented",
      "path": "./readiness.json",
      "schemaVersion": "elegy-readiness/v1"
    },
    "mcpAuthentication": {}
  }
}
```

Add `skills` only when workflow guidance materially helps an agent use the
typed operations. Add MCP only when a host requires that protocol. Neither
replaces the capability catalog or CLI behavior.

## Surface registration

```json
{
  "name": "elegy-example-adapter",
  "kind": "bundled-plugin",
  "surfaceClass": "adapter-plugin",
  "lifecycle": "active",
  "package": "elegy-example-adapter",
  "crateRoot": "plugins/example-adapter",
  "packaging": "plugin",
  "pluginRoot": "plugins/example-adapter",
  "marketplaceCategory": "Developer Tools",
  "disposition": "Retained as a reusable adapter to the example system.",
  "description": "Typed adapter for the example system."
}
```

If the implementation is not yet package-complete, use `lifecycle: rework`,
remove `packaging`, and reference readiness directly from the surface entry.
Do not publish an aspirational manifest.

## Standalone skill

A repeatable instruction workflow belongs under `skills/<name>/SKILL.md`. It
uses `surfaceClass: skill`, has no `packaging: plugin`, and is installed through
the target host's native skill lane.

## Domain library or tool

Scoring, analysis, reports, business rules, workflow state, and product
orchestration use `surfaceClass: tool` or remain a product library. Do not add
an Elegy plugin manifest. If the product later exposes a reusable external
adapter, package that boundary separately.

## Validation

```bash
elegy-plugin-packaging verify --plugin plugins/<adapter-name>
elegy-plugin-packaging pack \
  --plugin plugins/<adapter-name> \
  --binary <compiled-file> \
  --binary-name bin/<adapter-name>
cargo run -p elegy-documentation -- check --project .
```
