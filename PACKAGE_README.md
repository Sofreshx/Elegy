# Elegy distribution

Elegy no longer treats package-feed publication as the supported downstream surface.

Use GitHub release assets instead of package restore or sibling-repository project references.

- Repository: https://github.com/Sofreshx/Elegy
- Contracts bundle guidance: see `docs/distribution.md`
- Export command: `pwsh ./scripts/export-contracts.ps1 -CreateArchive`
- CLI release targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, and `aarch64-apple-darwin`
- CLI archive surfaces: `elegy-cli`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`
- Generic install helper: `pwsh ./scripts/install-distribution.ps1 -Tag <releaseTag> -Destination <path> -CliSurfaces <surface[,surface...]>`

Historical package feeds are no longer the supported downstream surface, and remaining migration and consumer overlap is tracked in `docs/migration/holon-purge-consumers.md`. Downstream consumers should integrate through the exported contract bundle and the release/archive CLI surfaces.
