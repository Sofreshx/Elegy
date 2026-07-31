# Live account validation

Live checks complement the deterministic fake-provider suite. They are intentionally optional: CI and acceptance never require a personal account.

Live proofs validate runtime packs; they never justify provider-name branches in the broker or UI. Before marking any pack ready, pass provider parsing, endpoint-policy, identity assertion, auth-adapter, proxy audience, lease, and redaction tests against a deterministic loopback fake. The GitHub lane below is deliberately narrower: it proves installed-package Device Flow, one approved packaged MCP read, local disconnect, the identical MCP call failing afterward, artifact scanning, and cleanup. Restart persistence and provider-side revocation require separate evidence before either may be claimed.

## Safety contract

- Obtain explicit user approval before creating an OAuth application or authorizing an account.
- Request the smallest read-only scope that proves the connector.
- Never print, screenshot, serialize, or return access tokens, private device codes, cookies, or passwords. Provider-issued user codes may appear only in the local consent UI while active.
- Keep remote mutations at zero unless a separately reviewed test explicitly requires one.
- Store live credentials only in the local encrypted vault. Delete temporary proof vaults automatically.
- Evidence may contain provider name, public account identity, timestamps, boolean checks, and mutation counts only.

## GitHub proof lanes

GitHub is the device-authorization proof pack, not a compiled special case.

### Packaged Device Flow proof

Use a dedicated GitHub OAuth application configured for Device Flow. Set only
its public client ID in `ELEGY_GITHUB_CLIENT_ID`; do not use a GitHub CLI
session, a personal OAuth client, or a client secret. The proof wrapper builds
the release binary, creates the plugin archive, installs that archive into a
fresh session-local install root, and invokes only the installed proof binary.
It never invokes `gh` or a source `cargo run -p elegy-accounts` proof.

For the package script, acknowledge the supervised read-only run in the
current PowerShell process only:

```powershell
$env:ELEGY_LIVE_PROOF_CONSENT = 'github-device-read-only'
$env:ELEGY_GITHUB_CLIENT_ID = '<dedicated-device-flow-client-id>'
npm run proof:github
Remove-Item Env:ELEGY_LIVE_PROOF_CONSENT
Remove-Item Env:ELEGY_GITHUB_CLIENT_ID
```

The wrapper creates a fresh temporary session root, writes
`localappdata/.elegy-accounts-live-proof.json` with schema version
`elegy-accounts-live-proof-root/v1`, and sets both `LOCALAPPDATA` and
`ELEGY_ACCOUNTS_PROOF_ROOT` to that marked isolated root. It preserves the
session root and prints the resulting `github-proof.json` path for review.

The confirmation is an operator acknowledgement, not a substitute for the
human GitHub consent or any CAPTCHA, MFA, account-selection, or provider
checkpoint described below.

1. Register a dedicated local GitHub Device Flow OAuth application. No client
   secret is stored or used.
2. Run the wrapper as above. It builds, packages, and installs the archive
   before starting the installed `elegy-accounts.exe` proof binary.
3. In the installed Account Center, choose GitHub.
4. The UI shows only the user code and GitHub verification URL. The private device code is persisted only as an authenticated-encryption envelope so an unexpired session survives broker restart.
5. The user approves the requested `read:user` permission on GitHub.
6. Confirm the verified identity appears in Account Center. The installed
   packaged Actions MCP server (`elegy-account-actions`) must advertise and execute exactly
   `github_profile_read`; approve its narrow local request if Account Center
   requires it.
7. Confirm its read-only result identifies the connected GitHub account.
8. Disconnect the account locally in Account Center. Call the same packaged
   `github_profile_read` MCP tool again and confirm it returns
   `account_unavailable`.
9. Scan the isolated database, backup, logs, evidence, and UI output for
   credential plaintext.

This proof establishes only the local disconnect and subsequent packaged MCP
failure. Provider-side GitHub token revocation is not yet supported or proven.

## Evidence matrix

| Provider/lane | Auth path | Minimum proof | Remote writes | MVP state |
|---|---|---|---:|---|
| Deterministic fake providers | OAuth PKCE and GitHub Device Flow | exact request shape, pending/slow/deny/success, identity validation, secret redaction | 0 | required in CI |
| GitHub live | OAuth Device Flow, `read:user` | installed package, connect, identity, approved packaged MCP read, local disconnect, identical MCP failure, plaintext scan, cleanup | 0 | narrow live proof; not persistence or provider revocation evidence |
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
