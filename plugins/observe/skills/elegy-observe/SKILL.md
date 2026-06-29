---
name: elegy-observe
description: Observe desktop and OS state for agentic workflows. Snapshot running processes, track foreground and visible windows, capture screen, read clipboard, diff filesystem changes, and query system information.
version: "2.0"
---

# Desktop & OS Observation

Observe desktop and OS state for agentic workflows.

## Capabilities

- `observe-processes`: Take a snapshot of running processes with PID, name, memory usage, and CPU percentage.
- `observe-window`: Get information about the current foreground (active) window. Windows only.
- `observe-windows`: List all visible top-level windows with title, PID, and bounds. Windows only.
- `observe-screen`: Capture the current screen as PNG. Windows only.
- `observe-clipboard`: Read the current system clipboard contents.
- `observe-filesystem`: Observe a directory for changes over a bounded time window using snapshot diff.
- `observe-system`: Snapshot system hardware and OS information.
- `observe-record`: Record bounded foreground-window activity for a short session.
