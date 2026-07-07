---
title: Shared Crate Boundaries
status: active
owner: elegy-core
doc_kind: reference
---

# Shared Crate Boundaries

Shared crates stay separate only when they own a real boundary: cross-surface
reuse, policy isolation, OS-specific code, schema/package tooling, or a stable
runtime contract.

## Current Decisions

| Crate | Evidence | Decision |
| --- | --- | --- |
| `shared/core` | Used by host, tool, and plugin crates; owns governed contract export and shared envelopes. | Keep. |
| `shared/plugin-sdk` | Zero internal workspace dependencies; used by packaging/tooling and downstream plugin repos. | Keep. |
| `shared/policy` | Used by descriptor/runtime adapters; owns fail-closed policy primitives. | Keep. |
| `shared/tooling` | Owns plugin packaging, marketplace generation, install/export tooling. | Keep. |
| `shared/desktop-win32` | Single consumer: `plugins/desktop`; isolates Windows desktop automation code. | Keep as OS boundary. |
| `shared/observe-win32` | Single consumer: `plugins/observe`; isolates Windows observation code. | Keep as OS boundary. |
| `shared/descriptor` | Used by `shared/core`, `shared/runtime`, and both runtime adapters. | Keep. |
| `shared/runtime` | Used by `shared/core`; composes descriptor, policy, adapters, and MCP analysis. | Keep as runtime contract. |
| `shared/adapter-fs` | Single direct consumer: `shared/runtime`; owns filesystem policy enforcement and symlink/size tests. | Review before merge. |
| `shared/adapter-http` | Single direct consumer: `shared/runtime`; owns HTTP policy enforcement, redirect denial, and bounded reads. | Review before merge. |

## Merge Rule

Merge a shared crate only when all are true:

- it has one real consumer,
- it does not isolate OS-specific code,
- it does not own policy or runtime fail-closed behavior,
- its tests can move without weakening coverage,
- the resulting dependency direction remains simpler than the current split.

`adapter-fs` and `adapter-http` are the only current merge candidates. They are
not mechanical deletions because they contain behavior-specific tests and
security-sensitive IO boundaries.
