# Contracts Authority

This directory is the authored, language-agnostic authority root for governed Elegy contract assets.

Use it for:

- schemas under `contracts/schemas`
- fixtures under `contracts/fixtures`
- compatibility and bundle manifests under `contracts/manifests`
- consumer support manifests under `contracts/support`

Do not treat `artifacts/contracts` as the authored source of truth. That directory is generated output for consumers and CI.

During the .NET purge, assets may still exist under `src/` for compatibility, but new ownership should move here first.