# Elegy distribution

This file ships inside downloadable Elegy archives as the archive-level `README.md`.

Elegy is distributed through GitHub release assets, not package feeds or sibling-repo workspace references. Each zip is intentionally self-describing: the archive contains its payload plus this README.

## Distribution model

- **Contracts bundle**: governed schemas, fixtures, and compatibility metadata.
- **CLI archives**: executable distributions for the published CLI surfaces.
- **Wrapper archives**: bounded repo-local integration surfaces for dedicated tools.
- **Installer bootstrap**: the generic install helper packaged as its own downloadable archive.
- **Release metadata**: manifest + checksums used by installer and validation flows.

## Archive families

- `elegy-cli-<cliVersion>-<target>.zip`
  - Ships the umbrella `elegy` binary.
  - Carries the umbrella feature families: mermaid, diagram, skills registry (`skills list|describe|get|search|resolve|capability|validate`), lower-level `generate skills`, `run`, observe, desktop, repo, web, data, and notify.
- `elegy-memory-<cliVersion>-<target>.zip`
  - Dedicated `elegy-memory` binary.
- `elegy-mcp-<cliVersion>-<target>.zip`
  - Dedicated `elegy-mcp` binary.
- `elegy-planning-<cliVersion>-<target>.zip`
  - Dedicated `elegy-planning` binary.
- `elegy-skills-<cliVersion>-<target>.zip`
  - Dedicated `elegy-skills` binary.
- `elegy-memory-wrapper-<bundleVersion>.zip`, `elegy-mcp-wrapper-<bundleVersion>.zip`, `elegy-skills-wrapper-<bundleVersion>.zip`
  - Dedicated wrapper surfaces with wrapper metadata, local install entrypoint, skill bridge, bundled installer helper, and this README.
- `elegy-installer-<bundleVersion>.zip`
  - Standalone installer bootstrap with `install-distribution.ps1` and this README.

## Targets and release lane

- Published CLI targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`
- Stable semver tags are the supported downstream contract.
- `main-snapshot` is a rolling prerelease for latest-branch validation.

## Where to read more

- Repository: https://github.com/Sofreshx/Elegy
- Main user-facing guide: `README.md`
- Distribution authority and maintainer guidance: `docs/distribution.md`
- Generic installer: `pwsh ./scripts/install-distribution.ps1 -Tag <releaseTag> -Destination <path> -CliSurfaces <surface[,surface...]> -WrapperSurfaces <surface[,surface...]>`
- Local artifact installer: `pwsh ./scripts/install-distribution.ps1 -LocalArtifactsRoot ./artifacts/distribution -Destination <path> -CliSurfaces <surface[,surface...]> -WrapperSurfaces <surface[,surface...]>`

The installer resolves the release manifest and checksums first, verifies asset size, SHA-256, and required archive entries, then writes `install-receipt.json` into the destination root.
