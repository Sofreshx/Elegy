# Contracts Authority

This directory is the authored, language-agnostic authority root for governed Elegy contract assets.

Use it for:

- schemas under `contracts/schemas`
- fixtures under `contracts/fixtures`
- compatibility and bundle manifests under `contracts/manifests`
- consumer support manifests under `contracts/support`

Portable plugin package contracts, such as `elegy-plugin-package/v1` and
`elegy-plugin-package/v2`, also live here. They describe bundle metadata and
component references for consuming hosts; they do not create an
Elegy-hosted plugin runtime.

Do not treat `artifacts/contracts` as the authored source of truth. That directory is generated output for consumers and CI.

No authored contract asset should live under a legacy implementation tree. If a governed schema, fixture, or manifest is kept, it belongs here or under `governance/`.
