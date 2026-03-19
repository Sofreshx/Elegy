# Rust Consolidation

## Purpose

This document records the first consolidation decision after the earlier split-topology plan.

The main Elegy repo is now the intended long-term home for both:

- the authoritative .NET formalization and contract families
- the first-party Rust runtime family for MCP-oriented execution behavior

## Direction

The repository should converge on this shape:

- `src/` and `tests/` remain the home of governed contracts, canonical skill models, schema authority, and package-family validation
- `rust/` becomes the in-repo Cargo workspace for runtime composition, host behavior, adapters, CLI, and Rust-native contract consumption

This is a consolidation move, not a transfer of contract authority out of the .NET packages.

## What stays authoritative in .NET

The following remain canonical in the .NET package families:

- governed JSON schemas and fixtures under `src/Elegy.Formalization.Contracts/Resources`
- compatibility manifest and compatibility matrix artifacts
- canonical skill definitions and validation semantics in `Elegy.Formalization.Skills`
- the package-boundary policy enforced for `.NET` source packages

## What should move to Rust first

The first replacement target is the remaining behavior-heavy MCP logic in `Elegy.Formalization.Mcp`:

- `McpToolAnalyzer`
- `McpSkillGenerator`
- `McpToolSearchService`
- `McpToolResolveService`

These are stronger Rust candidates because they are operational behavior over governed contracts rather than canonical contract ownership.

## Replacement rule

Use Rust where one or more of these dominate the problem:

- transport or protocol behavior
- host integration
- filesystem or HTTP execution
- runtime composition and orchestration
- behavior-heavy MCP logic over governed artifacts

Keep `.NET` where the main responsibility is:

- schema authority
- compatibility governance
- canonical contract definition
- formalization semantics that other runtimes must consume

## Initial implementation slice

The initial consolidation slice should do four things only:

1. rewrite docs so the repo tells the truth about the new topology
2. create the in-repo Rust workspace landing zone under `rust/`
3. import the first useful Rust contract-consumer crate without changing contract authority
4. keep parity tests and governed artifacts intact before replacing additional `.NET` behavior

## Status overview

### Completed work

- the main Elegy docs now describe the mixed-language monorepo topology instead of the old sibling-repo default
- `rust/` exists as the in-repo Cargo workspace landing zone
- `elegy-contracts` has been imported and consumes governed Elegy artifacts in-repo
- the next imported runtime support crate, `elegy-policy`, is part of the in-repo workspace
- parity-first Rust MCP behavior now exists for analyzer, generator, and discovery logic in `elegy-mcp`
- `elegy-descriptor` is now imported as the first remaining root crate from the old runtime stack, giving the monorepo a native descriptor-loading and normalization layer
- `elegy-adapter-fs` and `elegy-adapter-http` are now imported as the next runtime layer on top of descriptor plus policy, preserving bounded filesystem and HTTP adapter behavior in-repo
- `elegy-runtime`, `elegy-core`, `elegy-host-mcp`, and `elegy-cli` are now imported as the operator-facing runtime stack on top of the shared runtime layers
- runtime tests, examples, and parity-oriented assets now live in-repo under `rust/tests` and `rust/examples`
- the duplicated `.NET` MCP analyzer, generator, and discovery types are now retained as internal parity anchors instead of public runtime-facing behavior surfaces
- the repository root now owns contributor, security, conduct, architecture, and MCP spec-baseline posture for the mixed-language monorepo
- the main Elegy CI workflow now runs Rust formatting, linting, tests, multi-OS Rust lanes, and monorepo security automation alongside the existing `.NET` governance checks

### Remaining work

- harden and validate the fully imported Rust runtime stack as the day-to-day operator path inside Elegy
- complete the eventual removal path for the now-internal-only duplicated `.NET` MCP analyzer, generator, and discovery surfaces after remaining consumers finish cutover
- expand monorepo version-governance rules so the Rust workspace version story is documented as clearly as the `.NET` package and schema story
- finish archival or redirect cleanup for historical sibling repos so no stale implementation center remains implied

## Session closeout view

### What is done

- the topology decision is no longer ambiguous: Elegy is the single main repo and the Rust runtime family now lives in-repo under `rust/`
- the contract-authority side is in place: canonical skill contracts, governed MCP artifacts, package-boundary policy, and governed export flows remain anchored in `.NET`
- the imported Rust runtime stack now lives in Elegy end-to-end: contracts, policy, MCP behavior, descriptor loading, filesystem and HTTP adapters, runtime/core composition, MCP host, and CLI
- runtime tests, examples, and parity-oriented assets now live with that stack in the monorepo instead of being left behind elsewhere
- the mixed-language repo now has a real validation baseline instead of a documentation-only posture: `.NET` governance checks plus Rust formatting, linting, tests, multi-OS CI, and security automation run in the main repo
- the repository root is now the public home for contributor, governance, security, and spec-baseline posture

### What is left at a high level

- harden and validate the fully imported Rust runtime stack as the steady-state operator path
- decide the staged retirement path for duplicated `.NET` MCP behavior after remaining consumers are reviewed
- document Rust version and release governance as explicitly as the `.NET` package and schema story
- finish archival cleanup for historical sibling repos and any residual stale references

## Early-goal check

This section compares the earlier goals against the work completed so far and the work still required.

### Goal: make Elegy the single main repo

Status: achieved for active implementation and governance posture, with archival cleanup still remaining.

Completed:

- the docs and workspace now treat Elegy as the main monorepo
- the Rust runtime family now lives in-repo instead of being treated as a sibling-repo primary surface
- runtime tests, examples, and parity assets now live in the monorepo with the imported runtime stack
- contributor, governance, security, and MCP spec-baseline posture now point to the main Elegy repo

Still required:

- finish archival or forwarding cleanup in historical sibling repos so they stop reading like active implementation centers
- clean up any remaining stale references that still imply the old topology

### Goal: move the right Elegy behavior from C# to Rust

Status: achieved for the intended MCP/runtime boundary, with deprecation cleanup still remaining.

Completed:

- Rust now owns parity-first MCP analyzer, generator, and discovery behavior
- Rust now owns descriptor loading plus policy-bounded filesystem and HTTP adapter behavior
- the imported runtime, core, host, and CLI crates now consume the Rust MCP/runtime path in-repo

Still required:

- keep the imported runtime, core, host, and CLI crates thin over the shared runtime stack
- decide the deprecation path for the duplicated `.NET` MCP behavior once parity anchors are no longer needed

Boundary clarification:

- the goal is not to remove all C# from Elegy
- `.NET` remains the authority for governed contracts, compatibility artifacts, canonical skills, and formalization semantics
- Rust should continue taking over runtime-heavy MCP, host, transport, filesystem, HTTP, and CLI behavior where that is the better fit

### Goal: keep skills, MCP, and CLI responsibilities coherent inside Elegy

Status: the model is now embodied in the imported runtime stack, with additional hardening still required.

Skills:

- completed: canonical skill authority is already established in `Elegy.Formalization.Skills`
- remaining: ensure all new Rust runtime paths continue to consume canonical skill outputs rather than introducing a Rust-local skill contract

MCP:

- completed: governed MCP artifacts exist in `.NET`, parity-first MCP behavior exists in Rust, and the imported runtime stack now consumes that Rust MCP path in-repo
- completed: shared runtime tests, examples, and parity-oriented assets now live in the monorepo
- remaining: narrow duplicated `.NET` MCP behavior only after the parity anchors are no longer needed

CLI:

- completed: the repo-level design says the human-facing CLI belongs above the public runtime/core layers, not inside generation or contract packages
- completed: the Rust CLI and host crates are now imported into Elegy and remain thin over the shared core/runtime layers

### Goal: reduce responsibility in Holon or SAASTools by pushing reusable logic into Elegy

Status: barely started outside the contract-authority side.

Completed:

- the contract-authority and runtime-consolidation work in Elegy is creating the package surfaces needed for downstream reuse

Still required:

- identify concrete Holon and SAASTools responsibilities that should call into Elegy packages instead of owning their own copies of skills, MCP shaping, generation flows, or runtime logic
- replace app-local logic incrementally with package consumption, starting with the most obviously reusable MCP or skill-related surfaces
- prove that downstream repos can consume Elegy packages or the in-repo Rust runtime outputs without reintroducing responsibility drift back into those repos

The practical implication is that downstream replacement work should begin only after the relevant Elegy surface is stable enough to be consumed. The next likely candidates are MCP-related shaping or runtime responsibilities, not arbitrary application code.

### Current next sequence

1. validate and harden the imported operator-facing runtime stack in-repo
2. decide the staged deprecation path for the duplicated `.NET` MCP behavior once the remaining consumer review is complete
3. continue downstream responsibility extraction only where the relevant Elegy surface is stable enough to consume

## Current implementation wave

The current wave is intentionally narrow:

1. keep the imported runtime-stack crates in dependency order rather than collapsing them into the MCP parity crate
2. keep the descriptor and runtime-layer validation logic intact during import
3. avoid deprecating any `.NET` MCP behavior until the parity anchors are no longer required by real consumers
4. keep host and CLI validation narrow and operator-focused now that the core runtime layers are present in Elegy

The first crates in that sequence were `elegy-descriptor`, then the filesystem and HTTP adapters, because they form the dependency root and first real runtime layer for the remaining stack.

## High-level roadmap after this session

### 1. Finish runtime consolidation inside Elegy

- keep the imported `elegy-runtime`, `elegy-core`, `elegy-host-mcp`, and `elegy-cli` crates validated together as one in-repo operator stack
- add contributor-facing smoke coverage and docs around the imported operator flows where useful

### 2. Finish the `.NET` MCP retirement path inside Elegy

- preserve the existing parity anchors while they still provide useful confidence
- narrow duplicated `.NET` MCP behavior only after the remaining consumers no longer depend on them
- keep the retirement path staged and reversible until the operator path is fully settled

### 3. Start downstream responsibility extraction from Holon or SAASTools

- identify MCP, skill, and generation responsibilities that still live in app repos but should be owned by Elegy packages or by the in-repo Rust runtime family
- replace those surfaces incrementally with Elegy consumption so the app repos stop carrying reusable infrastructure logic
- use actual integration points, not aspirational boundaries, to decide what leaves those repos next

### 4. Harden the monorepo operating model

- document explicit Rust version-governance rules alongside the existing package/schema governance story
- keep runtime docs, MCP spec-baseline docs, contribution posture, and security posture rooted in Elegy
- archive or redirect historical runtime repos as closeout follow-through

## Validation posture

During the migration, the `.NET` MCP tests remain parity anchors until a Rust replacement is proven.

The mixed-language validation story should eventually include both:

- `.NET` governance and architecture checks
- Rust workspace formatting, linting, and tests
