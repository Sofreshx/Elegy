# Examples

Examples in the Elegy Rust workspace are intended to be more than tutorial material.

They are kept in-repo as:

- contributor onboarding assets
- acceptance inputs for runtime/core validation
- small, reviewable demonstrations of the supported runtime model
- preserved goldens for the currently exercised core, CLI, and host transport proofs

## Current imported status

The current monorepo runtime slice includes both examples that are exercised by the imported runtime/core tests and a deferred-scope example that remains intentionally scaffolded.

### Exercised today

- `examples/fs-static-minimal/`
  - active acceptance input for filesystem/static validation and runtime composition
  - covered by `crates/elegy-core/tests/bootstrap_slice.rs`
  - shared acceptance input for current core validation and host transport coverage
  - includes `expected-resources.json` as the deterministic catalog reference for the currently implemented path
- `examples/http-minimal/`
  - active acceptance input for the implemented plain constrained HTTP runtime path
  - covered by `crates/elegy-core/tests/bootstrap_slice.rs`
  - used by the current umbrella `elegy` dry-run acceptance and host transport coverage
  - preserves the expected JSON outputs used by the former standalone runtime repo so they remain available during monorepo consolidation

### Scaffolded today

- `examples/http-openapi-minimal/`
  - scope marker for OpenAPI descriptor and policy shape
  - useful for contributor review and future runtime composition work
  - retains placeholder expected outputs from the former standalone runtime repo
  - is currently exercised as a rejection-path example so unsupported OpenAPI runtime execution stays explicit

This distinction matters: not every example in the repository is meant to prove an implemented end-to-end runtime path yet.

## Example rules

When examples are added, they should be:

- small
- hermetic where possible
- easy to understand without prior project knowledge
- safe to use in local development and CI
- aligned with the actual supported feature set

## How the current examples are used

### Filesystem/static minimal

This is the primary imported acceptance example for the current bootstrap runtime slice. It exercises:

- project config loading
- descriptor normalization
- deterministic resource inspection inputs
- bounded filesystem behavior
- runtime catalog composition for filesystem and inline static resources

### HTTP minimal

This is the imported acceptance example for the implemented plain constrained HTTP slice. It exercises:

- HTTP descriptor loading and normalization
- policy-bounded outbound HTTP configuration
- runtime catalog composition for the current HTTP family
- deterministic expected outputs preserved for runtime-facing validation

The example should still be read conservatively: it proves the narrow bootstrap HTTP path, not a broad HTTP integration surface.

### HTTP/OpenAPI minimal

This example currently exists to keep the OpenAPI/bootstrap story honest and reviewable:

- the repository already has OpenAPI descriptor forms
- policy configuration already models the related outbound HTTP constraints
- the example gives contributors a concrete placeholder input to work from

What it does **not** currently prove is OpenAPI operation execution or successful runtime composition for that family.

## Reference links

- [Rust workspace README](../README.md)
- [Rust consolidation note](../../docs/architecture/rust-consolidation.md)
