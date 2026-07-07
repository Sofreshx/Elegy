# Elegy Memory MCP

## Start Here

- Read `README.md` and `docs/architecture/overview.md` before changing the transport model.
- Read `docs/AUTH.md` before changing HTTP, OAuth, DCR, JWT, bearer-token, or public route behavior.
- Read `docs/TRANSPORT.md` before changing stdio or Streamable HTTP behavior.
- Read `docs/CONFIG.md` and `docs/DEPLOYMENT.md` before changing environment variables or deployment guidance.
- Read `../../plugins/memory/AGENTS.md` before changing storage, salience, correction, provenance, or scope behavior.

## Boundaries

- This crate adapts `elegy-memory` to MCP transports. It does not define new memory authority or bypass memory guardrails.
- Tool calls must stay bound to `MemoryScope::Agent`; do not accept request-level `scope`, `scopes`, `namespace`, or similar override fields.
- The stdio binary is a local subprocess transport. It must not gain OAuth, DCR, JWT, HTTP discovery, or public network behavior.
- The HTTP binary owns the remote OAuth/Bearer lane. Keep `/mcp` protected and keep OAuth metadata, DCR, consent, and token routes deliberate and documented.
- `stdout` is reserved for the stdio MCP protocol. Send diagnostics and logs to `stderr`.
- Audit logs may include tool, id, scope, timestamp, and token id. Do not log memory content by default.
- Successful write tools must reuse the underlying salience gate and correction behavior rather than writing around `elegy-memory`.

## Review Focus

- Before public HTTP/OAuth emphasis expands, address the Review B note in `docs/PENDING.md`.
- Check scope isolation, auth failures, token handling, and degraded embedding behavior alongside normal success paths.
- For stdio changes, verify startup failures stay clear and protocol output stays clean.
- For HTTP changes, verify bearer enforcement and public-route intentionality, not only server startup.

## Validation

- Prefer crate-local validation first: `cargo test -p elegy-memory-mcp`.
- When shared memory behavior changes, also run the smallest relevant `elegy-memory` validation.
- When docs or config examples change, inspect the affected example JSON or command line directly instead of only proofreading prose.
