# Elegy distribution

Elegy no longer treats package-feed publication as the supported downstream surface.

Use the governed contract bundle exported from `contracts/` instead of package restore or sibling-repository project references.

- Repository: https://github.com/Sofreshx/Elegy
- Contracts bundle guidance: see `docs/distribution.md`
- Export command: `pwsh ./scripts/export-contracts.ps1 -CreateArchive`

Historical package feeds are retired. Downstream consumers should integrate through the exported contract bundle and the Rust runtime surfaces that consume it.
