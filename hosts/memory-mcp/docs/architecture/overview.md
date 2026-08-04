# Memory MCP architecture

```text
local host --stdio--> memory host adapter ----+
                                               +--> shared Memory tools --> SQLite
remote host --OAuth bearer--> HTTP resource --+
                    ^
                    |
          external identity provider
```

The identity provider owns authorization. The remote host performs OAuth and
holds tokens. The HTTP adapter fetches public JWKS metadata and validates
signature, issuer, audience, expiry, and required scopes. It publishes MCP
protected-resource metadata but no authorization-server metadata or endpoints.

The stdio adapter has no network authentication layer. Both adapters reuse the
same repository binding and cannot change Memory product policy or accept
caller-selected namespaces.

## Stdio memory visibility

The stdio adapter reads the scopes visible from the server-side
`ELEGY_MCP_READ_SCOPE` setting, defaulting to `session`, the widest range. It
also includes memories without an `agent_id`, which is how the local
`elegy-memory` CLI writes shared knowledge.

Read breadth never widens writes: mutations require a memory in the configured
agent scope. The setting is server-side and cannot be overridden by an MCP
request.
