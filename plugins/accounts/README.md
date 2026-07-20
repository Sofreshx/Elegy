# Elegy Accounts

Elegy Accounts is a strictly local authorization broker for people and AI agents. Connect an account once, then registered local tools can perform approved typed operations across sessions without receiving the credential. The broker owns provider authorization, encrypted storage, risk-based grants, authenticated execution, durable human checkpoints, audit, and revocation.

The current release is provider-generic. Runtime JSON packs describe OAuth PKCE, device authorization, scoped API tokens, HTTP Basic/app passwords, and client credentials. GitHub, Cloudflare, and Google are bundled proof packs—not compiled limits. See [provider packs](docs/provider-packs.md) to add another provider.

## Surfaces

- **Account Center:** loopback-only UI for connection, credential entry, consent, pending attention, account inventory, and revocation. It can run standalone or embedded in Holon/Elegy Studio.
- **Agent plugin:** MCP tools discover accounts, request least privilege, list attention, present the exact local checkpoint, resume/cancel durable requests, and audit decisions.
- **Typed action host:** a separate credential-free MCP surface exposes trusted GitHub and Cloudflare reads and sends signed requests to the persistent broker over current-user local IPC.
- **Authenticated broker:** validates a lease against client, purpose, provider, operation, scope, expiry, and destination; decrypts and injects auth internally; returns only sanitized provider output.
- **Brave bridge:** recognizes origins from the runtime pack registry and opens Account Center. It has no cookie, password-manager, broad host, or saved-credential access.

The bundled Codex action host currently exposes GitHub profile/repository reads and Cloudflare zone/DNS-record reads. Holon can embed Account Center, but its typed execution adapter is a separate host integration and is not shipped by this plugin yet. Existing host-owned connectors keep their own authentication; Elegy Accounts is the portable authorization path for local and custom tools.

## Security boundary

Credentials are authenticated-encrypted at rest with per-record keys protected by Windows DPAPI. OAuth state, PKCE, issuer/audience/redirect binding, provider identity verification, explicit user intent headers, operation maps, audience allowlists, expiring grants, single-use leases, redacted audit events, restart-safe authorization sessions, and deterministic retries are enforced locally.

The user must act for consent, CAPTCHA, MFA, terms, payment, KYC, ambiguous plans, and credential entry. Agents never receive passwords, tokens, OAuth codes, browser cookies, or refresh material. Browser discovery is only a hint until the broker verifies identity.

## Development

From `plugins/accounts`:

```powershell
npm install
npm run check
npm run acceptance
```

The full acceptance lane runs Rust tests and policy checks, UI tests and Playwright, npm/Rust audits, secret scanning, plugin validation, release packaging, Codex export, and an installed broker smoke test.

## Documentation

- [Architecture](docs/architecture.md)
- [Provider-pack authoring](docs/provider-packs.md)
- [Security model](docs/security.md)
- [Acceptance criteria](docs/acceptance.md)
- [Windows installation](docs/install-windows.md)
- [Live proof procedure](docs/live-validation.md)
