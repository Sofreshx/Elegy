# Distribution and downstream consumption

Elegy is intended to be consumed through versioned release assets, not through sibling-repository workspace references or package-feed distribution.

The active authority root is `contracts/`, with bundle and schema policy under `governance/version-policy.json`. The published general host CLI surface remains the existing `elegy` CLI built from `rust/crates/elegy-cli`.

The bounded local memory operator itself now lives in `rust/crates/elegy-memory` and exposes the `elegy-memory` binary. The shared `elegy` CLI keeps only a temporary compatibility bridge for legacy memory commands, and this document does not add a separate memory release-asset claim.

## Asset model

Tagged releases now publish two neutral surfaces:

- governed contracts bundle: `elegy-contracts-<bundleVersion>.zip`
- host CLI archive: `elegy-cli-<cliVersion>-<target>.zip`

The contracts bundle remains the canonical machine-readable handoff for schemas, fixtures, compatibility metadata, and parity fixtures.

The CLI archive is a thin distribution of the existing `elegy` executable for one explicitly published host target. The initial shipped target set is intentionally narrow:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

The installer only resolves those exact release targets and fails clearly on unsupported host architectures.

Bundle version and CLI version are intentionally independent. Consumers should resolve both assets from the same release tag rather than assuming `bundleVersion == cliVersion`.

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

Current governed memory-skill artifacts in that bundle include:

- `fixtures/skill-definition.elegy-memory.json`
- `fixtures/skill-discovery-index.elegy-memory.json`

The repo also materializes `.github/skills/elegy-memory/SKILL.md` for contributor routing over the `elegy-memory` surface. That markdown file is a repo-local non-authoritative render and is not part of the governed contracts bundle.

## CLI archive

Build and package the current-host `elegy` binary with:

```powershell
pwsh ./scripts/package-cli.ps1
```

Output:

- versioned archive: `artifacts/distribution/elegy-cli-<cliVersion>-<target>.zip`

Release workflows publish the explicit target set above by calling `pwsh ./scripts/package-cli.ps1 -Target <target>` for each supported target.

The archive contains the existing `elegy` executable only. It does not add host bootstrap logic, consumer config, or downstream runtime wiring.

## Holon-oriented quick start

For Holon or any other downstream host that wants the simplest supported consumption path:

1. Pick an Elegy release tag.
2. Run the generic installer helper from a checked-out copy or a vendored copy of `scripts/install-distribution.ps1`.
3. Consume the extracted `contracts` directory as the governed artifact surface and invoke the extracted `elegy` binary directly.

Example:

```powershell
pwsh ./scripts/install-distribution.ps1 -Tag v0.1.0 -Destination ./tools/elegy -Force
```

The installer resolves the release tag, downloads the contracts bundle and the matching host CLI archive, extracts them, and prints the resulting paths. It does not assume sibling repositories, write Holon-specific configuration, or depend on package feeds.

## Downstream guidance

- Prefer GitHub release assets or workflow artifacts for both contracts and CLI distribution.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Treat the extracted `contracts` directory and the `elegy` executable as the supported downstream handoff surfaces.
- Keep any host-specific runtime/bootstrap behavior in the consuming repository.

## Maintainer flow

1. Update bundle and manifest package metadata/version in `governance/version-policy.json` when the governed contracts surface changes.
2. Run `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
3. Ensure CLI publishing stays aligned to the explicit workflow target set: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, and `aarch64-apple-darwin`.
4. Run `pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs -RequireArchive`.
5. Run `pwsh ./scripts/validate-package-boundaries.ps1`.
6. Publish the generated assets through the GitHub Actions workflows when ready.
