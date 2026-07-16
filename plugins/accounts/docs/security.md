# Security and threat model

## Security invariants

1. Agent-visible APIs never contain plaintext secrets, browser cookie values, authorization headers, OAuth codes, or refresh tokens.
2. Secret material is encrypted at rest and decrypted only inside the broker for validation, refresh, revocation, or host execution.
3. Leases are opaque, short-lived, single-client, purpose-bound, audience-bound, operation-bound, and invalid after grant/account/client revocation.
4. Human presence is mandatory for CAPTCHA, MFA, terms acceptance, payment, KYC, identity verification, and ambiguous irreversible choices.
5. Browser permissions are optional and provider-specific; no global browsing or cookie permission is required.
6. The UI binds only IPv4 loopback, MCP uses child-process stdio, Brave uses an exact-origin Native Messaging registration, and agent grant requests reject unknown client IDs.
7. Logs, errors, audit payloads, crash reports, exports, clipboard operations, and UI state are secret-redacted by construction.
8. Account discovery is a hint, not proof of identity. A provider adapter validates the resulting credential and records the verified provider identity.

## Assets and adversaries

Protected assets include credentials, account identity, grant policy, action history, and local recovery material. Consider malicious web pages, compromised extensions, prompt-injected agents, accidental logging, another local OS user, stale clients, replayed leases, confused-deputy requests, provider redirects, token substitution, and partial signup failures.

The MVP does not claim protection from malware executing as the same fully compromised Windows user or from a compromised provider. It minimizes blast radius through operation scopes, local client binding, short leases, auditability, and revocation.

## Threat controls

| Threat | Required control | Verification |
|---|---|---|
| Agent asks for raw token | Typed contracts omit secret fields; executor is host-only | schema snapshot and redaction tests |
| Database copied | AES-GCM envelope + current-user DPAPI key protection | ciphertext inspection and restore test |
| Lease replay by another client | client/audience binding and expiry | cross-client and expired-lease tests |
| Revoked access remains usable | revocation generation checked on every redemption | revoke-then-execute test |
| OAuth interception/substitution | PKCE, exact redirect matching, state/nonce, issuer/audience/resource validation | callback negative tests |
| Malicious page fakes discovery | discovery marked unverified; broker validates connected credential | discovery/identity mismatch test |
| Browser extension compromise | optional hosts, no cookie/password APIs, no extension secret persistence | manifest and storage tests |
| Secret leaks through diagnostics | structured redactor at all output boundaries | seeded-canary scan of logs/errors/audit/export |
| Signup duplicates account | persisted idempotency key and resume-safe saga | crash/retry test |
| Agent crosses human boundary | checkpoint taxonomy enforced by state machine | CAPTCHA/MFA/terms/payment/KYC tests |
| Local network exposure | fixed IPv4 loopback bind; stdio and Native Messaging for non-UI clients | packaging health/bind test |
| CSRF/UI drive-by approval | non-simple intent header plus explicit confirmation UI; OAuth state/PKCE | forged mutation and callback tests |

## Credential lifecycle

- Capture: trusted local UI, OAuth redirect handler, device flow, or Native Messaging handoff.
- Validate: provider identity endpoint; requested operation compatibility; issuer/audience/resource checks where applicable.
- Store: encrypted envelope plus non-secret account metadata and credential fingerprint.
- Use: internal host executor under a valid lease.
- Refresh: broker-owned rotation with old-secret replacement performed transactionally.
- Revoke: provider revocation where supported, then immediate local invalidation regardless of remote outcome.
- Delete: remove ciphertext and account associations; retain only non-sensitive tombstone/audit facts needed for integrity.

## Practical browser boundary

“Use my Brave account” means the extension can recognize a supported provider page and offer an explicit Allow action that starts a safe provider flow. It does not mean scraping saved passwords or copying session cookies into the vault. Where a provider lacks OAuth/device authorization, the extension may guide the user to create a narrowly scoped API token and transfer the newly created value directly to the broker once; the agent and extension storage never receive it.
