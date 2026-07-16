---
name: manage-accounts
description: Connect, discover, create, select, review, and revoke online accounts through the local Elegy Accounts broker, and request least-privilege access for Codex or Holon agents without exposing credentials. Use when an agent needs an account for research, client work, deployment, email, cloud infrastructure, source control, or another provider-backed action; when a user asks what accounts are connected; or when auth, CAPTCHA, MFA, consent, terms, payment, identity verification, or account selection blocks a workflow.
---

# Manage accounts

Use the `elegy-accounts` MCP tools. Treat account metadata, grants, leases, and request status as safe agent data. Treat credentials, OAuth codes, browser sessions, and provider tokens as broker-only data.

## Resolve access

1. State the concrete purpose and smallest named operations required by the downstream tool, such as `dns.records.read` or `deployments.create`.
2. Call `account_require` with provider, purpose, operations, and an account ID only when the user already selected one.
3. If an account is available, call `account_request_access`. Never widen operations to avoid a later approval.
4. If interaction is required, call `account_discover` and explain the provider-supported choices. Use `account_open_center` when the person needs to act.
5. If no suitable account exists and creation is appropriate, call `account_request_creation` with purpose and user constraints. Poll only when useful with `account_request_status`; do not busy-wait.
6. Continue the original task only after the request reports approved/connected and the downstream provider tool accepts the capability.

## Human checkpoints

Stop automation and hand control to the person for CAPTCHA, MFA, terms, payment, KYC/identity verification, ambiguous plans, unexpected pages, or provider consent. Explain exactly what is waiting and how the agent will resume. Never click through, accept, solve, or bypass these boundaries without the user's direct action where policy permits.

## Browser discovery

Describe Brave discovery as "continue with your signed-in browser." It is an unverified hint until the broker validates provider identity. Never claim that Elegy imports saved passwords or cookies. Never ask the user to paste a password, session cookie, OAuth code, refresh token, or API token into chat. When a provider requires a limited token, direct capture to Account Center or the Elegy Brave extension.

## Multiple accounts and revocation

When more than one identity matches, show sanitized provider identities and ask the user to select; do not guess. Use `account_revoke_grant` when requested or when access is no longer needed. Use `account_audit_list` to explain a decision or failure without requesting secret diagnostics.

Read [references/provider-modes.md](references/provider-modes.md) only when choosing between provider-specific connection methods.
