---
title: Plugin marketplace v2
status: active
owner: Elegy
---

# Plugin marketplace v2

The canonical index is `.elegy/marketplace.json` with
`schemaVersion: elegy-marketplace/v2`.

Each entry declares its source and explicit policies:

- source: local, Git, Git subdirectory, npm, or Elegy artifact;
- install policy: `NOT_AVAILABLE`, `AVAILABLE`, or
  `INSTALLED_BY_DEFAULT`;
- authentication policy: `ON_INSTALL` or `ON_USE`.

The release catalog declares these values. Generation preserves the declared
authentication policy and may only reduce installation to `NOT_AVAILABLE`
when readiness is not routable. No hardcoded `AVAILABLE`/`ON_INSTALL` fallback
is permitted.

Only active `adapter-plugin` surfaces at `usable` or `production` readiness
enter default generation, listing, search, install, or export. An empty index
is valid. Explicit incubating inspection never changes readiness or
routability.

Local entries resolve to a verified v3 manifest under the marketplace root.
Git, Git-subdirectory, and npm declarations are descriptor-only in the current
implementation. Entries using them must declare installation `NOT_AVAILABLE`;
listing, installation, and export never imply a materializer exists. Promotion
to `AVAILABLE` requires implemented materialization, verification, and an
installed-task receipt. Elegy artifacts require target selection, checksum
verification, and archive identity matching.

Remote marketplace indexes are also non-routable in default discovery until
their selected archive has been materialized and verified locally. Fetching a
remote manifest alone does not establish package verification.

v1 marketplace roots and v1/v2 plugin manifests remain readable for migration.
They are not publishable.

Codex marketplace export must preserve entry order, category, install policy,
authentication policy, and the lossless v3 package. Authentication remains a
host lifecycle separate from installation.

Commands:

```powershell
elegy-plugin-packaging marketplace generate --project .
elegy-plugin-packaging marketplace validate --source .
elegy-plugin-packaging marketplace list --source . --json
elegy-plugin-packaging marketplace export-codex --source . --output ./dist/codex
```

Validation:

```powershell
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project . --check
cargo test -p elegy-tooling
```
