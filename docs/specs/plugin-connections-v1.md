---
title: Plugin Connections V1
status: active
owner: Elegy
---

# Plugin Connections V1

## Purpose

Authentication is a host lifecycle, not an LLM task. `elegy-plugin/v2`
requires every plugin to state whether it needs managed connections, and keeps
credential acquisition, storage, refresh, verification, and revocation outside
skills, CLI arguments, MCP tool inputs, and model context.

## Manifest declaration

Every `elegy-plugin/v2` manifest has a `connections` field.

Connectionless plugins declare:

```json
{
  "connections": {
    "requirements": { "mode": "none" }
  }
}
```

Connected plugins reference a governed requirements file:

```json
{
  "connections": {
    "requirements": {
      "mode": "declared",
      "path": "./connections.json",
      "schemaVersion": "elegy-plugin-connections/v1"
    }
  }
}
```

`elegy-plugin-connections/v1` binds the requirements to the plugin name and
version. Each requirement has a stable plugin-local `id`, portable `service`
identity, `required` flag, and user-facing description.

## Host bindings

Portable service identities are not host connector IDs. A Codex projection
must explicitly map every declared requirement in
`extensions["codex.plugin/v1"].connectionBindings`:

```json
{
  "connectionBindings": {
    "github-main": {
      "id": "connector_76869538009648d5b282a4bb21c3d157"
    }
  }
}
```

The exporter emits that opaque registered ID and the requirement's `required`
state into `.app.json`. It never manufactures an ID from `github`, `gmail`, or
another service slug. Plugin installation and app connection remain separate
host operations; Codex owns OAuth and connection state for Codex apps.

The catalog-driven v1 `appBinding.connector -> .app.json id` projection remains
read-compatible only for `elegy-plugin/v1`. Marketplace publication requires
`elegy-plugin/v2`.

## Connection providers

A plugin may additionally implement a host-neutral connection provider:

```json
{
  "connections": {
    "requirements": { "mode": "none" },
    "provider": {
      "path": "./connection-provider.json",
      "schemaVersion": "elegy-connection-provider/v1"
    }
  }
}
```

The provider descriptor declares an ID, the
`elegy-connection-control/v1` protocol, and a CLI invocation. The control
protocol supports list, connect-session status, verify, disconnect preview, and
confirmed disconnect operations. Requests are client-bound, signed, replay
protected, and credential-free. Responses expose explicit connection states
and sanitized account summaries, never tokens, cookies, authorization codes,
or refresh material.

Elegy Accounts is the first provider. A Holon integration embeds or launches
the provider's human authentication UI, calls the signed control protocol, and
stores only opaque connection references and status. Holon does not parse
provider credentials or ask an LLM to perform authentication.

## Lifecycle and safety

Connection states are `disconnected`, `connecting`, `connected`, `stale`,
`attention-required`, `unavailable`, and `error`. A plugin is ready only when
all required connections are verified as `connected`.

Disconnect is destructive and therefore uses preview plus execute with a
confirmation digest. Provider-owned credentials remain encrypted in the
provider vault; host-owned Codex credentials remain in Codex.

## Validation

Verification fails when a v2 plugin omits its connection posture, a referenced
requirements/provider file is missing or invalid, plugin identity does not
match, a Codex binding is missing or extra, or a binding uses a service slug in
place of an opaque registered ID.

