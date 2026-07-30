---
title: Adopt Codex-parity packages and delegated authentication
status: accepted
owner: Elegy
created: 2026-07-30
---

# Adopt Codex-parity packages and delegated authentication

## Context

Elegy's v1/v2 abstraction renamed or omitted native Codex package behavior and
then described the partial projection as compatibility. Authentication was
also blurred across MCP OAuth, upstream provider credentials, and host
approvals. Memory even shipped a bespoke authorization server.

## Decision

`elegy-plugin/v3` is the only publishable plugin manifest. Its top level uses
Codex-native package fields and accepted shapes. Elegy-only governance lives
under `elegy`. Codex projection removes exactly `schemaVersion` and `elegy`;
all other fields round-trip structurally, including unknown native fields.

“Plugin” has two meanings:

- a plugin package is a Codex-compatible distribution envelope;
- an Elegy adapter plugin is a routable connector to an external system,
  source, API, application, OS boundary, or executable.

A library, business product, skill, hook, app, or MCP transport may be carried
by the package envelope without becoming an Elegy adapter plugin.

Authentication has three independent boundaries:

1. host to remote MCP;
2. local adapter to upstream provider;
3. host approvals and side-effect policy.

Remote MCP OAuth belongs to the MCP server's identity provider and the host.
Elegy records an expected mode (`none`, `mcp-oauth`, or `bearer-env`) but
stores no OAuth endpoint, client secret, or token. Local stdio adds no OAuth.
Accounts may broker upstream provider credentials for local adapters; it does
not replace MCP OAuth.

Memory HTTP is an OAuth resource server only. Its former authorization,
consent, DCR, signing-key, and token-store implementation is removed from the
shipping binary.

Marketplace v2 carries explicit Codex-equivalent install and authentication
policies and describes local, Git, Git-subdirectory, npm, and Elegy artifact
sources. Git and npm variants remain descriptor-only and `NOT_AVAILABLE` until
materialization exists. v1/v2 plugin manifests remain readable legacy inputs
but cannot be exported, newly generated, published, or discovered by default.

Non-Codex projection fails when the target cannot represent package fields,
connections, or authentication. `--allow-lossy` writes a machine-readable loss
report and marks the result non-routable.

## Consequences

- Schema validity and fixture OAuth remain implementation evidence only.
- Codex manifest projection is deterministic and structurally preserving for
  validated known fields. Runtime parity remains unproven until a clean Codex
  receipt exercises the package.
- Other hosts cannot silently drop apps, hooks, UI, connections, or auth.
- Accounts' Google pack may claim implemented PKCE/refresh/revocation behavior
  from fixtures, but no authenticated usability until a reviewed live receipt.
- OpenAI-hosted review, organization administration, universal directory, and
  hosted marketplace services remain out of scope.
