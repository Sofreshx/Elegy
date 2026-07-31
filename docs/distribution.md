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
| Codex marketplace projection | `elegy-codex-marketplace-<target>.zip` | Generated Codex-native marketplace tree containing `.agents/plugins/marketplace.json`, plugin projections, skills, companion files, and target binaries. |
| Local pack default | `<plugin-name>-v<version>.plugin.zip` | Ad hoc output from `elegy-plugin-packaging pack` when `--output` is omitted. Not the GitHub release naming contract. |
| CLI asset | `<name>-<target>[.exe]` | Per binary surface and target, resolved through distribution/surfaces.json. Plugin-packaged surfaces bundle this with skills in plugin archives. |
| CLI asset checksum | `<name>-<target>[.exe].sha256` | Sidecar checksum used by the installer. |

## Surface Catalog

Release configuration uses `distribution/surfaces.json` as the central catalog.
It declares `schemaVersion: "elegy-surfaces/v3"` and maps workspace crates and
surfaces to their release identities, product class, lifecycle, build targets,
and disposition. The
publish orchestrator reads this catalog to discover which surfaces to build and
release.

To add a new release surface, add an entry to `distribution/surfaces.json` and ensure the crate builds. No per-feature workflow files are needed.

`kind` controls build mechanics:

| Kind | Contract |
| --- | --- |
| `bundled-plugin` | Installable adapter package with a Rust runtime and optional skills plus declared CLI, MCP-resource, or MCP-tool interfaces. |
| `cli` | Standalone CLI under `tools/` or `shared/`, not a host adapter. |
| `host-adapter` | Host or transport surface under `hosts/`. |
| `skill-package` | Standalone skill source under `skills/elegy-*`; not an Elegy plugin class. |
| `external-plugin-wrapper` | Historical external metadata retained for inspection; not published unless it independently qualifies as an active adapter plugin. |

`surfaceClass` states what the surface is: `adapter-plugin`, `tool`, `skill`,
`host-adapter`, or `host-extension`. `lifecycle` is `active`, `rework`,
`deprecated`, or `blocked`. Only an active `adapter-plugin` may set
`packaging: "plugin"`, and it must declare a typed capability catalog.

Binary surfaces may declare a `targets` array of supported Rust target triples.
When omitted, the publisher uses the default Windows, Linux, and macOS matrix;
skill sources are not emitted as Elegy plugin archives.

The former reusable external-wrapper publication workflow is removed. External
projects own their own releases; they enter Elegy only after the concrete
adapter independently satisfies the active plugin boundary and readiness
requirements. Public metadata cannot substitute for runtime proof.

## Install

Plugin-packaged surfaces install via `elegy-plugin-packaging install`:

```bash
elegy-plugin-packaging install --archive elegy-accounts-plugin-x86_64-pc-windows-msvc.zip
```

Non-plugin surfaces install via `scripts/install-distribution.sh`:

```bash
# Legacy flat binary install (non-plugin surfaces only)
bash ./scripts/install-distribution.sh --tag vX.Y.Z --destination ./tools/elegy --surface elegy-codegraph --force
```

Plugin-packaged surfaces should use `elegy-plugin-packaging install` as the
primary install lane.

Portable archives and host projections retain the typed capability catalog,
readiness artifact and receipts, and connection descriptors referenced by the
manifest. Packaging must not leave those authorities behind in the source
checkout.

Marketplace consumers use the generated static index:

```bash
elegy-plugin-packaging marketplace list --source . --json
elegy-plugin-packaging marketplace install elegy-accounts --source .
```

The same `--source` contract accepts an HTTPS base URL, so Holon and other
consumers are not tied to this repository. Remote archives require SHA-256
sidecars and are checked against the public wrapper manifest before install.

Codex consumers use the generated Codex marketplace projection:

```bash
codex plugin marketplace add <CODEX_HOME>/marketplaces/elegy --json
codex plugin add elegy-planning@elegy --json
codex plugin list --marketplace elegy --available --json
```

Downstream Codex apps should consume `elegy-codex-marketplace-<target>.zip`
and install through Codex plugin commands. Do not copy shared skills as the
primary Codex route for Elegy plugins. Shared skills are compatibility assets
when plugin installation is unavailable or disabled; they are not capability
authority. The v1 catalog `fallback` field has no active runtime consumer.

Private-source plugins may publish public proprietary binaries. Their wrapper
metadata, skills, scripts, and descriptors are public. Keep private behavior in
the compiled binary or behind a hosted service; hosts own all credentials and
OAuth state.

To install a surface, the surface must exist in the release assets and have a published `.sha256` sidecar. The installer verifies the downloaded asset SHA-256 before writing the executable into the destination `bin/` directory.

## Downstream guidance

- Prefer GitHub release assets for downstream consumption. Workflow artifacts are a maintainer/CI convenience, not the primary handoff lane.
- Pin an explicit Elegy semver release tag in downstream repositories and install into a repo-local tools directory.
- Use `main-snapshot` only for latest-branch integration, validation, and debugging.
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
