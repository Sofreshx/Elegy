# Elegy

Elegy is the main monorepo for governed contract and policy artifacts plus the Rust runtime family that executes or composes those artifacts.

The design target is cross-project reuse with focused reusable surfaces that stay:

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

### Wrapper entrypoints

- `src/Elegy-memory/` - thin wrapper and integration entrypoint for the shipped in-repo `elegy-memory` surface; not a repo center, authority layer, implementation center, or release surface.
- `src/Elegy-mcp/` - thin wrapper and integration entrypoint for the current dedicated in-repo `elegy-mcp` surface; not a repo center, authority layer, implementation center, or release surface.
- `src/Elegy-skills/` - thin wrapper and integration entrypoint for the current dedicated in-repo `elegy-skills` surface; not a repo center, authority layer, implementation center, or release surface.

Each wrapper root carries a `wrapper-entrypoint.json` asset plus helper-lane guidance under `docs/`, `agents/`, and `skills/`. Those lanes are for external-agent integration guidance and contributor routing only; they do not create an in-repo agent runtime or orchestration lane. The dedicated wrapper roots now also carry `install.ps1` plus a surface-local `skills/<surface>/SKILL.md` bridge so the wrapper archive lane stays actionable. Authority remains in `contracts/`, `governance/`, `rust/`, `.github/skills/`, and the canonical docs.

### Current direct system surfaces

The current CLI surfaces are:

- `elegy-memory` for the dedicated bounded memory surface
- `elegy-mcp` for the dedicated MCP descriptor authoring and analysis surface
- `elegy-skills` for the dedicated MCP-to-skill generation surface
- `elegy` as the umbrella general/compatibility surface

When an external agent outside Elegy needs one of these dedicated systems, it should load the associated skill material and invoke the matching `elegy-*` CLI directly. Elegy itself is not the place where agents are orchestrated or called internally.

### Mermaid tooling

The umbrella `elegy` CLI exposes Mermaid tooling as a derived projection surface:

- `elegy mermaid render` projects governed `canonical-workflow` or `canonical-workflow-graph` JSON into Mermaid `flowchart TD`
- `elegy mermaid reverse` projects Mermaid `flowchart TD` output compatible with the current renderer into a bounded workflow-graph-semantics report
- `elegy mermaid narrate` emits a concise derived narrative from either canonical workflow JSON or Mermaid input

Mermaid remains explicitly non-authoritative. Reverse output is not full canonical workflow reconstruction. See [docs/architecture/mermaid-tooling.md](docs/architecture/mermaid-tooling.md) for the current scope boundaries.

Release workflows and distribution support now exist for archives of the umbrella `elegy` CLI plus the dedicated `elegy-memory`, `elegy-mcp`, and `elegy-skills` binaries.

### Current self-authoring surface

The current implemented authoring slice is the Rust flow for MCP descriptor authoring, MCP descriptor analysis, and MCP-to-skill generation.

Those flows are exposed through the dedicated `elegy-mcp` and `elegy-skills` binaries, while `elegy` remains the compatibility/general surface. Shared crates such as `rust/crates/elegy-tooling` support descriptor and skill workflows as lower-level helper and compatibility infrastructure; they are not the preferred direct system surface for external consumers of `elegy-skills`.

Built-in MCP-native or skill-driven self-authoring remains the next milestone rather than a completed repo claim.

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
- [Elegy-memory V1](docs/architecture/elegy-memory-v1.md)
- [MCP spec baseline](docs/spec-baseline.md)

The wrapper entrypoints under `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are thin integration surfaces only. They do not replace `contracts/`, `governance/`, or `rust/` as the canonical owned surfaces, and `.github/skills/` remains only a repo-local non-authoritative contributor-routing surface. External agents should use those routing surfaces to find the dedicated CLI handoff, not to infer that Elegy runs an internal agent orchestration layer.

Historical sibling repositories such as `Elegy-MCP`, `Elegy-CLI`, and `Elegy-Skills` should be treated as archival or closeout references rather than the primary source of truth.

## Distribution and consumption

Elegy should be consumed through versioned GitHub release assets rather than local sibling-repository references, repo-internal package restore assumptions, or new package-feed distribution lanes.

- the neutral governed bundle is exported from `contracts/` and versioned by `governance/version-policy.json`.
- contract schemas, fixtures, and compatibility metadata can be exported as a versioned bundle with `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
- the `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills` binaries are the CLI surfaces covered by the explicit release/archive workflows for `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, and `aarch64-apple-darwin`.
- `rust/crates/elegy-memory`, `rust/crates/elegy-mcp`, and `rust/crates/elegy-skills` own the dedicated binaries for their bounded direct surfaces; `rust/crates/elegy-cli` keeps the umbrella general/compatibility surface.
- `rust/crates/elegy-tooling` remains shared lower-level helper and compatibility infrastructure for descriptor and skill workflows rather than a dedicated end-user system surface.
- tagged GitHub releases are the primary downstream lane; the standalone installer asset is only a convenience bootstrap around the same release assets.
- downstream consumers can use `pwsh ./scripts/install-distribution.ps1 -Tag <releaseTag> -Destination <path> -CliSurfaces <surface[,surface...]> -WrapperSurfaces <surface[,surface...]>` to fetch the contracts bundle plus matching CLI archives and wrapper archives without sibling checkouts or package feeds.
- wrapper archives already embed `scripts/install-distribution.ps1`, so dedicated-wrapper consumers do not need the standalone installer asset unless they want the generic bootstrap path.
- Holon and other downstream repos should pin an Elegy release tag and install into a repo-local tools directory such as `./tools/elegy`.
- historical GitHub Packages and NuGet publication surfaces are frozen/deprecated; keep them retired and only remove remaining metadata after consumer cutover evidence exists.
- downstream consumer guidance lives in [docs/distribution.md](docs/distribution.md).

## Release and versioning

Elegy uses SemVer for both bundle versions and schema versions under the current file-native version-governance model.

- Bundle version and legacy compatibility manifest metadata source of truth: `governance/version-policy.json`.
- Active schema version source of truth: `governance/version-policy.json`.

### Compatibility expectations

- **Bundle major**: breaking API/runtime contract changes.
- **Bundle minor**: backward-compatible feature additions.
- **Bundle patch**: backward-compatible fixes.
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

The script validates SemVer input and blocks schema-major bumps unless bundle major is also increased. Its `Package*` parameter names are retained for CLI compatibility.

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
- `fixtures/skill-definition.elegy-memory.json`
- `fixtures/skill-definition.elegy-mcp.json`
- `fixtures/skill-definition.elegy-skills.json`
- `fixtures/skill-definition.elegy-mermaid.json`
- `fixtures/skill-discovery-index.elegy-memory.json`
- `fixtures/skill-discovery-index.elegy-mcp.json`
- `fixtures/skill-discovery-index.elegy-skills.json`
- `fixtures/skill-discovery-index.elegy-mermaid.json`
- `compatibility-manifest.json`
- `compatibility-matrix.json`

The repo also materializes `.github/skills/elegy-memory/SKILL.md`, `.github/skills/elegy-mcp/SKILL.md`, `.github/skills/elegy-skills/SKILL.md`, and `.github/skills/elegy-mermaid/SKILL.md` for contributor routing over the current dedicated surfaces plus the umbrella Mermaid tooling surface. External agents outside Elegy can load those routing files to discover the correct dedicated CLI handoff, but those markdown files remain non-authoritative. The authority chain remains governed skill definition fixture -> governed skill discovery projection -> repo-local non-authoritative contributor-routing output.

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
