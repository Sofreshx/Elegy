# Distribution and downstream consumption

Elegy is intended to be consumed through versioned release assets, not through sibling-repository workspace references or package-feed distribution.

The active authority root is `contracts/`. The current in-repo CLI surfaces are the general `elegy` CLI plus the dedicated `elegy-memory`, `elegy-mcp`, `elegy-planning`, `elegy-skills`, `elegy-configuration`, and `elegy-documentation` binaries built from the in-repo Rust workspace. Tagged release workflows publish archives for those surfaces plus the six dedicated wrapper archives, and pushes to `main` refresh a rolling `main-snapshot` prerelease with the same asset set for latest-integration validation.

The bounded local memory operator lives in `rust/crates/elegy-memory` and exposes the `elegy-memory` binary. `rust/crates/elegy-mcp`, `rust/crates/elegy-planning`, and `rust/crates/elegy-skills` now expose their own dedicated binaries for descriptor authoring/analysis, durable planning authority, and governed skill-registry access/validation. Lower-level MCP-to-skill generation remains on the shared `elegy` CLI and tooling path. The shared `elegy` CLI remains the general and compatibility surface.

Mermaid tooling stays on that umbrella surface. `elegy mermaid render`, `elegy mermaid reverse`, and `elegy mermaid narrate` do not introduce a dedicated Mermaid binary, wrapper archive, or separate distribution lane.

## Stable vs prerelease

Elegy uses two distribution channels:

- Stable semver tags such as `v1.3.2`. These are the packages downstream consumers should pin.
- Rolling prerelease `main-snapshot`. This is refreshed on every push to `main` and is intended for validation, debugging, and latest-branch integration checks.

Those two channels publish the same asset families. The difference is lifecycle and stability promise, not package coverage.

## What most consumers should download

Most users do not need every asset in the release:

- If you need schemas, fixtures, or compatibility metadata, download the contracts bundle.
- If you want to run Elegy commands, download only the CLI archives you need.
- If you want a scripted installation path, use the installer bootstrap.
- If you want a bounded repo-local integration surface for one dedicated tool family, use a wrapper archive.
- The manifest and checksums assets are primarily installer/maintainer assets and usually do not need manual handling.

## Asset model

### Which Download To Use

| If you want... | Download |
| --- | --- |
| Simplest verified install path | `elegy-installer-<bundleVersion>.zip` |
| General-purpose `elegy` CLI | `elegy-cli-<cliVersion>-<target>.zip` |
| Contracts only | `elegy-contracts-<bundleVersion>.zip` |
| Dedicated memory CLI | `elegy-memory-<cliVersion>-<target>.zip` |
| Dedicated MCP CLI | `elegy-mcp-<cliVersion>-<target>.zip` |
| Dedicated planning CLI | `elegy-planning-<cliVersion>-<target>.zip` |
| Dedicated skill registry CLI | `elegy-skills-<cliVersion>-<target>.zip` |
| Dedicated documentation CLI | `elegy-documentation-<cliVersion>-<target>.zip` |
| Wrapper surface for a dedicated tool family | `elegy-*-wrapper-<bundleVersion>.zip` |

Direct release asset families include:

- `elegy-cli-<cliVersion>-<target>.zip` - umbrella `elegy` binary
- `elegy-memory-<cliVersion>-<target>.zip` - dedicated local memory CLI
- `elegy-mcp-<cliVersion>-<target>.zip` - dedicated MCP CLI
- `elegy-planning-<cliVersion>-<target>.zip` - dedicated planning CLI
- `elegy-skills-<cliVersion>-<target>.zip` - dedicated skill registry CLI
- `elegy-documentation-<cliVersion>-<target>.zip` - dedicated documentation authority CLI
- `elegy-contracts-<bundleVersion>.zip` - governed contracts bundle
- `elegy-installer-<bundleVersion>.zip` - installer bootstrap
- `elegy-*-wrapper-<bundleVersion>.zip` - dedicated wrapper surfaces

Tagged releases are configured to publish neutral asset families across the contracts, installer, metadata, CLI, and dedicated wrapper lanes:

- governed contracts bundle: `elegy-contracts-<bundleVersion>.zip`
- standalone installer bootstrap: `elegy-installer-<bundleVersion>.zip`
- release manifest: `elegy-release-manifest-<bundleVersion>.json`
- release checksums: `elegy-release-checksums-<bundleVersion>.json`
- umbrella CLI archive: `elegy-cli-<cliVersion>-<target>.zip`
- local memory CLI archive: `elegy-memory-<cliVersion>-<target>.zip`
- MCP CLI archive: `elegy-mcp-<cliVersion>-<target>.zip`
- planning CLI archive: `elegy-planning-<cliVersion>-<target>.zip`
- skills CLI archive: `elegy-skills-<cliVersion>-<target>.zip`
- configuration CLI archive: `elegy-configuration-<cliVersion>-<target>.zip`
- documentation CLI archive: `elegy-documentation-<cliVersion>-<target>.zip`
- local memory wrapper archive: `elegy-memory-wrapper-<bundleVersion>.zip`
- MCP wrapper archive: `elegy-mcp-wrapper-<bundleVersion>.zip`
- planning wrapper archive: `elegy-planning-wrapper-<bundleVersion>.zip`
- skills wrapper archive: `elegy-skills-wrapper-<bundleVersion>.zip`
- configuration wrapper archive: `elegy-configuration-wrapper-<bundleVersion>.zip`
- documentation wrapper archive: `elegy-documentation-wrapper-<bundleVersion>.zip`

The contracts bundle remains the canonical machine-readable handoff for schemas, fixtures, compatibility metadata, and parity fixtures.

GitHub Releases are the primary downstream distribution lane. The standalone installer archive is a convenience bootstrap that carries the generic install helper only; it does not introduce a separate package-feed or runtime distribution path. Stable downstream consumption should continue to use explicit semver tags such as `v1.3.2`, while the rolling `main-snapshot` prerelease exists only as a continuously refreshed integration build.

The repo intentionally keeps CI artifact assembly and GitHub Release publication as separate workflows. `.github/workflows/distribution-artifacts.yml` builds and uploads Actions artifacts for local and PR validation, while `.github/workflows/publish-distribution.yml` is the hosted publication lane that refreshes the downloadable `main-snapshot` prerelease on pushes to `main` and publishes matching release assets for tags and release events.

Downloadable archives are self-describing. Packaging stages `PACKAGE_README.md` into every downloadable zip as archive-root `README.md`, and manifest validation treats that README as a required payload entry for the CLI, wrapper, and installer archive families.

Each CLI archive is a thin distribution of its corresponding executable plus archive-root `README.md` for one explicitly published host target. The umbrella `elegy-cli-<cliVersion>-<target>.zip` archive specifically carries the `elegy` binary and is the downloadable general-purpose surface for agent onboarding, skills compatibility, docs tooling, Mermaid and diagram tooling, repo/web/data/notify utilities, read-only observation and desktop automation, optional MCP hosting, contracts export, deterministic configuration materialization, and lower-level `author`/`analyze`/`generate`/`validate`/`inspect` commands. The current published target set is intentionally narrow:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

The installer only resolves those exact release targets and fails clearly on unsupported host architectures.

Bundle version and CLI version are intentionally independent. Consumers should resolve both assets from the same release tag rather than assuming `bundleVersion == cliVersion`.

## Release metadata

Each published distribution set now includes two metadata documents in `artifacts/distribution`:

- `elegy-release-manifest-<bundleVersion>.json`
- `elegy-release-checksums-<bundleVersion>.json`

The manifest is the installer authority for distribution contents. It records the repository, tag marker, bundle version, generated timestamp, published targets, and every published asset with file name, asset kind, surface, target, version, size, SHA-256, and required archive entries.

The checksums document carries the release or local-artifacts tag marker plus the SHA-256 digest for every published asset and for the manifest itself. The installer resolves those two JSON assets first and fails closed if they are missing, duplicated, inconsistent, or do not match the downloaded payloads.

On pushes to `main`, the publish workflow writes the same metadata pair with tag marker `main-snapshot` and replaces the rolling `main-snapshot` prerelease so maintainers always have a fresh release-backed artifact set for the current branch head. On semver tags and published GitHub releases, the workflow writes the metadata with the semver release tag.

Local artifact installs use the same metadata lane. After building or copying local release assets into a staging directory, generate the metadata pair before invoking the installer:

```powershell
# (distribution packaging not yet implemented)
```

## Standalone installer archive

Build the standalone installer bootstrap asset with:

```powershell
pwsh ./scripts/package-installer.ps1
```

Output:

- versioned archive: `artifacts/distribution/elegy-installer-<bundleVersion>.zip`

The standalone installer archive contains `install-distribution.ps1` and `README.md` at the archive root so downstream repos can download a single GitHub release asset, extract it into a repo-local tools/bootstrap directory, and then fetch the contracts bundle plus the selected CLI and wrapper archives through the supported installer path.

The generic installer now requires the manifest and checksums JSON assets alongside the zip assets. It verifies exact asset presence, file size, SHA-256, and required archive entries before extraction, then writes `install-receipt.json` into the destination root with the request, source, host target, installed assets, and verification evidence.

## Contracts bundle

Contract schemas, fixtures, compatibility metadata, and parity fixtures are exported with:

```powershell
pwsh ./scripts/export-contracts.ps1 -CreateArchive
```

Outputs:

- expanded directory: `artifacts/contracts`
- versioned archive: `artifacts/distribution/elegy-contracts-<bundleVersion>.zip`

Current governed workflow artifacts in that bundle include both the portable workflow contract and the canonical workflow graph contract:

- `canonical-workflow.schema.json`
- `canonical-workflow-graph.schema.json`
- `fixtures/canonical-workflow.minimal.json`
- `fixtures/canonical-workflow-graph.minimal.json`

Current governed package artifacts in that bundle include the portable plugin
package contracts. These are metadata and validation support for consuming hosts,
plus conservative derived projection tooling support, not an Elegy plugin runtime:

- `elegy-plugin-package.schema.json`
- `fixtures/elegy-plugin-package.minimal.json`
- `fixtures/elegy-plugin-package.demo-config.json`

Current governed dedicated-surface skill artifacts in that bundle include:

- `fixtures/skill.elegy-memory.json`
- `fixtures/skill-discovery-index.elegy-memory.json`
- `fixtures/skill.elegy-mcp.json`
- `fixtures/skill-discovery-index.elegy-mcp.json`
- `fixtures/skill.elegy-planning.json`
- `fixtures/skill.elegy-skills.json`
- `fixtures/skill-discovery-index.elegy-skills.json`
- `fixtures/skill.elegy-documentation.json`
- `fixtures/skill-discovery-index.elegy-documentation.json`
- `fixtures/skill.elegy-mermaid.json`
- `fixtures/skill-discovery-index.elegy-mermaid.json`

The `.agents/skills/` and `.github/skills/` mirror paths are retired. Plugin packages are the authority, and host-specific exports are generated via `elegy plugin export`.

The current lower-level contributor tooling also includes `elegy generate codex-plugin` (legacy alias for `elegy plugin export codex`), which projects a portable package into a conservative local Codex plugin folder containing `.codex-plugin/plugin.json` and `skills/`. That generated plugin folder is a derived local output and is not currently a release asset family.

## CLI archive

Build and package a current-host CLI surface with:

```powershell
# (distribution packaging not yet implemented)
```

Output:

- versioned archive: `artifacts/distribution/<surface>-<cliVersion>-<target>.zip`

# (distribution packaging not yet implemented)

Each archive contains only its corresponding executable plus archive-root `README.md`. These archives do not add host bootstrap logic, consumer config, or downstream runtime wiring.

For Mermaid tooling, use the umbrella `elegy` archive. The same umbrella archive is also the downloadable surface for agent onboarding, skills compatibility, docs tooling, diagram, `run`, observe, desktop, repo, web, data, notify, contracts export, `configuration`, and lower-level `author`/`analyze`/`generate`/`validate`/`inspect`; those commands remain general-surface commands under the existing `elegy` executable rather than dedicated release targets.

## Configuration materialization

The umbrella CLI carries `elegy configuration list|show|apply|verify` and the
dedicated `elegy-configuration` archive carries `elegy-configuration
list|show|apply|verify` for deterministic materialization and drift
verification of repo-local or home-level agentic assets from governed templates
and profiles.

This is a post-install or from-source operator lane, not a new distribution
model:

- distribution install still owns release-tag selection, asset download,
  checksum verification, and archive extraction
- `elegy configuration ...` and `elegy-configuration ...` own deterministic
  file materialization and drift verification for declared assets such as skill
  mirrors, instruction blocks, MCP config, hooks, agents, and bounded text,
  JSON, or TOML patches
- local `elegy-plugin-package/v1` files may carry governed configuration
  templates and profiles for package-backed apply/verify flows
- consuming repos still own product-specific bootstrap, runtime auth/state,
  approvals, orchestration, and any host-local startup wiring

Current built-in templates and profiles live under `contracts/configuration/`.
The current built-ins intentionally stay small: repo skill mirroring,
repo-local OpenCode materialization, repo-local Codex skill mirroring, and a
bounded Codex home template.

Examples:

```bash
elegy configuration list --json
elegy configuration show --template-id repo-opencode-agentic-minimal --json
elegy configuration apply --profile-id repo-opencode-minimal --target . --dry-run --json
elegy configuration verify --profile-id repo-opencode-minimal --target . --json
elegy-configuration apply --package ./contracts/fixtures/elegy-plugin-package.demo-config.json --profile-id demo-profile --target . --dry-run --json
```

Binding defaults are template-local and overrideable. The `repo-skill-mirror-minimal` template has been removed; skill delivery should use plugin packages and host export instead.

## Wrapper archive

Build the platform-neutral wrapper archives with:

```powershell
# (distribution packaging not yet implemented)
```

Outputs:

- `artifacts/distribution/elegy-memory-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-mcp-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-planning-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-skills-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-configuration-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-documentation-wrapper-<bundleVersion>.zip`

Each wrapper archive contains archive-root `README.md`, the dedicated wrapper root content, `wrapper-entrypoint.json`, a surface-local `install.ps1`, a surface-local `skills/<surface>/SKILL.md` bridge, and a bundled copy of `scripts/install-distribution.sh` so the wrapper stays usable outside a full repo checkout.

Wrapper archives already embed the generic installer helper. Consumers that only need a dedicated wrapper surface can use the wrapper archive directly without separately downloading the standalone installer asset.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption; workflow artifacts remain a maintainer/CI convenience rather than the primary handoff lane.
- Treat the standalone installer asset as a convenience bootstrap, not as a separate distribution model.
- Pin an explicit Elegy semver release tag in downstream repositories and install into a repo-local tools directory.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Treat the extracted `contracts` directory and the selected CLI executables as the supported downstream handoff surfaces.
- Wrapper archives already carry the generic installer helper and remain valid when a downstream only needs that bounded surface.
- Keep any host-specific runtime/bootstrap behavior in the consuming repository.
- Do not reintroduce NuGet or GitHub Packages as the primary downstream lane.
- Treat the rolling `main-snapshot` prerelease as an integration/debug lane rather than a pinned downstream contract.

Historical GitHub Packages and NuGet publication surfaces remain frozen/deprecated. Remove any remaining metadata only after downstream consumer cutover evidence exists.

## Maintainer flow

1. Update bundle and manifest package metadata/version when the governed contracts surface changes.
2. Run `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
3. Ensure CLI publishing stays aligned to the explicit workflow target set and the current CLI surface selector set: `elegy-cli`, `elegy-memory`, `elegy-mcp`, `elegy-planning`, `elegy-skills`, `elegy-configuration`, and `elegy-documentation`; the umbrella `elegy-cli` selector publishes the `elegy` binary.
# (distribution packaging not yet implemented)
5. Run `pwsh ./scripts/package-installer.ps1`.
7. Run `pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs -RequireArchive -RequireWrapperArchives -RequireInstallerArchives -RequireReleaseMetadata`.
8. Publish the generated assets through the GitHub Actions workflows when ready. Pushes to `main` refresh the rolling `main-snapshot` prerelease; semver tags or published release events refresh the matching stable release.
