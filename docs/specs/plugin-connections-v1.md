---
title: Plugin connections and authentication
status: active
owner: Elegy
---

# Plugin connections and authentication

Authentication is not one boundary.

| Boundary | Owner | Elegy responsibility |
|---|---|---|
| host to remote MCP | MCP server identity provider and agent host | Declare expected mode and validate representability; never store endpoints, client secrets, or host tokens. |
| local adapter to upstream API | Codex app binding or optional Elegy Accounts broker | Declare connection requirement; keep credentials outside manifests/model context. |
| host approvals and side effects | Agent host and adapter policy | Preserve annotations/policy; never claim authentication proves approval. |

`elegy-plugin/v3` records connection requirements and per-server expected MCP
authentication under `elegy`. Modes are `none`, `mcp-oauth`, and `bearer-env`.
Inline HTTP MCP declarations without a mode are invalid. Remote unauthenticated
configurations are invalid.

For `mcp-oauth`, the host follows MCP protected-resource discovery and the
remote server's challenge. Elegy does not project OAuth endpoints, manufacture
a client, or receive tokens. Local stdio normally declares `none`; it does not
gain a second OAuth layer.

Codex-native apps remain native top-level package fields and use Codex's
connection behavior. Other hosts must represent the same binding or reject
projection. Accounts is an optional broker for credentials used by local
adapters, not an MCP authorization server.

The capability-catalog v1 `app-binding` kind is legacy metadata, not a
connection authority. It is meaningful only when a native Codex app connection
binding is explicitly present; a service slug alone never authorizes routing.

Credentials, bearer values, authorization headers, codes, refresh tokens,
cookies, signing keys, and client secrets are forbidden in package manifests,
skills, catalogs, loss reports, and receipts.

Readiness documents for adapters must report four facts separately:

1. host authentication exercised;
2. upstream authentication exercised;
3. refresh exercised;
4. provider revocation exercised.

Fixture OAuth, local grant revocation, or schema conformance cannot be called
authenticated usability.
