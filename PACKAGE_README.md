# Elegy distribution

Elegy no longer treats package-feed publication as the supported downstream surface.

Use GitHub release assets instead of package restore or sibling-repository project references.

- Repository: https://github.com/Sofreshx/Elegy
- Contracts bundle guidance: see `docs/distribution.md`
- Export command: `pwsh ./scripts/export-contracts.ps1 -CreateArchive`
- CLI release targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, and `aarch64-apple-darwin`
- Generic install helper: `pwsh ./scripts/install-distribution.ps1 -Tag <releaseTag> -Destination <path>`

Historical package feeds are retired. Downstream consumers should integrate through the exported contract bundle and the released `elegy` CLI archives.
