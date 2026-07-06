# Pending follow-ups

This file tracks crate-local documentation notes that are intentionally deferred.

## Review B

Keep this review open before any return to public HTTP/OAuth emphasis.

| Item | Why it stays open | When to do it |
|---|---|---|
| `memory_tools.rs` scope isolation review | The current stdio lane is allowed to ship without this deeper review, but public HTTP/OAuth usage should not rely on that assumption indefinitely. | Before any future push back toward public HTTP/OAuth deployment or re-expansion of the remote connector story. |
