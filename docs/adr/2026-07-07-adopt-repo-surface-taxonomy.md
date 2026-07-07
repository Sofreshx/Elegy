---
title: Adopt Repo Surface Taxonomy
status: accepted
owner: elegy-core
---

# Adopt Repo Surface Taxonomy

## Status

Accepted.

## Context

Elegy ships several surface types from one repository: bundled plugins,
standalone CLI crates, host adapters, skill-only packages, marketplace wrappers,
and shared libraries. The current tree places multiple surface types under
`plugins/`, which makes ownership, release behavior, and agent navigation
ambiguous.

The release catalog already distinguishes several surface kinds in
`distribution/surfaces.json`. The filesystem should expose the same taxonomy.

## Decision

Elegy will use top-level directories that match surface roles:

| Surface role | Directory |
| --- | --- |
| Bundled installable plugin packages | `plugins/` |
| Standalone CLI crates | `tools/` |
| Host adapters and transport servers | `hosts/` |
| Standalone skill packages | `skills/` |
| External/private plugin wrappers | `marketplace-wrappers/` |
| Reusable Rust libraries | `shared/` |

`distribution/surfaces.json` remains the canonical release catalog. Plugin
manifests remain the canonical package metadata for installable plugin roots.

## Consequences

- Existing directories under `plugins/` need migration into the role-specific
  roots.
- Validation can check directory kind, catalog entries, and local artifact
  hygiene without inferring intent from names.
- README and architecture docs can route contributors to one authoritative
  layout document instead of repeating policy.
- Marketplace wrappers do not need fake implementation files to look like local
  plugins.

## Validation

Run:

```powershell
pwsh scripts/check-repo-shape.ps1 -Project . -Json
cargo metadata --format-version 1 --no-deps
```

After migration, run the shape checker with `-FailOnIssues` in CI.
