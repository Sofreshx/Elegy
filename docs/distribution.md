# Distribution and downstream consumption

Elegy ships capability binaries through GitHub release assets, not through package feeds or sibling-repo workspace references. The release contract is intentionally narrow:

- **Stable semver tags** (e.g. `v1.3.2`) are the supported downstream contract that consumers should pin.
- **Rolling prerelease `main-snapshot`** is refreshed on every push to `main` and is intended for validation, debugging, and latest-branch integration checks. Same asset families, different lifecycle promise.

## Published targets

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

The installer only resolves those exact release targets and fails closed on unsupported host architectures.

## Release channels

| Channel | When | Use it for |
| --- | --- | --- |
| Stable semver (e.g. `v1.3.2`) | Tagged release | Pin in downstream consumers |
| `main-snapshot` rolling prerelease | Every push to `main` | Validation, debug, latest-branch integration |

Both channels publish the same asset families. The difference is lifecycle and stability promise, not package coverage.

Bundle version and CLI version are intentionally independent. Consumers should resolve both assets from the same release tag rather than assuming `bundleVersion == cliVersion`.

## Asset families (conventions)

| Family | Pattern | Notes |
| --- | --- | --- |
| Contracts bundle | `elegy-contracts-bundle.zip` | Governed schemas, fixtures, compatibility metadata. The canonical machine-readable handoff. |
| Standalone installer bootstrap | `elegy-installer-<bundleVersion>.zip` | Carries `install-distribution.sh` (canonical) + `install-distribution.ps1` (thin shim) + `README.md`. |
| Release manifest | `elegy-release-manifest-<tag>.json` | Emitted by `.github/workflows/publish-orchestrator.yml`. |
| Release checksums | `elegy-release-checksums-<tag>.json` | SHA-256 of every published archive and the manifest. |
| CLI archive | `<name>-<target>-<commitSha>.zip` | Per binary surface and target, resolved through `distribution/surfaces.json`. |

## Surface Catalog

Release configuration uses `distribution/surfaces.json` as the central catalog. It maps workspace crates and surfaces to their release identities, build targets, and description. The publish orchestrator reads this catalog to discover which surfaces to build and release.

To add a new release surface, add an entry to `distribution/surfaces.json` and ensure the crate builds. No per-feature workflow files are needed.

## Install

The canonical installer is `scripts/install-distribution.sh`. The `scripts/install-distribution.ps1` file is a thin shim that forwards all arguments to the bash script via `bash`. The shim exists for Windows users who want a native-pwsh entry point; it carries no install logic of its own.

```bash
# From a repo checkout
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-memory -Force

# From a release archive
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-memory -Force

# PowerShell entry point (forwards to bash)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-memory -Force
```

The installer downloads the contracts bundle first, extracts it, and uses `distribution/surfaces.json` from the repo or release to derive the per-surface build list. To install a surface, the surface must exist in the catalog.

The installer resolves the release manifest and checksums, verifies asset size and SHA-256, then writes an install receipt into the destination root.

## Contracts bundle

```bash
cargo run -p elegy-cli -- contracts export --output-path artifacts/contracts --create-archive --archive-output-path artifacts/distribution/elegy-contracts-bundle.zip
```

Output: `artifacts/distribution/elegy-contracts-bundle.zip`. The contracts bundle is the canonical machine-readable handoff for schemas, fixtures, compatibility metadata, and parity fixtures.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption. Workflow artifacts are a maintainer/CI convenience, not the primary handoff lane.
- Pin an explicit Elegy semver release tag in downstream repositories and install into a repo-local tools directory.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Keep any host-specific runtime/bootstrap behavior in the consuming repository. Elegy owns the contracts, the binaries, and the generic installer; the consuming repo owns product wiring.
- Do not reintroduce NuGet or GitHub Packages as the primary downstream lane.
- Treat the rolling `main-snapshot` prerelease as an integration/debug lane, not a pinned downstream contract.

## Where to read more

- Per-feature distribution: each `plugins/<feature>/DISTRIBUTION.md` and `hosts/cli/DISTRIBUTION.md` etc.
- Release publishing (CLI archives, aggregate artifacts, manifest, checksums): `.github/workflows/publish-orchestrator.yml`.
- Manual metadata recovery for an existing release: `.github/workflows/release-finalize.yml`.
- Aggregate artifacts (contracts bundle, installer bootstrap): `.github/workflows/distribution-artifacts.yml`.
- Authority surfaces: [`docs/architecture/ecosystem-topology.md`](./architecture/ecosystem-topology.md).
