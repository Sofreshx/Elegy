# MVP acceptance contract

The MVP is complete only when every `MUST` item passes and evidence is saved by `npm run acceptance`. Tests use fake provider servers and synthetic secrets; no real account is required in CI.

## Product scenarios

| ID | Requirement | Level | Evidence |
|---|---|---:|---|
| AC-01 | Account Center launches locally and shows connected, needs-attention, and revoked accounts without exposing secret values. | MUST | E2E + screenshot |
| AC-02 | On an arbitrary pack-declared provider origin, the Brave extension offers **Allow**, starts the provider flow without host/cookie/password permission, and the verified account appears in Account Center. | MUST | extension integration test + E2E |
| AC-03 | The same connected account is visible through Codex tools and the Holon/Elegy embedded surface without reconnecting. | MUST | contract + embed E2E |
| AC-04 | A Codex-visible typed action requests one read approval, reuses the approved 30-day grant, executes through authenticated local IPC, and returns sanitized results without a credential or lease. | MUST | action MCP + named-pipe broker integration test |
| AC-05 | A write operation not included in a read-only grant is denied and audited. | MUST | policy integration test |
| AC-06 | Multiple accounts for one provider are distinguishable by verified identity and can be selected explicitly. | MUST | broker + UI test |
| AC-07 | Revoking a grant immediately invalidates derived leases and updates all surfaces. | MUST | broker + E2E |
| AC-08 | Restarting the broker preserves encrypted accounts, grants, audit history, resumable signup state, and unexpired authorization sessions; provider polling does not depend on an open UI. | MUST | restart integration test + authorization-session vault test |
| AC-09 | Encrypted backup and same-user restore recover metadata and usable credentials without plaintext in the archive. | MUST | backup integration test + canary scan |
| AC-10 | Arbitrary provider packs exercise OAuth PKCE, device authorization, scoped tokens, Basic/app passwords, and client credentials against deterministic fakes and return verified identities. | MUST | pack + adapter conformance suites |

## Autonomous creation scenarios

| ID | Requirement | Level | Evidence |
|---|---|---:|---|
| AC-11 | An agent can request account creation with purpose and constraints; the saga is idempotent and reports structured progress. | MUST | saga integration test |
| AC-12 | CAPTCHA, MFA, terms, payment, KYC/identity, ambiguous plan, and unexpected page states pause with a clear human checkpoint; automation cannot acknowledge or bypass them. | MUST | state-machine parameterized tests |
| AC-13 | After a simulated crash, retry resumes the same signup rather than creating a duplicate. | MUST | crash/retry integration test |
| AC-14 | Cancellation closes the active attempt where possible, stores no partial credential, and records an audit event. | MUST | saga cancellation test |

## Security gates

| ID | Requirement | Level | Evidence |
|---|---|---:|---|
| AC-15 | A seeded secret canary is absent from agent responses, UI state, logs, errors, audit payloads, browser storage, backups, and database plaintext scans. | MUST | automated canary suite |
| AC-16 | Vault records contain authenticated ciphertext and a DPAPI-protected key; tampering fails closed. | MUST | vault unit/integration tests |
| AC-17 | Expired, wrong-client, wrong-purpose, wrong-audience, widened-operation, revoked, and replayed single-use leases fail closed. | MUST | policy parameterized tests |
| AC-18 | OAuth callback validation rejects state, nonce, issuer, audience/resource, redirect, and PKCE mismatches. | MUST | OAuth negative tests |
| AC-19 | Brave manifest has no password API, no unconditional broad host access, and no secret-bearing extension storage. | MUST | manifest/static + runtime tests |
| AC-20 | UI transport binds only IPv4 loopback; account/action MCP uses stdio; typed execution uses signed current-user named-pipe requests with replay rejection; Brave uses exact-origin Native Messaging; unknown clients and forged UI mutations are rejected. | MUST | transport boundary + packaging tests |
| AC-21 | Dependency, license, secret, and vulnerability checks have no unresolved critical finding. | MUST | audit report |

## Usability and quality gates

| ID | Requirement | Level | Evidence |
|---|---|---:|---|
| AC-22 | A non-technical user can complete the simulated Allow flow with no copied credential and no more than one broker approval after provider login/consent; closing/reopening the UI preserves the request and expiry exposes a clear retry. | MUST | E2E action count + resumed/expired UI tests |
| AC-23 | Standalone and embedded Account Center share components and behavior; embed mode does not duplicate auth logic. | MUST | source architecture test + E2E |
| AC-24 | Keyboard navigation, focus visibility, labels, contrast, reduced motion, desktop, and mobile layouts pass the local accessibility/visual checks. | MUST | automated a11y + visual QA |
| AC-25 | Plugin manifest validates and agent-facing schemas contain no generic raw execute or secret-read tool. | MUST | plugin validator + schema test |
| AC-26 | Windows install, start, stop, extension setup, local backup, restore, and uninstall are documented and smoke-tested. | MUST | packaging smoke test |

## Done definition

`npm run acceptance` must generate `artifacts/acceptance/report.json` mapping AC-01 through AC-26 to passing evidence. Manual visual evidence may supplement but never replace functional or security tests. Any skipped MUST item fails the release.
