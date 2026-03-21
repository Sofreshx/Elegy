# Distribution and downstream consumption

Elegy is intended to be consumed through versioned exported artifacts, not through brittle sibling-repository workspace references or package-feed distribution.

The active authority root is `contracts/`, with version policy under `governance/version-policy.json`.

## Contract bundles

Contract schemas, fixtures, compatibility metadata, and parity fixtures are exported with:

```powershell
pwsh ./scripts/export-contracts.ps1 -CreateArchive
```

Outputs:

- expanded directory: `artifacts/contracts`
- versioned archive: `artifacts/distribution/elegy-contracts-<bundleVersion>.zip`

The archive is intended for downstream consumers including Rust consumers, integration environments, and any external tooling that only needs the governed contract bundle.

Current governed workflow artifacts in that bundle include both the portable workflow contract and the canonical workflow graph contract:

- `canonical-workflow.schema.json`
- `canonical-workflow-graph.schema.json`
- `fixtures/canonical-workflow.minimal.json`
- `fixtures/canonical-workflow-graph.minimal.json`

## Downstream guidance

- Prefer release assets or workflow artifacts for schema/fixture bundles.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Treat `artifacts/contracts` and release bundle contents as the supported machine-readable handoff surface.

## Maintainer flow

1. Update bundle and manifest package metadata/version in `governance/version-policy.json`.
2. Run `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
3. Run `pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs -RequireArchive`.
4. Publish the generated bundle through the GitHub Actions workflows when ready.
