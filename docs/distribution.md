# Distribution and downstream consumption

Elegy is intended to be consumed through versioned release assets, not through sibling-repository workspace references or package-feed distribution.

The active authority root is `contracts/`, with bundle and schema policy under `governance/version-policy.json`. The current in-repo CLI surfaces are the general `elegy` CLI plus the dedicated `elegy-memory`, `elegy-mcp`, `elegy-planning`, and `elegy-skills` binaries built from the in-repo Rust workspace. Tagged release workflows publish archives for those surfaces plus the four dedicated wrapper archives, and pushes to `main` refresh a rolling `main-snapshot` prerelease with the same asset set for latest-integration validation.

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
- local memory wrapper archive: `elegy-memory-wrapper-<bundleVersion>.zip`
- MCP wrapper archive: `elegy-mcp-wrapper-<bundleVersion>.zip`
- planning wrapper archive: `elegy-planning-wrapper-<bundleVersion>.zip`
- skills wrapper archive: `elegy-skills-wrapper-<bundleVersion>.zip`

The contracts bundle remains the canonical machine-readable handoff for schemas, fixtures, compatibility metadata, and parity fixtures.

GitHub Releases are the primary downstream distribution lane. The standalone installer archive is a convenience bootstrap that carries the generic install helper only; it does not introduce a separate package-feed or runtime distribution path. Stable downstream consumption should continue to use explicit semver tags such as `v1.3.2`, while the rolling `main-snapshot` prerelease exists only as a continuously refreshed integration build.

Downloadable archives are self-describing. Packaging stages `PACKAGE_README.md` into every downloadable zip as archive-root `README.md`, and manifest validation treats that README as a required payload entry for the CLI, wrapper, and installer archive families.

Each CLI archive is a thin distribution of its corresponding executable plus archive-root `README.md` for one explicitly published host target. The umbrella `elegy-cli-<cliVersion>-<target>.zip` archive specifically carries the `elegy` binary and is the downloadable surface for the umbrella feature families: Mermaid, diagram, skills registry, lower-level `generate skills`, `run`, observe, desktop, repo, web, data, and notify. The current published target set is intentionally narrow:

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
pwsh ./scripts/write-distribution-manifest.ps1 -OutputDirectory ./artifacts/distribution -Tag local-artifacts
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
package contract. This is metadata and validation support for consuming hosts,
plus conservative derived projection tooling support, not an Elegy plugin runtime:

- `elegy-plugin-package-v1.schema.json`
- `fixtures/elegy-plugin-package-v1.minimal.json`

Current governed dedicated-surface skill artifacts in that bundle include:

- `fixtures/skill-definition-v2.elegy-memory.json`
- `fixtures/skill-discovery-index.elegy-memory.json`
- `fixtures/skill-definition-v2.elegy-mcp.json`
- `fixtures/skill-discovery-index.elegy-mcp.json`
- `fixtures/skill-definition-v2.elegy-planning.json`
- `fixtures/skill-definition-v2.elegy-skills.json`
- `fixtures/skill-discovery-index.elegy-skills.json`
- `fixtures/skill-definition-v2.elegy-mermaid.json`
- `fixtures/skill-discovery-index.elegy-mermaid.json`

The repo carries `.agents/skills/elegy-memory/SKILL.md`, `.agents/skills/elegy-mcp/SKILL.md`, `.agents/skills/elegy-skills/SKILL.md`, and `.agents/skills/elegy-mermaid/SKILL.md` as repo-local host-facing derived mirrors for those surfaces. The repo also carries matching `.github/skills/.../SKILL.md` files as repo-local non-authoritative contributor-routing mirrors. Those markdown files are not part of the governed contracts bundle.

The current lower-level contributor tooling also includes `elegy generate codex-plugin`, which projects a portable package into a conservative local Codex plugin folder containing `.codex-plugin/plugin.json` and `skills/`. That generated plugin folder is a derived local output and is not currently a release asset family.

## CLI archive

Build and package a current-host CLI surface with:

```powershell
pwsh ./scripts/package-cli.ps1 -Surface elegy-cli
pwsh ./scripts/package-cli.ps1 -Surface elegy-memory
pwsh ./scripts/package-cli.ps1 -Surface elegy-mcp
pwsh ./scripts/package-cli.ps1 -Surface elegy-planning
pwsh ./scripts/package-cli.ps1 -Surface elegy-skills
```

Output:

- versioned archive: `artifacts/distribution/<surface>-<cliVersion>-<target>.zip`

Release workflows publish the explicit target set above by calling `pwsh ./scripts/package-cli.ps1 -Surface <surface> -Target <target>` for each current CLI surface and supported target.

Each archive contains only its corresponding executable plus archive-root `README.md`. These archives do not add host bootstrap logic, consumer config, or downstream runtime wiring.

For Mermaid tooling, use the umbrella `elegy` archive. The same umbrella archive is also the downloadable surface for diagram, skills registry, lower-level `generate skills`, lower-level `generate codex-plugin`, `run`, observe, desktop, repo, web, data, and notify; those commands remain general-surface commands under the existing `elegy` executable rather than dedicated release targets.

## Wrapper archive

Build the platform-neutral wrapper archives with:

```powershell
pwsh ./scripts/package-wrapper-surface.ps1
```

Outputs:

- `artifacts/distribution/elegy-memory-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-mcp-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-planning-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-skills-wrapper-<bundleVersion>.zip`

Each wrapper archive contains archive-root `README.md`, the dedicated wrapper root content, `wrapper-entrypoint.json`, a surface-local `install.ps1`, a surface-local `skills/<surface>/SKILL.md` bridge, and a bundled copy of `scripts/install-distribution.ps1` so the wrapper stays usable outside a full repo checkout.

Wrapper archives already embed the generic installer helper. Consumers that only need a dedicated wrapper surface can use the wrapper archive directly without separately downloading the standalone installer asset.

## Holon-oriented quick start

For Holon or any other downstream host that wants the simplest supported consumption path:

1. Pick and pin an Elegy release tag in the downstream repository.
2. Download the standalone installer asset or vendor the same `install-distribution.ps1` helper into a repo-local bootstrap directory such as `./tools/elegy-bootstrap`.
3. Run the generic installer helper into a repo-local tools directory such as `./tools/elegy`.
4. Consume the extracted `contracts` directory as the governed artifact surface and invoke the extracted binaries directly from `bin/<surface>/`.

Example using the standalone installer asset after extraction into `./tools/elegy-bootstrap`:

```powershell
pwsh ./tools/elegy-bootstrap/install-distribution.ps1 -Tag v0.1.0 -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-planning,elegy-skills -WrapperSurfaces elegy-mcp,elegy-skills -Force
```

Example using a checked-out or vendored installer helper against release assets:

```powershell
pwsh ./scripts/install-distribution.ps1 -Tag v0.1.0 -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-planning,elegy-skills -WrapperSurfaces elegy-mcp,elegy-skills -Force
```

Example using local artifacts only:

```powershell
pwsh ./scripts/write-distribution-manifest.ps1 -OutputDirectory ./artifacts/distribution -Tag local-artifacts
pwsh ./scripts/install-distribution.ps1 -LocalArtifactsRoot ./artifacts/distribution -Destination ./tools/elegy-local -CliSurfaces elegy-memory -WrapperSurfaces elegy-memory -Force
pwsh ./src/Elegy-memory/install.ps1 -LocalArtifactsRoot ./artifacts/distribution -Destination ./tools/elegy-memory-wrapper -Force
```

The installer resolves either a release tag or a local artifacts directory, downloads or copies the manifest and checksums first, validates that every requested asset exists in the manifest, then verifies exact file size, SHA-256, and required archive entries before extracting the contracts bundle under `contracts/`, CLI assets under `bin/<surface>/`, and wrapper assets under `wrappers/<surface>/`. When `-LocalArtifactsRoot` is used, the root must contain exactly one manifest and checksum file plus the exact assets referenced by that manifest; the installer now fails on ambiguous or stale metadata instead of guessing between stale files. For backward compatibility, selecting `elegy-cli` also populates the legacy `cli/` path. The installer does not assume sibling repositories, write Holon-specific configuration, or depend on package feeds.

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

1. Update bundle and manifest package metadata/version in `governance/version-policy.json` when the governed contracts surface changes.
2. Run `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
3. Ensure CLI publishing stays aligned to the explicit workflow target set and the current CLI surface selector set: `elegy-cli`, `elegy-memory`, `elegy-mcp`, `elegy-planning`, and `elegy-skills`; the umbrella `elegy-cli` selector publishes the `elegy` binary.
4. Run `pwsh ./scripts/package-wrapper-surface.ps1`.
5. Run `pwsh ./scripts/package-installer.ps1`.
6. Run `pwsh ./scripts/write-distribution-manifest.ps1 -OutputDirectory ./artifacts/distribution -Tag local-artifacts` for local validation, or let the publish workflow generate the same files with the release tag.
7. Run `pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs -RequireArchive -RequireWrapperArchives -RequireInstallerArchives -RequireReleaseMetadata`.
8. Run `pwsh ./scripts/validate-package-boundaries.ps1`.
9. Publish the generated assets through the GitHub Actions workflows when ready. Pushes to `main` refresh the rolling `main-snapshot` prerelease; semver tags or published release events refresh the matching stable release.
