# Memory MCP authentication

`elegy-memory-mcp` is never an OAuth authorization server.

## Security boundaries

1. The MCP host and external identity provider own user login, OAuth discovery,
   client registration, consent, tokens, and refresh.
2. The Memory HTTP binary is only an OAuth resource server: it validates the
   external issuer, audience, expiry, signature, and required scopes.
3. Host approvals and Memory write policy remain separate from bearer
   authentication. Evidence for one boundary proves nothing about another.

The shipping binary has no authorization, token, dynamic-client-registration,
or consent endpoints and stores no OAuth client secret, signing key, access
token, or refresh token.

## Modes

`ELEGY_MCP_AUTH_MODE=local-none` is accepted only when
`ELEGY_MCP_BIND` is loopback. It is intended for an explicitly local HTTP
boundary.

`ELEGY_MCP_AUTH_MODE=external-oauth` requires:

- `ELEGY_MCP_PUBLIC_URL`
- `ELEGY_MCP_OAUTH_ISSUER`
- `ELEGY_MCP_OAUTH_AUDIENCE`
- `ELEGY_MCP_OAUTH_JWKS_URL`
- `ELEGY_MCP_OAUTH_SCOPES`

All external URLs must use HTTPS and contain no credentials, query, or
fragment. Startup fetches the configured JWKS with a bounded, no-redirect
client and fails unless it contains unique, usable ES256 or RS256 public
signature keys. Token algorithms must exactly match the declared JWK
algorithm. A rate-limited refresh is attempted when a previously unknown key
ID appears.
Requests to `/mcp` without a valid token return `401` and an MCP
protected-resource challenge. The only OAuth metadata published by Memory is
`GET /.well-known/oauth-protected-resource`, which delegates to the configured
issuer.

Local stdio uses the subprocess boundary and has no OAuth layer.

## Evidence limit

Source tests cover the local guard, HTTPS configuration, asymmetric key policy,
JWKS rotation, external JWT signature/issuer/audience/expiry/scope checks, the
MCP bearer challenge, and absence of authorization-server endpoints. These are
implementation evidence only. They are not an authenticated clean-install or
real-consumer receipt.
