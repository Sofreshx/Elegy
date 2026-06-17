# Contracts Authority

This directory is the authored, language-agnostic authority root for governed Elegy contract assets.

Use it for:

- schemas under `contracts/schemas`
- fixtures under `contracts/fixtures`


The portable plugin package contract, `elegy-plugin-package/v1`, also lives
here. It describes bundle metadata and component references for consuming
hosts; it does not create an Elegy-hosted plugin runtime.

Do not treat `artifacts/contracts` as the authored source of truth. That directory is generated output for consumers and CI.

Current governed schemas include:
- `elegy-codegraph.graph.v0.json` — Normalized graph IR for elegy-codegraph (entities, edges, provenance, confidence)

All other authored contract assets live here.
