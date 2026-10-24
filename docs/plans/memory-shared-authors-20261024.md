---
title: Shared Memory with distinct authors
status: active
owner: elegy-memory
doc_kind: planning
---

# Shared Memory with distinct authors

Goal: prepare opt-in shared agent reads on the existing database without
rewriting authors or accessing real data. Authority: root AGENTS.md â†’
architecture â†’ [Memory MCP spec](../specs/memory-mcp-v1/spec.md) â†’ host guidance.
The user explicitly authorized preparing the isolated correction; cloud
activation and credentials remain outside this implementation slice.

Facts: each stdio host has one fixed agent identity; namespace is not a row
partition; search has metadata effects; correction and consolidation can have
indirect targets. No existing shared-owner correction was found across local
branches/worktrees. Source starts at cdaaaccc462c986f4a2c0032b75698f15c71f62b.

Implementation: add binding-owned allowlist and shared-space configuration;
keep writes owner-only; filter search before effects; prevent correction from
merging across authors; filter consolidation candidates before evaluation;
expose original agent ID and record actor in content-history operations.

Validation: observe new fictional-data regressions fail before implementation,
then pass; run affected Memory and MCP suites, formatting and contracts/docs
checks. Review authorization edges, inspect exact diff, scan private exclusions
before committing owned files on the named branch. Preserve the checkout and
evidence. No push, installation, real database opening, migration, credentials,
tunnel changes or readiness promotion in this slice. Stop and report any such
boundary rather than broadening it.

Progress and results are recorded with the implementation commit and test logs;
tests do not establish cloud connectivity or caller identity delegation.

## Implementation evidence

The [execution receipt](../../hosts/memory-mcp/evidence/shared-agents-2026-10-24.json)
binds source hashes to the local commands and log digests. Full Memory: 408
tests passed. Full MCP: 88 passed, including 17 new shared-access regressions
and four real stdio protocol tests. The four stdio tests were rerun after
style-only helper lint fixes. Formatting, focused Clippy and contracts passed.

Runtime RED/GREEN evidence covers 1001 hidden vector neighbors starving an
authorized result, and page over-fetch touching a non-returned row. The
missing binding API was first observed as a compile failure. Review also
required preserving the caller's own tenant/user-bound rows and filtering
correction/consolidation candidates before indirect mutation. The final
targeted static review approved those changes.

The deterministic documentation bundle was regenerated and the documentation
check passed. One DOCS-CHECK-009 freshness warning concerns the unchanged
observation roadmap; it remains outside this slice.

Activation remains a separate operation: build/install the committed source,
back up the existing database using the previously qualified SQLite method,
then configure each authorized host with its own actor, the common database
and the same explicit owner allowlist. Reuse existing authors, do not
reattribute rows. One tunnel/plugin bound to one process remains one actor.
No runtime key or actual cloud connection was created in this slice.
