# elegy-memory-mcp OAuth 2.1 + MCP Bearer Enforcement (WU4/WU5)

This auth model applies to `elegy-memory-mcp-http` only; `elegy-memory-mcp-stdio` does not use OAuth or JWT.

## Fixed behavior

- Scope is fixed to `claude-ai-remote`.
- Auth server and resource server live in the same binary.
- Public clients only: `token_endpoint_auth_method = "none"`, `client_id` is a UUID, no client secret.
- Redirect allowlist:
  - `https://claude.ai/...`
  - `https://claude.com/...`
  - `http://127.0.0.1:<port>/...`
  - `http://localhost:<port>/...`
- Access tokens are HS256 JWTs with a 1 hour lifetime.
- Refresh tokens are random 32-byte values, persisted as hashes, valid for 30 days, and rotated on refresh.
- Dynamic client registrations, refresh tokens, and the signing key are stored under `ELEGY_MCP_DATA_DIR`.
- `ELEGY_MCP_ADMIN_PASSWORD` is the consent-page password itself; startup hashes it with Argon2 for in-memory verification and rejects pre-hashed Argon2 strings.
- `POST /mcp` requires `Authorization: Bearer <jwt>` with an HS256 signature, non-expired `exp`, and `scope = "claude-ai-remote"`.
- Missing or invalid `/mcp` bearer tokens return `401` with `WWW-Authenticate: Bearer realm="elegy-mcp", resource_metadata="<PUBLIC_URL>/.well-known/oauth-protected-resource"`.

## Endpoints

- `GET /.well-known/oauth-protected-resource`
- `GET /.well-known/oauth-authorization-server`
- `POST /oauth/register`
- `GET /oauth/authorize`
- `POST /oauth/authorize`
- `POST /oauth/token`
- `POST /mcp` (bearer required)

## Flow

```text
Claude -> POST /oauth/register
Claude -> GET /oauth/authorize?response_type=code&scope=claude-ai-remote&code_challenge=...
User   -> POST /oauth/authorize (password form)
Server -> 302 redirect_uri?code=...&state=...
Claude -> POST /oauth/token (authorization_code + PKCE verifier)
Claude -> POST /oauth/token (refresh_token) later, with rotation
```

## Example

```bash
curl -X POST "$ELEGY_MCP_PUBLIC_URL/oauth/register" \
  -H "content-type: application/json" \
  -d '{"redirect_uris":["https://claude.ai/callback"],"token_endpoint_auth_method":"none"}'

curl "$ELEGY_MCP_PUBLIC_URL/.well-known/oauth-authorization-server"

curl -X POST "$ELEGY_MCP_PUBLIC_URL/oauth/token" \
  -H "content-type: application/x-www-form-urlencoded" \
  --data-urlencode "grant_type=refresh_token" \
  --data-urlencode "client_id=<uuid>" \
  --data-urlencode "refresh_token=<token>" \
  --data-urlencode "scope=claude-ai-remote"
```

## Persistence files

- `{DATA_DIR}/signing-key`
- `{DATA_DIR}/clients.json`
- `{DATA_DIR}/refresh-tokens.json`

None of these files should be committed.
