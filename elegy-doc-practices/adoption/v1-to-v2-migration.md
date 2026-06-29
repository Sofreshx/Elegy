# Adoption: V1-compat `.elegy/docs.yaml` for legacy consumers

If a downstream repo already uses the V1 schema, keep the legacy fields
flat. The V1-compat loader remains supported:

```yaml
schemaVersion: elegy-docs/v1
adrPath: docs/adr
specPath: docs/specs
indexPath: docs/docs-index.md
requiredDocTriggers:
  - architecture-change
  - durable-decision
  - behavior-change
  - cross-repo-impact
  - onboarding-change
localExceptions: []
```

## When to migrate to V2

Migrate to V2 when the downstream repo needs:

- per-class freshness warnings (current vs planning vs research)
- derived-surface drift detection (llms.txt, bundle.json)
- entrypoint validation
- richer `authorityRoots` classification

## Migration command

```bash
elegy-documentation init --project . --json
```

The init command writes a V2 config with sensible defaults while preserving
the existing V1 path configuration where possible.
