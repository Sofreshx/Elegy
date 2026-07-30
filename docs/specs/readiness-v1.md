---
title: Evidence-backed readiness v1
status: active
owner: elegy-core
created: 2026-07-30
---

# Evidence-backed readiness v1

## Authority

Each distributed surface owns one JSON artifact with `schemaVersion: elegy-readiness/v1`. Plugin manifests reference it through `readiness`; standalone entries reference it from `distribution/surfaces.json`.

Required fields are:

- surface identity and version;
- one of `concept`, `implemented`, `usable`, or `production`;
- an honest summary;
- non-empty `worksToday`, `limitations`, and `supportedEnvironments`;
- exact installation and invocation posture;
- typed evidence references.

The generated schema is `shared/plugin-sdk/schemas/elegy-readiness-v1.schema.json`.

## Promotion

| Target | Evidence rule |
|---|---|
| `implemented` | At least one `source-tests` and one `package-verification` receipt. |
| `usable` | Implemented evidence plus `clean-install` and a `real-task` receipt marked `nonFixture: true`. |
| `production` | Usable evidence plus `release` and `consumer` receipts. |

Evidence paths are package-relative and must exist. Manifest stage, artifact stage, surface name, and surface version must match.

Mocks, fixtures, generated files, source compilation, schema validation, and archive construction can support `implemented`; they cannot satisfy `clean-install`, `real-task`, `release`, or `consumer`.

## Routing

- Missing readiness: backward-compatible `implemented`, non-routable.
- `concept` and `implemented`: non-routable.
- `usable` and `production`: routable only when the artifact validates.
- Default marketplace and Codex projections omit non-routable surfaces.
- `--include-incubating` and `--allow-incubating` are maintainer overrides, not promotions.

## Documentation

`docs/readiness.md` is generated deterministically from the catalog and readiness artifacts. Documentation validation fails for:

- missing or invalid readiness authority;
- manifest/artifact mismatch;
- matrix drift;
- a non-routable skill that does not identify itself as not agent-routable.

Surface documentation must state current stage, what works today, limitations, exact source/install invocation, and whether providers or external consumers were exercised. Claims of usability or production must point to the qualifying receipts.

## Acceptance

A default-discovery consumer must be unable to route to a surface below `usable`, including when a valid schema, fixture, package archive, or host projection exists.
