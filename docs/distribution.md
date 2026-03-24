# Distribution and downstream consumption

Elegy is intended to be consumed through versioned release assets, not through sibling-repository workspace references or package-feed distribution.

The active authority root is `contracts/`, with bundle and schema policy under `governance/version-policy.json`. The current in-repo CLI surfaces are the general `elegy` CLI plus the dedicated `elegy-memory`, `elegy-mcp`, and `elegy-skills` binaries built from the in-repo Rust workspace, and tagged release workflows are configured to publish archives for those surfaces plus the three dedicated wrapper archives.

The bounded local memory operator lives in `rust/crates/elegy-memory` and exposes the `elegy-memory` binary. `rust/crates/elegy-mcp` and `rust/crates/elegy-skills` now expose their own dedicated binaries for descriptor authoring/analysis and MCP-to-skill generation. The shared `elegy` CLI remains the general and compatibility surface.

## Asset model

Tagged releases are configured to publish nine neutral assets across the contracts, installer, CLI, and dedicated wrapper lanes:

- governed contracts bundle: `elegy-contracts-<bundleVersion>.zip`
- standalone installer bootstrap: `elegy-installer-<bundleVersion>.zip`
- umbrella CLI archive: `elegy-cli-<cliVersion>-<target>.zip`
- local memory CLI archive: `elegy-memory-<cliVersion>-<target>.zip`
- MCP CLI archive: `elegy-mcp-<cliVersion>-<target>.zip`
- skills CLI archive: `elegy-skills-<cliVersion>-<target>.zip`
- local memory wrapper archive: `elegy-memory-wrapper-<bundleVersion>.zip`
- MCP wrapper archive: `elegy-mcp-wrapper-<bundleVersion>.zip`
- skills wrapper archive: `elegy-skills-wrapper-<bundleVersion>.zip`

The contracts bundle remains the canonical machine-readable handoff for schemas, fixtures, compatibility metadata, and parity fixtures.

GitHub Releases are the primary downstream distribution lane. The standalone installer archive is a convenience bootstrap that carries the generic install helper only; it does not introduce a separate package-feed or runtime distribution path.

Each CLI archive is a thin distribution of its corresponding executable for one explicitly published host target. The umbrella `elegy-cli-<cliVersion>-<target>.zip` archive specifically carries the `elegy` binary. The current published target set is intentionally narrow:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

The installer only resolves those exact release targets and fails clearly on unsupported host architectures.

Bundle version and CLI version are intentionally independent. Consumers should resolve both assets from the same release tag rather than assuming `bundleVersion == cliVersion`.

## Standalone installer archive

Build the standalone installer bootstrap asset with:

```powershell
pwsh ./scripts/package-installer.ps1
```

Output:

- versioned archive: `artifacts/distribution/elegy-installer-<bundleVersion>.zip`

The standalone installer archive contains `install-distribution.ps1` at the archive root so downstream repos can download a single GitHub release asset, extract it into a repo-local tools/bootstrap directory, and then fetch the contracts bundle plus the selected CLI and wrapper archives through the supported installer path.

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

Current governed dedicated-surface skill artifacts in that bundle include:

- `fixtures/skill-definition.elegy-memory.json`
- `fixtures/skill-discovery-index.elegy-memory.json`
- `fixtures/skill-definition.elegy-mcp.json`
- `fixtures/skill-discovery-index.elegy-mcp.json`
- `fixtures/skill-definition.elegy-skills.json`
- `fixtures/skill-discovery-index.elegy-skills.json`

The repo carries `.github/skills/elegy-memory/SKILL.md`, `.github/skills/elegy-mcp/SKILL.md`, and `.github/skills/elegy-skills/SKILL.md` as repo-local non-authoritative contributor-routing files for those surfaces. Those markdown files are not part of the governed contracts bundle.

## CLI archive

Build and package a current-host CLI surface with:

```powershell
pwsh ./scripts/package-cli.ps1 -Surface elegy-cli
pwsh ./scripts/package-cli.ps1 -Surface elegy-memory
pwsh ./scripts/package-cli.ps1 -Surface elegy-mcp
pwsh ./scripts/package-cli.ps1 -Surface elegy-skills
```

Output:

- versioned archive: `artifacts/distribution/<surface>-<cliVersion>-<target>.zip`

Release workflows publish the explicit target set above by calling `pwsh ./scripts/package-cli.ps1 -Surface <surface> -Target <target>` for each current CLI surface and supported target.

Each archive contains only its corresponding executable. These archives do not add host bootstrap logic, consumer config, or downstream runtime wiring.

## Wrapper archive

Build the platform-neutral wrapper archives with:

```powershell
pwsh ./scripts/package-wrapper-surface.ps1
```

Outputs:

- `artifacts/distribution/elegy-memory-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-mcp-wrapper-<bundleVersion>.zip`
- `artifacts/distribution/elegy-skills-wrapper-<bundleVersion>.zip`

Each wrapper archive contains the dedicated wrapper root content, `wrapper-entrypoint.json`, a surface-local `install.ps1`, a surface-local `skills/<surface>/SKILL.md` bridge, and a bundled copy of `scripts/install-distribution.ps1` so the wrapper stays usable outside a full repo checkout.

Wrapper archives already embed the generic installer helper. Consumers that only need a dedicated wrapper surface can use the wrapper archive directly without separately downloading the standalone installer asset.

## Holon-oriented quick start

For Holon or any other downstream host that wants the simplest supported consumption path:

1. Pick and pin an Elegy release tag in the downstream repository.
2. Download the standalone installer asset or vendor the same `install-distribution.ps1` helper into a repo-local bootstrap directory such as `./tools/elegy-bootstrap`.
3. Run the generic installer helper into a repo-local tools directory such as `./tools/elegy`.
4. Consume the extracted `contracts` directory as the governed artifact surface and invoke the extracted binaries directly from `bin/<surface>/`.

Example using the standalone installer asset after extraction into `./tools/elegy-bootstrap`:

```powershell
pwsh ./tools/elegy-bootstrap/install-distribution.ps1 -Tag v0.1.0 -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-skills -WrapperSurfaces elegy-mcp,elegy-skills -Force
```

Example using a checked-out or vendored installer helper against release assets:

```powershell
pwsh ./scripts/install-distribution.ps1 -Tag v0.1.0 -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-skills -WrapperSurfaces elegy-mcp,elegy-skills -Force
```

Example using local artifacts only:

```powershell
pwsh ./scripts/install-distribution.ps1 -LocalArtifactsRoot ./artifacts/distribution -Destination ./tools/elegy-local -CliSurfaces elegy-memory -WrapperSurfaces elegy-memory -Force
pwsh ./src/Elegy-memory/install.ps1 -LocalArtifactsRoot ./artifacts/distribution -Destination ./tools/elegy-memory-wrapper -Force
```

The installer resolves either a release tag or a local artifacts directory, downloads or copies the contracts bundle and the matching host CLI archives for the selected surfaces, extracts the CLI assets under `bin/<surface>/`, extracts wrapper assets under `wrappers/<surface>/`, and prints the resulting paths. When `-LocalArtifactsRoot` is used, the root must contain exactly one matching archive for each required asset; the installer now fails on ambiguous roots instead of guessing between stale files. For backward compatibility, selecting `elegy-cli` also populates the legacy `cli/` path. The installer does not assume sibling repositories, write Holon-specific configuration, or depend on package feeds.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption; workflow artifacts remain a maintainer/CI convenience rather than the primary handoff lane.
- Treat the standalone installer asset as a convenience bootstrap, not as a separate distribution model.
- Pin an explicit Elegy release tag in downstream repositories and install into a repo-local tools directory.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Treat the extracted `contracts` directory and the selected CLI executables as the supported downstream handoff surfaces.
- Wrapper archives already carry the generic installer helper and remain valid when a downstream only needs that bounded surface.
- Keep any host-specific runtime/bootstrap behavior in the consuming repository.
- Do not reintroduce NuGet or GitHub Packages as the primary downstream lane.

Historical GitHub Packages and NuGet publication surfaces remain frozen/deprecated. Remove any remaining metadata only after downstream consumer cutover evidence exists.

## Maintainer flow

1. Update bundle and manifest package metadata/version in `governance/version-policy.json` when the governed contracts surface changes.
2. Run `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
3. Ensure CLI publishing stays aligned to the explicit workflow target set and the current CLI surface selector set: `elegy-cli`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`; the umbrella `elegy-cli` selector publishes the `elegy` binary.
4. Run `pwsh ./scripts/package-wrapper-surface.ps1`.
5. Run `pwsh ./scripts/package-installer.ps1`.
6. Run `pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs -RequireArchive -RequireWrapperArchives -RequireInstallerArchives`.
7. Run `pwsh ./scripts/validate-package-boundaries.ps1`.
8. Publish the generated assets through the GitHub Actions workflows when ready.
