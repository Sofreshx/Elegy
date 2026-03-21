# Elegy distribution

Elegy no longer treats package-feed publication as the supported downstream surface.

Use the governed contract bundle exported from `contracts/` instead of `.NET` package restore or sibling-repository project references.

- Repository: https://github.com/Sofreshx/Elegy
- Contracts bundle guidance: see `docs/distribution.md`
- Export command: `pwsh ./scripts/export-contracts.ps1 -CreateArchive`

Common package families include:

- `Elegy.Formalization.Core`
- `Elegy.Formalization.Contracts`
- `Elegy.Formalization.Skills`
- `Elegy.Formalization.Skills.Discovery`
- `Elegy.Formalization.Mcp`
- `Elegy.Formalization.SkillForge`

For installation and authentication guidance, see the repository distribution guide.
