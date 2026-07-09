---
title: Distribution and downstream consumption
status: active
owner: elegy-core
doc_kind: guide
---

# Distribution and downstream consumption

Elegy ships release assets through GitHub Releases, not package feeds or
sibling-repo workspace assumptions.

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

Both channels publish the same asset families. The difference is lifecycle and
stability promise.

## Asset families (conventions)

| Family | Pattern | Notes |
| --- | --- | --- |
| Release manifest | `elegy-release-manifest-<tag>.json` | Emitted by `.github/workflows/publish-orchestrator.yml`. |
| Release checksums | `elegy-release-checksums-<tag>.json` | SHA-256 of every published asset and the manifest. |
| Plugin archive | `<surface>-plugin-<target>.zip` | Primary release for binary-backed plugin-packaged surfaces. Contains plugin.json, skills/, and binary. |
| Skill-only plugin archive | `<surface>-plugin-any.zip` | Primary release for skill-only plugin packages. Contains plugin.json and skills/, with no `bin/`. |
| Local pack default | `<plugin-name>-v<version>.plugin.zip` | Ad hoc output from `elegy-plugin-packaging pack` when `--output` is omitted. Not the GitHub release naming contract. |
| CLI asset | `<name>-<target>[.exe]` | Per binary surface and target, resolved through distribution/surfaces.json. Plugin-packaged surfaces bundle this with skills in plugin archives. |
| CLI asset checksum | `<name>-<target>[.exe].sha256` | Sidecar checksum used by the installer. |

## Surface Catalog

Release configuration uses `distribution/surfaces.json` as the central catalog.
It declares `schemaVersion: "elegy-surfaces/v2"` and maps workspace crates and
surfaces to their release identities, build targets, and description. The
publish orchestrator reads this catalog to discover which surfaces to build and
release.

To add a new release surface, add an entry to `distribution/surfaces.json` and ensure the crate builds. No per-feature workflow files are needed.

Each surface uses one explicit repository role:

| Kind | Contract |
| --- | --- |
| `bundled-plugin` | Installable plugin package under `plugins/`, usually with a Rust crate and skill surface. |
| `cli` | Standalone CLI under `tools/` or `shared/`, not a host adapter. |
| `host-adapter` | Host or transport surface under `hosts/`. |
| `skill-package` | Standalone skill package under `skills/elegy-*`, with no dedicated Rust binary. |
| `external-plugin-wrapper` | Public marketplace metadata under `marketplace-wrappers/` for an external/private implementation. |

External plugin wrappers use `kind: "external-plugin-wrapper"`,
`packaging: "plugin"`, `pluginRoot`, and `artifactBaseUrl`. The generated
marketplace keeps `source.path` local for wrapper metadata and points artifact
URLs at the external release repository.

External plugin repositories own their release pipeline. They must publish
`<name>-plugin-<target>.zip` plus `<name>-plugin-<target>.zip.sha256` for each
marketplace target under the release tag used by the generated index.
For the public Elegy marketplace, those assets live on the public Elegy release
even when the producing source repository is private.

External wrapper surfaces may set `marketplacePublished: false` while archives
are not yet public. Draft wrappers stay in source, but the generated
marketplace omits them until the public archives exist.

## Install

Plugin-packaged surfaces install via `elegy-plugin-packaging install`:

```bash
elegy-plugin-packaging install --archive elegy-planning-plugin-x86_64-pc-windows-msvc.zip
```

Non-plugin surfaces install via `scripts/install-distribution.sh`:

```bash
# Legacy flat binary install (non-plugin surfaces only)
bash ./scripts/install-distribution.sh --tag vX.Y.Z --destination ./tools/elegy --surface elegy-codegraph --force
```

Plugin-packaged surfaces should use `elegy-plugin-packaging install` as the
primary install lane.

Marketplace consumers use the generated static index:

```bash
elegy-plugin-packaging marketplace list --source . --json
elegy-plugin-packaging marketplace install elegy-planning --source .
```

The same `--source` contract accepts an HTTPS base URL, so Holon and other
consumers are not tied to this repository. Remote archives require SHA-256
sidecars and are checked against the public wrapper manifest before install.

Private-source plugins may publish public proprietary binaries. Their wrapper
metadata, skills, scripts, and descriptors are public. Keep private behavior in
the compiled binary or behind a hosted service; hosts own all credentials and
OAuth state.

To install a surface, the surface must exist in the release assets and have a published `.sha256` sidecar. The installer verifies the downloaded asset SHA-256 before writing the executable into the destination `bin/` directory.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption. Workflow artifacts are a maintainer/CI convenience, not the primary handoff lane.
- Pin an explicit Elegy semver release tag in downstream repositories and install into a repo-local tools directory.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Keep any host-specific runtime/bootstrap behavior in the consuming repository. Elegy owns the contracts, the binaries, and the generic installer; the consuming repo owns product wiring.
- Use `cargo add elegy-plugin-sdk` for external plugin repos that need plugin types, validation, packaging, and export.
- Prefer plugin archives over flat binaries for plugin-packaged surfaces. Binary-backed archives carry the manifest, skills, and built binary in a single verifiable artifact. Skill-only archives use `target: "any"` and omit `bin/`.
- Do not reintroduce NuGet or GitHub Packages as the primary downstream lane.
- Treat the rolling `main-snapshot` prerelease as an integration/debug lane, not a pinned downstream contract.

## Where to read more

- Release publishing: `.github/workflows/publish-orchestrator.yml`
- Release finalize: `.github/workflows/release-finalize.yml`
- Installer/bootstrap artifacts: `.github/workflows/distribution-artifacts.yml`
- Authority surfaces: [`docs/architecture/ecosystem-topology.md`](./architecture/ecosystem-topology.md).
