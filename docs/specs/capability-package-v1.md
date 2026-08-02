---
title: Capability package v1
status: active
owner: elegy-tooling
doc_kind: spec
---

# Capability package v1

`elegy-package/v1` is the host-neutral package manifest. It is stored at
`elegy-package.json` beside the declared executable, catalog, readiness
artifact, and optional skills.

The manifest owns package name and SemVer, publisher/repository identity,
supported targets, executable or native-MCP entrypoints, capability catalog
path, readiness path, optional skill paths, and the complete packaged-file
list. Every referenced file must be declared, regular, and package-relative.
Publishable packages carry a SHA-256 for every declared file; `pack` fills in
missing digests before creating the archive. Optional release provenance binds
the package to a source commit, build workflow, and builder identity.
Archives are generated in stable manifest-then-sorted-file order.

Use the unified authoring flow:

```text
elegy init --name my-tool --output ./my-tool
elegy check --package ./my-tool
elegy test --package ./my-tool
elegy pack --package ./my-tool --output ./dist/my-tool.zip
elegy project --package ./my-tool --host mcp --output ./dist/mcp
```

`elegy pack` writes a deterministic `<archive>.sbom.json` sidecar containing
the archive digest, package identity, provenance, and per-file digests. A
projection created with `--lock` must point at an installed package whose
receipt verifies against that exact lock; it copies the lock and receipt into
the projection.

Use `elegy install --update` for an integrity-checked atomic replacement. If
publication fails, the old installation is restored. Use `elegy uninstall`
with the same lock to remove only a verified package directory.

The generated scaffold is intentionally not publishable evidence: the author
must replace its placeholder publisher, catalog, readiness, skill, and
executable content and earn `usable` or `production` readiness.

The package manifest is not a Codex manifest. Codex, MCP, Holon, and shell
outputs are projections from this authority.
