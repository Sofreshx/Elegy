# Portable Agent Authorization

Status: approved for implementation on 2026-07-20.

## Decision

Elegy Accounts is a local authorization broker for agent tools. A person
connects an account once in Account Center, then registered Codex and Holon
tools may perform approved, typed provider operations without receiving the
credential.

Browser-session sharing, replacement of host-owned connectors, remote vault
sync, and autonomous acceptance of provider checkpoints are outside this
boundary.

## Runtime boundary

One current-user broker process owns credentials, account state, grants,
policy, refresh, revocation, audit, and authenticated provider execution.
Account Center remains the enrollment and approval surface. Brave remains an
optional provider-discovery shortcut.

Credential-free action hosts call a versioned local execution protocol. Client
identity is established by the installed host adapter and cannot be selected by
an LLM. Requests contain a provider, typed operation, schema-validated
arguments, optional account selector, and stable purpose class. Responses are
either a sanitized typed result, a durable interaction checkpoint, or a stable
failure code with an audit correlation identifier.

Opaque leases remain broker-internal on the new execution path.

## Provider operations and policy

Provider-pack v2 adds executable operation definitions with stable identifiers,
risk classes, required scopes, fixed audiences, argument/result schemas, and a
constrained HTTP recipe or reviewed code-adapter identity. Agents cannot choose
arbitrary URLs, methods, headers, or scopes.

Read grants default to thirty days and remain immediately revocable. Scope,
account, client, or purpose-class changes require a new approval. Writes require
confirmation bound to the normalized action digest.

The first end-to-end operations are GitHub profile and repository reads plus
Cloudflare zone and DNS-record reads. Google and provider mutations are
deferred.

## Host surfaces

The existing MCP server remains the account-control surface. The plugin also
ships a separate credential-free MCP action host that exposes trusted typed
operations and calls the broker execution boundary. Holon consumes the same
versioned protocol through its own adapter.

Existing encrypted accounts migrate unchanged. Legacy `codex-local` grants and
leases are invalidated because they are not bound to an authenticated client.
Provider-pack v1 remains valid for enrollment; execution requires trusted v2
operations.

## Acceptance

Release requires real Codex-visible read-only GitHub and Cloudflare operations,
the equivalent Holon protocol conformance path, restart and grant reuse,
multiple-account selection, expiry, reauthentication, revocation, client
impersonation rejection, trusted-pack enforcement, schema and audience
confinement, response redaction, and secret-canary scans across every public
surface.

Do not claim portable authorization if no real consumer completes a broker-
backed operation, if credentials leave the broker, if an untrusted pack can
select the credential destination, or if a write can occur without exact-action
approval.
