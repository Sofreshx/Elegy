# Live account validation

Live checks complement the deterministic fake-provider suite. They are intentionally optional: CI and acceptance never require a personal account.

## Safety contract

- Obtain explicit user approval before creating an OAuth application or authorizing an account.
- Request the smallest read-only scope that proves the connector.
- Never print, screenshot, serialize, or return access tokens, private device codes, cookies, or passwords. Provider-issued user codes may appear only in the local consent UI while active.
- Keep remote mutations at zero unless a separately reviewed test explicitly requires one.
- Store live credentials only in the local encrypted vault. Delete temporary proof vaults automatically.
- Evidence may contain provider name, public account identity, timestamps, boolean checks, and mutation counts only.

## GitHub proof lanes

### Ephemeral broker proof

`npm run proof:github` borrows the existing GitHub CLI session in memory, verifies `/user`, exercises encrypted storage, a read-only grant and lease, restart persistence, revocation, plaintext scans, and cleanup. It never adds that broad CLI credential to the user's permanent Elegy vault.

### Production Device Flow proof

1. Register a dedicated local GitHub OAuth app with Device Flow enabled.
2. Set its public client ID in `ELEGY_GITHUB_CLIENT_ID`; no client secret is stored or used.
3. Start Account Center and choose GitHub.
4. The UI shows only the user code and GitHub verification URL. The private device code is persisted only as an authenticated-encryption envelope so an unexpired session survives broker restart.
5. The user approves the requested `read:user` permission on GitHub.
6. Confirm the verified GitHub identity appears in Account Center and through the bounded MCP account-list tool.
7. Restart Account Center and confirm the encrypted connection persists.
8. Issue and approve a `profile.read` request, execute one read-only `/user` call through the broker boundary, revoke it, and prove the lease fails.
9. Scan the database, backup, logs, evidence, and UI output for credential plaintext.

The July 16, 2026 live run verified `Sofreshx`, UI close/reopen recovery, broker-owned polling, successful GitHub identity validation, encrypted account persistence across broker restart, zero active authorization sessions after completion, and zero remote mutations.

## Evidence matrix

| Provider/lane | Auth path | Minimum proof | Remote writes | MVP state |
|---|---|---|---:|---|
| Deterministic fake providers | OAuth PKCE and GitHub Device Flow | exact request shape, pending/slow/deny/success, identity validation, secret redaction | 0 | required in CI |
| GitHub live | OAuth Device Flow, `read:user` | connect, identity, persistence, lease, read, revoke, plaintext scan | 0 | first live release gate |
| Cloudflare live | user-created scoped token | verify active token; list account/zones only; no DNS edits | 0 | next proof target |

CAPTCHA, MFA, passkeys, consent, email verification, and provider risk challenges are always human checkpoints. The system can open the correct page and resume afterward; it does not bypass them.
