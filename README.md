# Elegy

Elegy provides formalization extraction building blocks with focused packages for core models, serialization, validation, governance, contracts, and projections.

## Project map

- `src/Elegy.Formalization.Core` - Core abstractions and domain model primitives.
- `src/Elegy.Formalization.Serialization` - Serialization support over core formalization models.
- `src/Elegy.Formalization.Validation` - Validation utilities and rules for formalization artifacts.
- `src/Elegy.Formalization.Projections.Mermaid` - Mermaid projection output from core formalization structures.
- `src/Elegy.Formalization.Governance` - Governance policies and enforcement helpers.
- `src/Elegy.Formalization.Contracts` - Shared contracts for integration boundaries.
- `tests/*` - Unit test projects aligned to source packages.

## Release and versioning

Elegy uses SemVer for both package versions and schema versions.

- Package version source of truth: `Directory.Build.props` (`VersionPrefix`).
- Schema version source of truth: `schemas/schema-version.json` (`schemaVersion`).

### Compatibility expectations

- **Package major**: breaking API/runtime contract changes.
- **Package minor**: backward-compatible feature additions.
- **Package patch**: backward-compatible fixes.
- **Schema major**: breaking schema changes for serialized/governed artifacts.
- **Schema minor/patch**: non-breaking schema additions/fixes.

### Governance rule

If `schemaVersion` major is incremented, the package major version in `Directory.Build.props` must also be incremented in the same change.

This is enforced by CI in `.github/workflows/versioning-governance.yml`.

### Version bumping

Use the helper script:

```powershell
pwsh ./scripts/bump-version.ps1 -DryRun
pwsh ./scripts/bump-version.ps1 -PackageBump minor
pwsh ./scripts/bump-version.ps1 -PackageVersion 2.0.0 -SchemaBump major
pwsh ./scripts/bump-version.ps1 -PackageBump patch -SchemaVersion 1.2.3
```

The script validates SemVer input and blocks schema-major bumps unless package major is also increased.

## Contracts artifacts for Node.js consumers

Export publishable contracts artifacts (schema + fixtures + compatibility manifest):

```powershell
pwsh ./scripts/export-contracts.ps1
```

This produces deterministic files under `artifacts/contracts`:

- `canonical-workflow.schema.json`
- `fixtures/canonical-workflow.minimal.json`
- `compatibility-manifest.json`
- `compatibility-matrix.json`

### Compatibility matrix for consumers

`compatibility-matrix.json` is the canonical machine-readable source for cross-repo support expectations between:

- Holon version ranges
- instruction-engine version ranges
- Elegy package version ranges
- Elegy schema version ranges
- Optional policy schema version ranges

Consumers should evaluate entries in order and choose the first matching tuple for their effective versions. If no entry matches, treat the integration as `unsupported`.

Use the schema from Node.js (example with `ajv`):

```bash
npm install ajv
```

```js
const fs = require('node:fs');
const path = require('node:path');
const Ajv = require('ajv');

const contractsRoot = path.resolve(__dirname, '..', 'artifacts', 'contracts');
const schema = JSON.parse(fs.readFileSync(path.join(contractsRoot, 'canonical-workflow.schema.json'), 'utf8'));
const fixture = JSON.parse(fs.readFileSync(path.join(contractsRoot, 'fixtures', 'canonical-workflow.minimal.json'), 'utf8'));

const ajv = new Ajv({ strict: true });
const validate = ajv.compile(schema);
const isValid = validate(fixture);

if (!isValid) {
  console.error(validate.errors);
  process.exit(1);
}
```
