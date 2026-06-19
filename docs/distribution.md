# Distribution and downstream consumption

Elegy ships capability binaries through GitHub release assets, not through
package feeds or sibling-repo workspace references. The release contract is
intentionally narrow:

- **Stable semver tags** (e.g. `v1.3.2`) are the supported downstream
  contract that consumers should pin.
- **Rolling prerelease `main-snapshot`** is refreshed on every push to `main`
  and is intended for validation, debugging, and latest-branch integration
  checks. Same asset families, different lifecycle promise.

## Published targets

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

The installer only resolves those exact release targets and fails closed on
unsupported host architectures.

## Release channels

| Channel | When | Use it for |
| --- | --- | --- |
| Stable semver (e.g. `v1.3.2`) | Tagged release | Pin in downstream consumers |
| `main-snapshot` rolling prerelease | Every push to `main` | Validation, debug, latest-branch integration |

Both channels publish the same asset families. The difference is lifecycle
and stability promise, not package coverage.

Bundle version and CLI version are intentionally independent. Consumers
should resolve both assets from the same release tag rather than assuming
`bundleVersion == cliVersion`.

## Asset families (conventions)

| Family | Pattern | Notes |
| --- | --- | --- |
| Contracts bundle | `elegy-contracts-<bundleVersion>.zip` | Governed schemas, fixtures, compatibility metadata. The canonical machine-readable handoff. |
| Standalone installer bootstrap | `elegy-installer-<bundleVersion>.zip` | Carries `install-distribution.sh` (canonical) + `install-distribution.ps1` (thin shim) + `README.md`. |
| Release manifest | `elegy-release-manifest-<bundleVersion>.json` | Finalized by `.github/workflows/release-finalize.yml` on `release: published` events. |
| Release checksums | `elegy-release-checksums-<bundleVersion>.json` | SHA-256 of every published asset. Same workflow. |
| CLI archive | `elegy-<surface>-<cliVersion>-<target>.zip` | Per binary surface, per target. Self-describing. |
| Wrapper archive | `elegy-<surface>-wrapper-<bundleVersion>.zip` | Per-feature bounded integration surface bundling a CLI binary + SKILL.md mirror + installer. |

## Per-feature self-ownership

Each binary or wrapper surface owns its own distribution end-to-end:

- **The per-feature `contracts/fixtures/elegy-plugin-package.<feature>.json` fixture**
  declares the surface name, CLI binary, archive family, asset prefix, skill
  bridge, installer filename, optional pre/post publish hooks, and an optional
  target override. This is the **only** place publish metadata lives; the
  per-feature publish workflow reads from this fixture and the central
  install/validate scripts derive surface lists from the same fixtures. There
  is no central catalog.
- **The per-feature `.github/workflows/publish-<feature>.yml` workflow** is a
  thin caller of the generic `._reusable-publish.yml` workflow. Each
  per-feature workflow is ~25 lines and declares only its own inputs; adding
  a new feature is one fixture + one caller file, with no change to a central
  orchestrator.
- **The per-feature `rust/features/<feature>/DISTRIBUTION.md`** describes the
  feature's distribution shape: binary name, archive family, install command,
  build-from-source command, validation command, where to read more.

The `docs/distribution.md` file is the entry point. It does not enumerate
per-feature tables or archive family lists; those live in each per-feature
`DISTRIBUTION.md`. To install a feature, read its `DISTRIBUTION.md`.

## Install

The canonical installer is `scripts/install-distribution.sh`. The
`scripts/install-distribution.ps1` file is a thin shim that forwards all
arguments to the bash script via `bash`. The shim exists for Windows users
who want a native-pwsh entry point; it carries no install logic of its own.

```bash
# From a repo checkout
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-memory -Force

# From a release archive
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-memory -Force

# PowerShell entry point (forwards to bash)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-memory -Force
```

The installer downloads the contracts bundle first, extracts it, and reads
`contracts/fixtures/elegy-plugin-package.*.json` from the extracted location
to derive the per-feature surface list. To install a surface, the surface
must exist as a fixture in the contracts bundle; there is no separate
catalog.

The installer resolves the release manifest and checksums, verifies asset
size, SHA-256, and required archive entries, then writes
`install-receipt.json` into the destination root.

## Contracts bundle

```bash
cd rust && cargo run -p elegy-cli -- contracts export --output ../artifacts/contracts --create-archive --archive-output ../artifacts/distribution/elegy-contracts-bundle.zip
```

Output: `artifacts/distribution/elegy-contracts-*.zip`. The
contracts bundle is the canonical machine-readable handoff for schemas,
fixtures, compatibility metadata, and parity fixtures.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption. Workflow artifacts
  are a maintainer/CI convenience, not the primary handoff lane.
- Pin an explicit Elegy semver release tag in downstream repositories and
  install into a repo-local tools directory.
- Do not hard-code sibling checkout paths or assume a shared parent workspace
  layout.
- Keep any host-specific runtime/bootstrap behavior in the consuming
  repository. Elegy owns the contracts, the binaries, and the generic
  installer; the consuming repo owns product wiring.
- Do not reintroduce NuGet or GitHub Packages as the primary downstream lane.
- Treat the rolling `main-snapshot` prerelease as an integration/debug lane,
  not a pinned downstream contract.
- Historical GitHub Packages and NuGet publication surfaces remain
  frozen/deprecated. Remove any remaining metadata only after downstream
  consumer cutover evidence exists.

## Where to read more

- Per-feature distribution: each `rust/features/<feature>/DISTRIBUTION.md`
  and `rust/bin/elegy-cli/DISTRIBUTION.md` etc.
- Per-feature publish workflow: each `.github/workflows/publish-<feature>.yml`.
- Generic publish mechanics: `.github/workflows/_reusable-publish.yml`.
- Release finalization (manifest + checksums): `.github/workflows/release-finalize.yml`.
- Aggregate artifacts (contracts bundle, installer bootstrap): `.github/workflows/distribution-artifacts.yml`.
- Authority surfaces: [`docs/architecture/ecosystem-topology.md`](./architecture/ecosystem-topology.md).
- Authoring a new plugin package: `contracts/AGENTS.md` and `contracts/fixtures/elegy-plugin-package.template.json`.
