# Elegy

Elegy is the main monorepo for governed formalization artifacts and the Rust runtime family that executes or composes those artifacts.

The design target is cross-project reuse with focused packages that stay:

- LLM-agnostic
- provider-agnostic
- framework-agnostic

The neutral authority roots under `contracts/` and `governance/` own governed schemas, fixtures, manifests, support metadata, and policy for the repository. Rust is the preferred implementation surface for self-contained, reusable, executable agentic/runtime capabilities where protocol, transport, host, and IO concerns dominate.

## Ecosystem position

- `Elegy` is the single main repository.
- governed contracts, schemas, fixtures, manifests, and support metadata live under `contracts/`, with policy and version governance under `governance/`.
- the first-party Rust runtime family lives under `rust/` and is the preferred implementation surface for shared executable capabilities such as CLI, host, policy execution, retrieval, memory, and behavior-heavy MCP runtime logic.
- standalone `Elegy-Skills` and `Elegy-CLI` repos should not be treated as the primary implementation surfaces.

See [docs/architecture/ecosystem-topology.md](docs/architecture/ecosystem-topology.md) for the current high-level organization and dependency direction.

## Project map

### Substrate

- `contracts/` - Authored neutral authority root for schemas, fixtures, manifests, and support metadata.
- `governance/` - Version policy, canonical output inventory, and boundary policy for the neutral authority surface.
- `schemas/` - Shared schema support material that survives independently of any removed implementation tree.
- `policies/` - Policy assets and formalization inputs that govern repository behavior.

### Rust Runtime Family

- `rust/` - in-repo Rust workspace for runtime composition, MCP host concerns, contract-consumer utilities, and shared executable behavior.

### Current self-authoring surface

The shipped self-authoring surface today is the Rust CLI flow for `author mcp`, `analyze mcp`, and `generate skills`, backed by `rust/crates/elegy-tooling`.

Built-in MCP-native or skill-driven self-authoring remains the next milestone rather than a shipped repo claim.

### Operational support

- `scripts/` - export, validation, and release support scripts for the governed bundle.
- `artifacts/` - generated outputs and distribution artifacts.
- `docs/` - architecture, migration, distribution, and governance documentation.

## Burden-of-Proof Reset

Elegy now follows a stricter shared-code rule:

- keep shared code only when it proves either canonical authority value or reusable runtime value
- prefer schemas, fixtures, policy artifacts, and docs over libraries when consumers mainly need to read, validate, or conform
- keep host-specific runtime behavior, auth, persistence, UI orchestration, HTTP endpoints, and composition-root logic in consuming repos

Under that rule, the current durable authority center is intentionally small:

- `contracts/`
- `governance/`
- `rust/`

## Organization rules

- shared substrate assets must not depend on provider-specific SDKs, app frameworks, or host-specific runtime glue
- governed schemas, fixtures, manifests, and support metadata belong under `contracts/`, with policy and version governance under `governance/`
- behavior-heavy host, transport, filesystem, HTTP, execution, retrieval, and projection logic should live in the in-repo Rust runtime family when shared implementation is justified
- the exported contract bundle is the machine-readable handshake between the neutral authority roots and first-party Rust runtime crates, plus any external consumers
- new shared executable features should default to Rust unless they can be expressed as neutral artifacts instead of code
- downstream consumers should integrate through exported artifacts or thin adapters rather than using Elegy as a catch-all runtime host

## Consolidation posture

The current reset direction is:

- keep neutral artifacts authoritative for governed contracts, schemas, fixtures, compatibility manifests, and support metadata during the purge
- keep the Rust runtime family in `rust/` as the shared executable surface for CLI, host, runtime composition, policy execution, retrieval, memory, and behavior-heavy MCP logic
- keep product-hosted runtime logic in downstream consumers unless the capability is demonstrably reusable and host-agnostic
- treat the removed legacy package tree as retired, not as a compatibility surface to preserve

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

Elegy should be consumed through versioned exported artifacts rather than local sibling-repository references or repo-internal package restore assumptions.

- the neutral governed bundle is exported from `contracts/` and versioned by `governance/version-policy.json`.
- contract schemas, fixtures, and compatibility metadata can be exported as a versioned bundle with `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
- downstream consumer guidance lives in [docs/distribution.md](docs/distribution.md).

## Release and versioning

Elegy uses SemVer for both bundle versions and schema versions during the purge transition.

- Bundle and manifest package version source of truth: `governance/version-policy.json`.
- Active schema version source of truth: `governance/version-policy.json`.

### Compatibility expectations

- **Package major**: breaking API/runtime contract changes.
- **Package minor**: backward-compatible feature additions.
- **Package patch**: backward-compatible fixes.
- **Schema major**: breaking schema changes for serialized/governed artifacts.
- **Schema minor/patch**: non-breaking schema additions/fixes.

### Governance rule

If `schemaVersion` major is incremented, the bundle major version in `governance/version-policy.json` must also be incremented in the same change.

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

## Contracts artifacts for consumers

Export publishable contracts artifacts (schema + fixtures + compatibility manifest):

```powershell
pwsh ./scripts/export-contracts.ps1
```

This produces deterministic files under `artifacts/contracts` for downstream consumers such as the in-repo Rust workspace under `rust/` and any external integrations:

- `canonical-workflow.schema.json`
- `canonical-workflow-graph.schema.json`
- `skill-definition.schema.json`
- `skill-discovery-index.schema.json`
- `mcp-tool-definition.schema.json`
- `mcp-server-descriptor.schema.json`
- `mcp-analysis-result.schema.json`
- `agent-request-envelope.schema.json`
- `agent-response-envelope.schema.json`
- `agent-event-envelope.schema.json`
- `fixtures/canonical-workflow.minimal.json`
- `fixtures/canonical-workflow-graph.minimal.json`
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
