---
created: 2026-03-30
updated: 2026-05-28
category: docs
status: active
doc_kind: reference
---

# Distribution Proof Hardening

## Purpose

Record the two local distribution-proof phases completed in this session: claim-versus-proof hardening first and local validation second.

## Phase Boundaries

- This roadmap only covers proof that can be produced inside the current repo.
- Hosted release publication now runs on `main`, tags, and release events; ongoing remote verification stays tracked through [Unresolved Goals](../issues/unresolved-goals.md) rather than as an active phase here.

## Completed Phases

### RM-distribution-proof-hardening-001

Covers: [RB-001](../backlog.md)

Phase: 1

Goal: completed locally. Claim-versus-proof acceptance is now hardened for the current contributor-facing runtime, CLI, host, and distribution posture.

Operational Focus:

- inventoried current contributor-facing claims across architecture and distribution docs
- reduced or qualified claims that exceeded current runnable proof
- tied accepted claims to concrete local evidence from the Rust workspace and distribution workflow, including direct user-facing umbrella CLI and dedicated MCP and skills binary coverage

Primary Evidence:

- [docs/architecture/README.md](../architecture/README.md)
- [docs/architecture/rust-consolidation.md](../architecture/rust-consolidation.md)
- [docs/distribution.md](../distribution.md)
- [rust/crates/elegy-cli/tests/authoring.rs](../../rust/crates/elegy-cli/tests/authoring.rs)
- [rust/crates/elegy-cli/tests/mermaid.rs](../../rust/crates/elegy-cli/tests/mermaid.rs)
- [rust/crates/elegy-mcp/tests/cli.rs](../../rust/crates/elegy-mcp/tests/cli.rs)
- [rust/crates/elegy-skills/tests/cli.rs](../../rust/crates/elegy-skills/tests/cli.rs)
- [.github/workflows/distribution-artifacts.yml](../../.github/workflows/distribution-artifacts.yml)

Exit Signal: achieved locally. Contributor-facing runtime and distribution claims are narrowed to what the repo currently builds, validates, and explains locally.

### RM-distribution-proof-hardening-002

Covers: [RB-002](../backlog.md)

Phase: 2

Goal: completed locally. The local distribution lane is now validated for packaging, metadata, installer, and validation flows.

Operational Focus:

- exercised local contract, CLI, wrapper, and installer packaging paths
- verified release manifest and checksum generation for local artifacts
- confirmed installer consumption and canonical distribution validation matched the local artifact flow described in repo docs

Primary Evidence:

- [docs/distribution.md](../distribution.md)
- [.github/workflows/distribution-artifacts.yml](../../.github/workflows/distribution-artifacts.yml)
- [scripts/package-installer.ps1](../../scripts/package-installer.ps1)
- [scripts/install-distribution.ps1](../../scripts/install-distribution.ps1)
- [scripts/validate-canonical-outputs.ps1](../../scripts/validate-canonical-outputs.ps1)

Exit Signal: achieved locally. Local distribution validation is documented and evidenced independently of the hosted publish lane, which is now active and verified through the release workflow itself.

## References

- [docs/backlog.md](../backlog.md)
- [docs/architecture/README.md](../architecture/README.md)
- [docs/architecture/rust-consolidation.md](../architecture/rust-consolidation.md)
- [docs/distribution.md](../distribution.md)
- [docs/issues/unresolved-goals.md](../issues/unresolved-goals.md)
