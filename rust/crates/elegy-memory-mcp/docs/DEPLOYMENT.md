# elegy-memory-mcp deployment

This guide covers the HTTP/OAuth binary; local stdio consumers use subprocess config instead of tunnel deployment.

## What you are deploying

`elegy-memory-mcp-http` exposes public OAuth metadata and auth endpoints, but keeps `/mcp` behind bearer auth.

- Public:
  - `/.well-known/oauth-protected-resource`
  - `/.well-known/oauth-authorization-server`
  - `/oauth/register`
  - `/oauth/authorize`
  - `/oauth/token`
- Protected:
  - `/mcp`

## DNS prerequisite

`holon.it.com` must be managed in Cloudflare DNS before you create the tunnel route.

- If the zone is already in Cloudflare, continue.
- If not, add the zone in Cloudflare and update the domain's nameservers at the registrar to the Cloudflare-assigned NS set first.
- Wait for the NS change to become active before running `cloudflared tunnel route dns`.

## Install `cloudflared` on Windows

Install the Cloudflare Tunnel client first.

```powershell
winget install Cloudflare.cloudflared
```

Then sign in and create the tunnel:

```powershell
cloudflared tunnel login
cloudflared tunnel create elegy-memory
cloudflared tunnel route dns elegy-memory elegy-memory.holon.it.com
```

## Tunnel config

Point the public hostname at the local HTTP MCP server on port `8765`.

File: `C:\Users\<you>\.cloudflared\config.yml`

```yaml
tunnel: <TUNNEL_ID>
credentials-file: C:\Users\<you>\.cloudflared\<TUNNEL_ID>.json

ingress:
  - hostname: elegy-memory.holon.it.com
    service: http://localhost:8765
  - service: http_status:404
```

## Environment variables

Set the runtime env vars before starting the binary or installing services.

```powershell
setx ELEGY_MCP_ADMIN_PASSWORD "replace-with-consent-password"
setx ELEGY_MCP_DB_PATH "C:\Elegy\data\elegy-memory.db"
setx ELEGY_MCP_PUBLIC_URL "https://elegy-memory.holon.it.com"
setx ELEGY_MCP_PORT "8765"
setx ELEGY_MCP_DATA_DIR "C:\Elegy\data\mcp"
setx ELEGY_MCP_LOG_CONTENT "0"
```

Open a new PowerShell session after `setx` so new processes inherit the values.

## Manual run

Start the local HTTP server and tunnel in separate terminals.

```powershell
cloudflared tunnel run elegy-memory
```

```powershell
elegy-memory-mcp-http.exe
```

If the binary is not on `PATH`, run it by full path, for example:

```powershell
C:\Elegy\bin\elegy-memory-mcp-http.exe
```

## Recommended Windows services

For an always-on setup, run both processes as services.

### `cloudflared`

Install the Cloudflare Tunnel service after `config.yml` is in place.

```powershell
cloudflared service install
```

### `elegy-memory-mcp-http.exe`

Use NSSM for the MCP binary.

```powershell
nssm install ElegyMemoryMcp "C:\Elegy\bin\elegy-memory-mcp-http.exe"
nssm set ElegyMemoryMcp AppDirectory "C:\Elegy\bin"
nssm set ElegyMemoryMcp Start SERVICE_AUTO_START
nssm start ElegyMemoryMcp
```

Set the env vars before installing the service, or configure them in NSSM and restart the service after changes.

## Claude.ai connector setup

Claude.ai uses the public `/mcp` URL and auto-registers through DCR.

1. Open **Settings**.
2. Open **Connectors**.
3. Click **+ Custom**.
4. Enter `https://elegy-memory.holon.it.com/mcp`.
5. Leave client credentials empty.
6. Continue; Claude.ai will use DCR automatically.

## First connect flow

The first connect opens the browser consent flow.

1. Claude opens the browser.
2. The server shows the consent/password page.
3. Enter `ELEGY_MCP_ADMIN_PASSWORD`.
4. Approve the connection.
5. Claude exchanges the code and starts using `/mcp`.

## Claude Desktop

Claude Desktop can use the remote HTTP connector, but local stdio is now the simpler desktop path.

- URL: `https://elegy-memory.holon.it.com/mcp`
- Client secret: none
- Consent flow: same browser round-trip
- Local stdio example: [claude-desktop-config.example.json](claude-desktop-config.example.json)

## Local stdio consumers

Claude Desktop and future Holon hosting can spawn `elegy-memory-mcp-stdio` directly instead of going through Cloudflare Tunnel.

| Consumer | Example |
|---|---|
| Claude Desktop | [claude-desktop-config.example.json](claude-desktop-config.example.json) |
| Holon DesktopHost (planned) | [holon-mcp-config.example.json](holon-mcp-config.example.json) |

## Kill switch

Use service stop for immediate cut-off; use password rotation only to block future approvals.

| Action | What it cuts off | How fast |
|---|---|---|
| Stop `cloudflared` service | Public access to OAuth and `/mcp` | Immediate once the tunnel stops |
| Stop `ElegyMemoryMcp` service | The local server itself, including OAuth and `/mcp` | Immediate once the process stops |
| Disconnect the connector in Claude.ai / Claude Desktop | That client's connector entry | As soon as the client removes it |
| Rotate `ELEGY_MCP_ADMIN_PASSWORD` and restart | New browser consent approvals only | After restart |

Notes:

- Stopping either service is the emergency cut-off.
- Rotating the password does **not** revoke already-issued access or refresh tokens by itself.
- Changing or wiping `ELEGY_MCP_DATA_DIR` also changes persisted signing/client state, but that is a heavier recovery step than a normal kill switch.

## Troubleshooting

These checks cover the common deployment failures for this crate.

| Problem | Check |
|---|---|
| 401 loop on connect | Make sure `ELEGY_MCP_PUBLIC_URL` is the same public URL Claude uses, keep `ELEGY_MCP_DATA_DIR` stable across restarts, and reconnect if old client/token state is stale. |
| DCR failure | Confirm `https://elegy-memory.holon.it.com/.well-known/oauth-authorization-server` and `/oauth/register` are reachable through the tunnel. |
| `redirect_uri` rejected | The allowlist only accepts `https://claude.ai/...`, `https://claude.com/...`, `http://127.0.0.1:<port>/...`, and `http://localhost:<port>/...`. |
| Tunnel down | Restart `cloudflared tunnel run elegy-memory` or the Cloudflare Tunnel service, then retry. |
| Forgotten password | Set a new `ELEGY_MCP_ADMIN_PASSWORD`, restart the MCP service, and reconnect if needed. |
| DNS not propagated yet | Wait for Cloudflare DNS/NS propagation and verify `elegy-memory.holon.it.com` resolves before testing Claude. |
| Accidental rate limit | The OAuth endpoints apply in-memory per-IP limits; wait about 60 seconds, then retry. |
