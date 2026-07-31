# Live account validation

Live checks complement the deterministic fake-provider suite. They are intentionally optional: CI and acceptance never require a personal account.

Live proofs validate runtime packs; they never justify provider-name branches in the broker or UI. Before marking any pack ready, pass provider parsing, endpoint-policy, identity assertion, auth-adapter, proxy audience, lease, and redaction tests against a deterministic loopback fake. Then install the real pack, connect through Account Center, verify close/reopen recovery, approve one narrow read through the broker, restart, revoke, prove the old lease fails, scan artifacts for plaintext, and clean up.

## Safety contract

- Obtain explicit user approval before creating an OAuth application or authorizing an account.
- Request the smallest read-only scope that proves the connector.
- Never print, screenshot, serialize, or return access tokens, private device codes, cookies, or passwords. Provider-issued user codes may appear only in the local consent UI while active.
- Keep remote mutations at zero unless a separately reviewed test explicitly requires one.
- Store live credentials only in the local encrypted vault. Delete temporary proof vaults automatically.
- Evidence may contain provider name, public account identity, timestamps, boolean checks, and mutation counts only.

## GitHub proof lanes

GitHub is the device-authorization proof pack, not a compiled special case.

### Ephemeral broker proof

`npm run proof:github` borrows the existing GitHub CLI session in memory, verifies `/user`, exercises encrypted storage, a read-only grant and lease, restart persistence, revocation, plaintext scans, and cleanup. It never adds that broad CLI credential to the user's permanent Elegy vault. The command fails closed unless `ELEGY_LIVE_PROOF_CONSENT=github-read-only` is set after the operator has approved this supervised run; the wrapper passes the matching `--consent=github-read-only` confirmation to the binary.

The direct invocation is:

```text
cargo run -p elegy-accounts -- proof-github artifacts/live/github-proof.json --consent=github-read-only
```

For the package script, set the acknowledgement in the current process only:

```powershell
$env:ELEGY_LIVE_PROOF_CONSENT = 'github-read-only'
npm run proof:github
Remove-Item Env:ELEGY_LIVE_PROOF_CONSENT
```

The confirmation is an operator acknowledgement, not a substitute for the human GitHub consent or any CAPTCHA, MFA, account-selection, or provider checkpoint described below.

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
| Google live | OAuth PKCE/OIDC | consent, identity, restart, forced refresh, narrow read, local grant revoke, provider revoke, post-revoke failure, plaintext scan, cleanup | 0 | required before any Google usability claim |

For Google, obtain explicit human approval, create a dedicated desktop OAuth
client, set `ELEGY_GOOGLE_CLIENT_ID`, and authorize only the bundled pack's
declared scopes. Record no token, code, cookie, client secret, or refresh
material. The reviewed receipt must cover consent and verified identity,
encrypted restart persistence, forced refresh and scope validation, one narrow
read, local grant revocation, Google's provider revocation endpoint,
post-revocation failure, plaintext scans, and cleanup.

CAPTCHA, MFA, account selection, consent, and recovery remain human
checkpoints. Do not send mail, modify labels, or retain a broader credential.

CAPTCHA, MFA, passkeys, consent, email verification, and provider risk challenges are always human checkpoints. The system can open the correct page and resume afterward; it does not bypass them.
