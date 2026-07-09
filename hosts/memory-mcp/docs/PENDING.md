# Pending follow-ups

This file tracks crate-local documentation notes that are intentionally deferred.

## Review B — scope isolation

The stdio namespace was fixed to use the agent-scoped namespace instead of
the HTTP/OAuth `"claude-ai-remote"` default.

| Item | Status | Evidence |
|---|---|---|
| `memory_tools.rs` scope isolation review | Resolved for stdio transport (`fix/stdio-namespace-isolation`). The HTTP/OAuth lane retains `"claude-ai-remote"` as its fixed scope — this is correct per design and enforced by OAuth scope validation on `/mcp`. | `src/stdio_main.rs:125`: `MemoryBinding::new(&config.agent_id, &config.agent_id)`. Test at `src/stdio_main.rs:396-445` asserts `namespace == "default-agent"` when `ELEGY_MCP_AGENT_ID` is unset. |

### Remaining

- HTTP/OAuth lane: scope is pinned to `DEFAULT_NAMESPACE` = `"claude-ai-remote"`. This is intentional and enforced at the bearer-token level (`OAUTH_SCOPE`). No change needed.
- Future: if a new transport or binding mode is added, repeat the same isolation pattern (dedicated namespace per transport, not shared defaults).