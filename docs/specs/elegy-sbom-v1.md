---
title: Elegy SBOM v1
status: active
owner: elegy-tooling
doc_kind: spec
---

# Elegy SBOM v1

`elegy-sbom/v1` is a deterministic sidecar for one packaged archive. It binds
the package name, exact version, publisher, archive SHA-256, every packaged
file's role and SHA-256, and any release provenance declared by the package.
The sidecar is evidence about artifact contents; it does not replace the
package manifest or the reviewed `elegy-lock/v1`.

The unified CLI writes `<archive>.sbom.json` by default and accepts
`--sbom-output` for an explicit path. It is safe to store the sidecar beside a
release archive and review it together with the lock update.
