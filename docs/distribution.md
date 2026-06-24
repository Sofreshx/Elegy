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

## Asset families (conventions)

| Family | Pattern | Notes |
| --- | --- | --- |
| Standalone installer bootstrap | `elegy-installer-<bundleVersion>.zip` | Carries `install-distribution.sh` (canonical) + `install-distribution.ps1` (thin shim) + `README.md`. |
| Release manifest | `elegy-release-manifest-<tag>.json` | Emitted by `.github/workflows/publish-orchestrator.yml`. |
| Release checksums | `elegy-release-checksums-<tag>.json` | SHA-256 of every published asset and the manifest. |
| CLI asset | `<name>-<target>[.exe]` | Per binary surface and target, resolved through `distribution/surfaces.json`. |
| CLI asset checksum | `<name>-<target>[.exe].sha256` | Sidecar checksum used by the installer. |

## Surface Catalog

Release configuration uses `distribution/surfaces.json` as the central catalog. It maps workspace crates and surfaces to their release identities, build targets, and description. The publish orchestrator reads this catalog to discover which surfaces to build and release.

To add a new release surface, add an entry to `distribution/surfaces.json` and ensure the crate builds. No per-feature workflow files are needed.

Each dedicated binary is listed in the catalog with kind `cli`. Most build from a package with the same name; surfaces with a different package declare `package` explicitly. Skill-only surfaces (those without a corresponding Rust binary) are listed with kind `skill-only`.

## Install

The canonical installer is `scripts/install-distribution.sh`. The `scripts/install-distribution.ps1` file is a thin shim that maps PowerShell-style flags to the bash script and then delegates via `bash`.

The installer is a simplified script that downloads one flat binary asset plus its `.sha256` sidecar. It does not depend on jq or archive extraction.

```bash
# From a repo checkout or release archive
bash ./scripts/install-distribution.sh --tag vX.Y.Z --destination ./tools/elegy --surface elegy-planning --force

# PowerShell entry point
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -Surface elegy-planning -Force
```

To install a surface, the surface must exist in the release assets and have a published `.sha256` sidecar. The installer verifies the downloaded asset SHA-256 before writing the executable into the destination `bin/` directory.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption. Workflow artifacts are a maintainer/CI convenience, not the primary handoff lane.
- Pin an explicit Elegy semver release tag in downstream repositories and install into a repo-local tools directory.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Keep any host-specific runtime/bootstrap behavior in the consuming repository. Elegy owns the contracts, the binaries, and the generic installer; the consuming repo owns product wiring.
- Use `cargo add elegy-plugin-sdk` for external plugin repos that need plugin types, validation, scaffolding, and export.
- Do not reintroduce NuGet or GitHub Packages as the primary downstream lane.
- Treat the rolling `main-snapshot` prerelease as an integration/debug lane, not a pinned downstream contract.

## Where to read more

- Release publishing (CLI archives, aggregate artifacts, manifest, checksums): `.github/workflows/publish-orchestrator.yml`.
- Manual metadata recovery for an existing release: `.github/workflows/release-finalize.yml`.
- Aggregate artifacts (installer bootstrap): `.github/workflows/distribution-artifacts.yml`.
- Authority surfaces: [`docs/architecture/ecosystem-topology.md`](./architecture/ecosystem-topology.md).
