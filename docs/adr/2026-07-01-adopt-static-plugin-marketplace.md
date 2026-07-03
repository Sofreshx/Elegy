---
title: Adopt a static plugin marketplace
status: accepted
date: 2026-07-01
owner: Elegy
---

# Adopt a static plugin marketplace

## Decision

Elegy distributes marketplace metadata as one static `elegy-marketplace/v1`
JSON document. The format follows the Codex marketplace shape:

```text
.elegy/marketplace.json
  -> ordered local plugin references
    -> .elegy-plugin/plugin.json
    -> optional target archives
```

Plugin manifests own package identity and version. `distribution/surfaces.json`
owns listing order, category, and release routing. The checked-in marketplace
index is generated from those sources.

Compiled plugins publish one archive and SHA-256 sidecar per supported target.
Private source code may produce public proprietary binaries. Files shipped in
the wrapper or archive remain readable.

## Consequences

- Consumers need only a local or HTTPS marketplace root.
- Holon and other hosts use the same SDK types and JSON output.
- Codex marketplace files remain derived host projections.
- V1 has no service, account system, dependency solver, or authenticated feed.
- Hosts continue to own credentials, OAuth state, approvals, and execution policy.

