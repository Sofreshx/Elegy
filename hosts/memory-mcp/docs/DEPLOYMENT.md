# Memory HTTP MCP deployment

The HTTP binary is implemented but not evidenced as usable. There is no
reviewed clean-installed authenticated deployment or real MCP consumer receipt.

## Local-only HTTP

```powershell
$env:ELEGY_MCP_AUTH_MODE = "local-none"
$env:ELEGY_MCP_DB_PATH = "C:\Elegy\data\elegy-memory.db"
$env:ELEGY_MCP_BIND = "127.0.0.1"
elegy-memory-mcp-http.exe
```

The process refuses `local-none` on a non-loopback address.

## Authenticated remote resource server

Provision an OAuth/OIDC authorization server independently. The agent host
performs login and stores its tokens. Configure Memory only with public
validation metadata:

```powershell
$env:ELEGY_MCP_AUTH_MODE = "external-oauth"
$env:ELEGY_MCP_DB_PATH = "C:\Elegy\data\elegy-memory.db"
$env:ELEGY_MCP_BIND = "0.0.0.0"
$env:ELEGY_MCP_PUBLIC_URL = "https://memory.example.com/"
$env:ELEGY_MCP_OAUTH_ISSUER = "https://identity.example.com/"
$env:ELEGY_MCP_OAUTH_AUDIENCE = "https://memory.example.com/mcp"
$env:ELEGY_MCP_OAUTH_JWKS_URL = "https://identity.example.com/.well-known/jwks.json"
$env:ELEGY_MCP_OAUTH_SCOPES = "memory.read,memory.write"
elegy-memory-mcp-http.exe
```

Expose `/mcp` and `/.well-known/oauth-protected-resource`. Do not configure
routes for `/oauth/token`, `/oauth/register`, `/oauth/authorize`, or
`/.well-known/oauth-authorization-server`; the shipping server does not provide
them.

Startup fails when authentication configuration or JWKS validation is missing.
A `401` challenge points the host to Memory's protected-resource metadata,
which in turn names the external issuer.
