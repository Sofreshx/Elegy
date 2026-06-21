---
created: 2026-03-30
updated: 2026-05-28
category: docs
status: active
doc_kind: reference
---

# Backlog

## Purpose

Track the small set of active, durable planning items that should drive the next execution slices without reopening a full roadmap rewrite.

## Scope Boundaries

- This slice is complete locally; no active backlog item remains for contributor-facing proof hardening or local distribution validation.
- Hosted release publication is now live on `main` and tags; ongoing remote verification stays tracked through [Unresolved Goals](issues/unresolved-goals.md) instead of reopening this backlog.

## Session Closeout

No active backlog item remains from this slice. RB-001 and RB-002 both completed locally in this session, and only ongoing hosted release verification carries forward through [Unresolved Goals](issues/unresolved-goals.md).

### RB-001

Title: Claim-versus-proof acceptance hardening for current contributor-facing runtime and distribution surfaces.

Outcome: completed locally. Contributor-facing runtime, CLI, host, and distribution statements now stay narrowed to behavior the repo proves through local runnable evidence.

Execution Slice: docs, acceptance framing, and proof anchors were aligned around the current Rust-owned runtime and distribution posture, including direct user-facing coverage for the umbrella CLI and the dedicated MCP and skills binaries.

Evidence Anchors:

- [docs/architecture/README.md](architecture/README.md)
- [docs/architecture/rust-consolidation.md](architecture/rust-consolidation.md)
- [docs/distribution.md](distribution.md)
- [rust/crates/elegy-cli/tests/authoring.rs](../rust/crates/elegy-cli/tests/authoring.rs)
- [rust/crates/elegy-cli/tests/mermaid.rs](../rust/crates/elegy-cli/tests/mermaid.rs)
- [rust/crates/elegy-mcp/tests/cli.rs](../rust/crates/elegy-mcp/tests/cli.rs)
- [rust/crates/elegy-skills/tests/cli.rs](../rust/crates/elegy-skills/tests/cli.rs)
- [.github/workflows/distribution-artifacts.yml](../.github/workflows/distribution-artifacts.yml)

Covered By: [RM-distribution-proof-hardening-001](roadmaps/distribution-proof-hardening.md)

### RB-002

Title: Local distribution validation for packaging, metadata, installer, and validation flows.

Outcome: completed locally. Packaging and installer guidance now rest on repeatable local artifact and validation evidence, and the hosted publish lane is now exercised separately through the live release workflow.

Execution Slice: local archive production, release metadata composition, installer consumption, and canonical distribution checks completed from repo-supported scripts and the local distribution-artifacts workflow.

Evidence Anchors:

- [docs/distribution.md](distribution.md)
- [.github/workflows/distribution-artifacts.yml](../.github/workflows/distribution-artifacts.yml)
- [scripts/package-installer.ps1](../scripts/package-installer.ps1)
- [scripts/install-distribution.ps1](../scripts/install-distribution.ps1)
- [scripts/validate-canonical-outputs.ps1](../scripts/validate-canonical-outputs.ps1)

Covered By: [RM-distribution-proof-hardening-002](roadmaps/distribution-proof-hardening.md)

## References

- [docs/architecture/README.md](architecture/README.md)
- [docs/architecture/rust-consolidation.md](architecture/rust-consolidation.md)
- [docs/distribution.md](distribution.md)
- [docs/issues/unresolved-goals.md](issues/unresolved-goals.md)
