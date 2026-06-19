# Elegy Substrate Governance

## Purpose

This document is the canonical governance baseline (historical) for the current Elegy repo.

It defines:

- the governed artifact boundary
- the Rust executable boundary
- allowed dependency direction between those layers
- the rules for promoting concepts into durable repo-owned surfaces
- the shared-contract governance model for schemas, fixtures, manifests, and policy artifacts

This document is intentionally narrower than the broader ecosystem topology doc. The topology doc explains the high-level repo relationship. This document explains the concrete governance needed for the repo that exists today.

## Repository layers

Elegy now has three practical layers.

### Layer 1: governed artifacts

These are the durable authority roots and must stay language-agnostic.

| Surface | Responsibility |
| --- | --- |
| `contracts/schemas/` | Governed JSON schema authority |
| `contracts/fixtures/` | Minimal and parity fixtures |

### Layer 2: Rust executable crates

These crates consume governed artifacts and provide reusable executable behavior.

| Surface | Responsibility |
| --- | --- |
| `rust/core/elegy-contracts` | Rust consumption of governed contracts |
| `rust/core/elegy-policy` | Policy enforcement helpers |
| `rust/features/elegy-memory` | Dedicated bounded-memory executable behavior and CLI surface |
| `rust/features/elegy-mcp` | Dedicated MCP descriptor authoring/analysis behavior and CLI surface |
| `rust/features/elegy-skills` | Dedicated MCP-to-skill generation behavior and CLI surface |
| `rust/core/elegy-tooling` | Shared helper and compatibility infrastructure for descriptor and skill workflows |
| `rust/core/elegy-descriptor` | Descriptor loading and normalization |
| `rust/core/elegy-adapter-fs` and `rust/core/elegy-adapter-http` | Bounded adapter behavior |
| `rust/core/elegy-runtime` and `rust/core/elegy-core` | Reusable runtime composition |
| `rust/bin/elegy-host-mcp` and `rust/bin/elegy-cli` | Thin host and umbrella general/compatibility surfaces |

### Layer 3: export and validation surfaces

These surfaces validate and ship the governed and executable layers without redefining them.

| Surface | Responsibility |
| --- | --- |
| `elegy-cli contracts export` | Bundle export |
| `elegy-cli contracts validate` | Canonical output validation |
| Conformance tests in `rust/core/elegy-contracts/tests/conformance.rs` | Per-feature publish-metadata contract |
| `.github/workflows/*.yml` | CI enforcement for artifacts, Rust, security, and distribution |

## Dependency direction rules

The following rules are mandatory until a later architecture decision changes them explicitly:

1. Governed artifacts are the authority boundary and must not depend on Rust implementation details.
2. Rust crates may consume governed artifacts, but they must not silently redefine schema, fixture, manifest, or policy authority.
3. Runtime-composition crates may depend on lower Rust crates and governed artifacts, but lower layers must never depend upward on CLI or host shells.
4. Operator surfaces such as `elegy-cli` and `elegy-host-mcp` must remain thin over explicit runtime and tooling crates.
5. Export scripts and workflows validate or package the repo surfaces; they are not alternate places to invent contract truth.
6. Downstream consumers should integrate through exported bundles, documented policy artifacts, explicit Rust crates, or CLI outputs rather than through removed solution-level or source-package assumptions.
7. External agents outside Elegy should load the associated skill guidance and invoke the dedicated `elegy-*` CLI directly when one exists.

## Post-legacy rule

Elegy no longer has an active first-party `.NET` source-package family in-repo.

That means:

1. docs must not describe `src/` or `tests/` as active repo centers
2. any downstream `.NET` consumer is now just that: a consumer of governed outputs, not a co-owned in-repo authority surface
3. new shared responsibilities should be expressed either as governed artifacts or as Rust executable behavior, not by reintroducing legacy compatibility framing

## Public-surface graduation rule

A concept should become a durable Elegy-owned surface only when all of the following are true:

1. the responsibility is stable and not just a temporary helper
2. the boundary is clearer as a governed artifact or reusable Rust executable feature than as consumer-local behavior
3. the concept has at least one real validation path
4. the concept improves ownership more than it increases maintenance cost

If those conditions are not met, keep the capability as docs, policy, or consumer-local logic until the abstraction proves itself.

## Core contract change policy

Changes are considered core contract changes when they alter any of the following:

- schema shape or required fields in publishable contract artifacts
- fixture meaning or compatibility evidence
- compatibility manifests, support metadata, or version-policy semantics
- dependency-direction policy between governed artifacts and Rust executable layers

Core contract changes must update the relevant docs, fixtures, and validation paths in the same change.

## Shared-contract governance

Shared contracts, fixtures, manifests, and policy artifacts are governed centrally in this repo.

That means:

1. the authoritative source lives in `Elegy`, not in downstream consuming repos
2. versioning rules are defined in `contracts/schemas/schema-version.json`
3. first-party Rust crates and downstream consumers should consume published artifacts or versioned files, not co-own the truth through copy-paste drift
4. coordinated change procedures are required before a governed contract family becomes a wider dependency

## Fixture and conformance corpus rule

Fixtures are governed compatibility evidence.

Every publishable schema family should eventually have:

- at least one minimal valid fixture
- compatibility-manifest coverage
- machine-readable compatibility description where downstream consumers depend on the contract

When a schema, fixture, or manifest is changed, the governed corpus must be reviewed in the same change.

## Enforcement surfaces

Current enforcement lives in these surfaces:

- `elegy-cli contracts export`
- `elegy-cli contracts validate`
- Conformance tests in `rust/core/elegy-contracts/tests/conformance.rs`
- `.github/workflows/distribution-artifacts.yml`
- `.github/workflows/rust-ci.yml`
- Rust workspace tests that exercise CLI and tooling behavior

## Completion standard

The governance baseline is only complete when the repo is not just described but enforced.

The minimum bar is:

1. governed artifact roots are documented
2. export and canonical-output validation are runnable from the repo root
3. Rust executable surfaces are linted and tested from the Rust workspace
4. contributor docs point to the real validation and export path rather than to removed solution-era flows
