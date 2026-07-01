# Elegy distribution

This file ships inside downloadable Elegy archives as the archive-level `README.md`.

Elegy is distributed through GitHub release assets, not package feeds or sibling-repo workspace references. Each zip is intentionally self-describing: the archive contains its payload plus this README.

## Distribution model

- **Contracts bundle**: governed schemas, fixtures, and compatibility metadata. The canonical machine-readable handoff.
- **CLI assets**: executable distributions for each binary surface, per published target.
- **Wrapper archives**: bounded repo-local integration surfaces for dedicated tools.
- **Installer bootstrap**: the generic install helper packaged as its own downloadable archive.
- **Release metadata**: manifest + checksums used by installer and validation flows.

## Asset family conventions

- `elegy-<surface>-<target>` — per-binary CLI asset for Linux/macOS
- `elegy-<surface>-<target>.exe` — per-binary CLI asset for Windows
- `elegy-<surface>-<target>[.exe].sha256` — SHA-256 sidecar for each CLI asset
- `elegy-contracts-bundle.zip` — governed contracts bundle
- `elegy-installer-<bundleVersion>.zip` — standalone installer bootstrap
- `elegy-release-manifest-<tag>.json` — release manifest (required)
- `elegy-release-checksums-<tag>.json` — release checksums (required)

`<surface>` and `<target>` values, exact archive family names, and per-binary
install steps are owned by each binary's per-feature distribution note
(`<crate>/DISTRIBUTION.md`). This README stays thin on purpose; adding a new
binary does not require editing it.

## Targets and release lane

- Published CLI targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`
- Stable semver tags (e.g. `v1.3.2`) are the supported downstream contract.
- `main-snapshot` is a rolling prerelease for latest-branch validation.

## Where to read more

- Repository: https://github.com/Sofreshx/Elegy
- Main user-facing guide: `README.md`
- Distribution index (release channels, targets, per-binary link list): `docs/distribution.md`
- Per-binary distribution note: each binary's `<crate>/DISTRIBUTION.md`
- Generic installer (Bash, canonical): `bash ./scripts/install-distribution.sh --tag <releaseTag> --destination <path> --surface <surface> [--force]`
- Generic installer (PowerShell, thin shim that maps PowerShell args to bash; requires bash in PATH): `pwsh ./scripts/install-distribution.ps1 -Tag <releaseTag> -Destination <path> -Surface <surface> [-Force]`

The installer downloads one surface-specific release asset plus its `.sha256`
sidecar, verifies the SHA-256, and writes the executable into `bin/`.
