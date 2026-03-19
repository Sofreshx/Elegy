# Elegy

Elegy is the main monorepo for reusable formalization, governance, MCP-facing analysis, skill definitions, generation-oriented building blocks, and the Rust runtime family that executes or composes those governed artifacts.

The design target is cross-project reuse with focused packages that stay:

- LLM-agnostic
- provider-agnostic
- framework-agnostic

The .NET solution remains the contract and formalization authority. Runtime-oriented MCP behavior lives in the in-repo Rust workspace where protocol, transport, host, and IO concerns dominate.

## Ecosystem position

- `Elegy` is the single main repository.
- governed contracts, schemas, fixtures, and canonical skill models remain authoritative in the .NET package families under `src/`.
- the first-party Rust runtime family lives under `rust/` and is the preferred implementation surface for behavior-heavy MCP runtime logic.
- standalone `Elegy-Skills` and `Elegy-CLI` repos should not be treated as the primary implementation surfaces.

See [docs/architecture/ecosystem-topology.md](docs/architecture/ecosystem-topology.md) for the current high-level organization and dependency direction.

## Project map

### Substrate

- `src/Elegy.Formalization.Core` - Core abstractions and domain model primitives.
- `src/Elegy.Formalization.Contracts` - Shared contracts for integration boundaries.
- `src/Elegy.Formalization.Serialization` - Serialization support over core formalization models.
- `src/Elegy.Formalization.Validation` - Validation utilities and rules for formalization artifacts.
- `src/Elegy.Formalization.Governance` - Governance policies and enforcement helpers.
- `src/Elegy.Formalization.Projections.Mermaid` - Mermaid projection output from core formalization structures.

### Skills and generation

- `src/Elegy.Formalization.Skills` - Core skill definitions and lifecycle metadata.
- `src/Elegy.Formalization.Skills.Discovery` - Discovery-oriented skill surfaces.
- `src/Elegy.Formalization.DynamicSkills` - Dynamic materialization and runtime-oriented skill helpers.
- `src/Elegy.Formalization.SkillForge` - Skill and tooling generation/materialization flows.

### MCP and adjacent runtime-facing analysis

- `src/Elegy.Formalization.Mcp` - MCP descriptors, governed analysis artifacts, and canonical MCP-to-skill projection rules.

### Rust Runtime Family

- `rust/` - in-repo Rust workspace for runtime composition, MCP host concerns, contract-consumer utilities, and future Rust replacements for behavior-heavy .NET MCP logic.

### Additional families

- `src/Elegy.Formalization.Agents` - Agent-facing formalization primitives.
- `src/Elegy.Formalization.AgentFactory` - Agent creation/build helpers.
- `src/Elegy.Formalization.Monitoring` - Monitoring-oriented formalization surfaces.
- `tests/*` - Unit test projects aligned to source packages.

## Organization rules

- shared substrate packages must not depend on provider-specific SDKs, app frameworks, or host-specific runtime glue
- MCP formalization in `src/` remains the schema and canonical projection authority, but behavior-heavy host, transport, filesystem, HTTP, and execution logic should move to the in-repo Rust runtime family when Rust is the stronger implementation fit
- the exported contract bundle is the machine-readable handshake between authoritative .NET contract families and first-party Rust runtime crates, plus any external consumers
- generation/tooling concerns belong in `SkillForge`-style packages rather than being conflated with the human-facing CLI concept
- if a package family later needs its own repository, split only after the package boundary is proven and at least two real consumers exist

## Consolidation posture

The current consolidation direction is:

- keep .NET authoritative for governed contracts, schemas, fixtures, compatibility manifests, and canonical skill definitions
- keep useful Rust MCP implementation layers in `rust/` instead of treating a sibling repo as the long-term default topology
- replace .NET with Rust where runtime, protocol, host, transport, or filesystem behavior is the dominant concern
- start that replacement work with the existing .NET MCP analyzer, generator, and discovery behavior rather than with broader package families such as DynamicSkills or SkillForge

## Documentation and operational posture

`Elegy` is the authoritative public entrypoint for contributor and governance posture.

- [Contributing guide](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)
- [Architecture docs](docs/architecture/README.md)
- [MCP spec baseline](docs/spec-baseline.md)

Historical sibling repositories such as `Elegy-MCP`, `Elegy-CLI`, and `Elegy-Skills` should be treated as archival or closeout references rather than the primary source of truth.

## Distribution and consumption

Elegy should be consumed through versioned packages and versioned exported artifacts rather than local sibling-repository references.

- .NET package distribution is prepared for GitHub Packages.
- contract schemas, fixtures, and compatibility metadata can be exported as a versioned bundle with `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
- downstream consumer guidance lives in [docs/distribution.md](docs/distribution.md).

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

This produces deterministic files under `artifacts/contracts` for downstream consumers such as Node.js tools, the in-repo Rust workspace under `rust/`, and any external integrations:

- `canonical-workflow.schema.json`
- `skill-definition.schema.json`
- `skill-discovery-index.schema.json`
- `mcp-tool-definition.schema.json`
- `mcp-server-descriptor.schema.json`
- `mcp-analysis-result.schema.json`
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
