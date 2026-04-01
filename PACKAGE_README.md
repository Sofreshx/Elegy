# Elegy distribution

Elegy no longer treats package-feed publication as the supported downstream surface.

Use GitHub release assets instead of package restore, sibling-repository project references, or new package-feed publication lanes.

- Repository: https://github.com/Sofreshx/Elegy
- Contracts bundle guidance: see `docs/distribution.md`
- Export command: `pwsh ./scripts/export-contracts.ps1 -CreateArchive`
- CLI release targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, and `aarch64-apple-darwin`
- CLI archive surfaces: `elegy-cli`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`
- Wrapper archive surfaces: `elegy-memory-wrapper`, `elegy-mcp-wrapper`, and `elegy-skills-wrapper`
- Standalone installer bootstrap asset: `elegy-installer-<bundleVersion>.zip`
- Release metadata assets: `elegy-release-manifest-<bundleVersion>.json` and `elegy-release-checksums-<bundleVersion>.json`
- Generic install helper: `pwsh ./scripts/install-distribution.ps1 -Tag <releaseTag> -Destination <path> -CliSurfaces <surface[,surface...]> -WrapperSurfaces <surface[,surface...]>`
- Local artifact install helper: `pwsh ./scripts/install-distribution.ps1 -LocalArtifactsRoot ./artifacts/distribution -Destination <path> -CliSurfaces <surface[,surface...]> -WrapperSurfaces <surface[,surface...]>`

The installer now resolves the manifest and checksums first, verifies exact asset size and SHA-256 plus required archive entries, and writes `install-receipt.json` into the destination root.

GitHub Releases are the primary downstream lane. The standalone installer asset is a convenience bootstrap, and the wrapper archives already embed the same installer helper. Stable semver tags remain the supported downstream contract, while pushes to `main` now refresh a rolling `main-snapshot` prerelease for latest-branch validation.

Holon and other downstream consumers should pin an Elegy semver release tag and install into a repo-local tools directory. Historical GitHub Packages and NuGet surfaces are frozen/deprecated, and any remaining cleanup should wait until consumer cutover evidence exists. Downstream consumers should integrate through the exported contract bundle and the release/archive CLI surfaces rather than the rolling `main-snapshot` prerelease.
