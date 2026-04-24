---
created: 2026-03-31
updated: 2026-03-31
category: governance
status: proposed
doc_kind: reference
---

# Reusable AI Substrate Roadmap

> Status note, 2026-04-25: the foundation contract schemas listed in Phase 1
> now exist in `contracts/schemas/` and have conformance coverage in the Rust
> workspace. Treat the remaining Phase 1 work as runtime wiring, shared
> machine-output normalization, and real CLI/MCP output validation rather than
> initial schema creation. See
> [AI Agent Integration Roadmap](./ai-agent-integration-roadmap.md)
> for the current integration hardening plan.

## Purpose

Turn the research direction in [Reusable AI substrate patterns for Elegy](../research/reusable-ai-substrate-patterns.md) into a concrete phased roadmap that can guide contract work, Rust crate work, CLI hardening, and downstream adoption without pulling host-specific orchestration into Elegy.

This roadmap is intentionally delivery-oriented. It describes what Elegy should make real, in what order, what evidence each phase should produce, and what should remain outside the repo boundary.

## Source of direction

This roadmap is grounded in the complete research note:

- [Research: reusable AI substrate patterns for Elegy](../research/reusable-ai-substrate-patterns.md)

That note is the rationale and pattern inventory. This roadmap is the execution shape derived from it.

## Boundary rules

This roadmap does **not** change the current architecture boundary already documented elsewhere in the repo.

The following remain true:

- governed artifacts under `contracts/`, `governance/`, and `policies/` remain the durable authority surfaces
- Rust crates under `rust/` remain the reusable executable layer
- thin CLIs remain operator shells, not new authority roots
- host-specific orchestration, auth, tenancy, prompt assembly, approval UX, and control-plane ownership remain consumer-local

Primary boundary references:

- [Ecosystem topology](../architecture/ecosystem-topology.md)
- [Substrate governance](../architecture/substrate-governance.md)
- [MCP, skill, and tooling placement](../architecture/mcp-skill-tooling-placement.md)

## Desired end state

Elegy becomes a protocol-first reusable substrate for AI systems with a clear separation between:

1. governed reusable contracts
2. Rust-first reusable execution traits and implementations
3. thin CLI and host surfaces
4. consumer-local orchestration and product policy

At the end of this roadmap, Elegy should be able to support both standard AI apps and advanced automation systems through reusable building blocks for:

- capability discovery
- capability invocation
- retrieval and memory
- workflow execution and replay
- policy-gated adapters
- machine-first CLI operation
- structured tracing and evaluation

## Non-goals

This roadmap does not propose:

- a monolithic default agent runtime
- transferring Holon or other host orchestration authority into Elegy
- making MCP the only internal abstraction
- early broad desktop automation ownership without policy, replay, and approval contracts
- app-specific planner logic in shared crates

## Delivery principles

### 1. Contract-first

Every reusable family should begin with governed contracts, fixtures, and ownership rules before broad implementation expansion.

### 2. Rust-first execution

Reusable behavior should live in Rust crates, not in markdown or consumer-specific wrappers.

### 3. Eval-by-contract

New substrate families are not complete until they have fixtures, conformance checks, and measurable acceptance signals.

### 4. Progressive disclosure by default

Do not solve reuse by dumping more prompt context into agents. Prefer explicit indexes, summaries, handles, and on-demand expansion.

### 5. High-risk families last

Desktop and OS actuation stay later and gated. Lower-risk reusable contracts come first.

## Workstreams

The roadmap is organized into seven linked workstreams:

1. capability contracts and registries
2. invocation and execution envelopes
3. retrieval and memory substrate
4. workflow and replay substrate
5. policy gates and adapter boundaries
6. machine-first CLI surfaces
7. observability and eval

## Phase 1: foundation contracts and CLI posture

### Goal

Create the minimum reusable contract foundation so all later work uses consistent capability, invocation, and machine-facing CLI semantics.

### Scope

- define a governed capability contract family
- define a governed invocation envelope family
- define machine-first CLI rules and normalize current CLIs around them
- establish trace and correlation identifiers as first-class execution facts

### Deliverables

#### Governed contracts

- `contracts/schemas/capability-definition.schema.json`
- `contracts/schemas/invocation-request.schema.json`
- `contracts/schemas/invocation-response.schema.json`
- `contracts/schemas/execution-event.schema.json`
- `contracts/schemas/structured-failure.schema.json`

#### Governed fixtures

- minimal valid fixtures for each new schema family
- compatibility-manifest coverage for the new families

#### Rust crates or modules

- initial capability types and validation support in Rust
- initial invocation-envelope types and execution-result helpers

#### CLI posture hardening

Normalize `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills` around:

- `--json`
- stable exit codes
- explicit `--non-interactive`
- deterministic stdout and stderr behavior
- correlation ID emission
- stable structured error objects

### Why this phase comes first

The research note’s strongest repeated theme is that reusable AI infrastructure fails when each surface invents its own tool model and execution payload. Phase 1 creates the common contract layer that later retrieval, workflow, and policy work can depend on.

### Exit signal

Phase 1 is complete when:

- the contract families above exist with fixtures and validation
- at least one existing CLI flow maps cleanly to the invocation envelope
- each current operator CLI exposes machine-stable JSON output
- correlation IDs are consistently available in structured output

## Phase 2: capability registry and MCP normalization

### Goal

Create a shared internal capability model and resolver so MCP-backed, generated, and native capabilities can be discovered through one governed shape.

### Scope

- capability registry traits
- resolver traits
- capability metadata for routing and policy
- MCP integration mapped onto the internal capability contract

### Deliverables

#### Governed contracts

- capability metadata guidance for:
  - side-effect class
  - trust level
  - auth mode
  - idempotence
  - latency and cost hints
  - observability labels

#### Rust crates or modules

- `elegy-capability` or equivalent family for:
  - registry loading
  - resolution
  - normalization
  - capability inspection

#### Integration slices

- map current `elegy-mcp` outputs into the capability model
- map current skill-generation outputs into the capability model
- support capability inspection through CLI JSON output

### Why this phase matters

MCP is valuable, but the research note makes clear that protocol interoperability alone is not enough. Elegy needs an internal reusable capability model so routing, policy, and workflow systems do not depend on protocol-specific shapes.

### Exit signal

Phase 2 is complete when:

- native and MCP-backed capabilities can be resolved through one internal model
- capability metadata is inspectable and policy-relevant
- routing can be evaluated against structured fixtures rather than prompt-only descriptions

## Phase 3: retrieval and memory substrate

### Goal

Evolve `elegy-memory` from a bounded local operator surface into a clearer reusable substrate with memory kinds, lifecycle contracts, and retrieval pipeline traits.

### Scope

- define retrieval pipeline stages
- define memory record and memory-write contracts
- distinguish memory kinds and scopes
- support async consolidation and re-embed lifecycle hooks
- preserve current boundary where host-owned ranking and policy can stay outside Elegy
- add explicit room for episodic execution memory, verifier outcomes, and experiment lineage without turning Elegy into a host control plane

### Deliverables

#### Governed contracts

- `contracts/schemas/retrieval-pipeline.schema.json`
- `contracts/schemas/retrieval-result-package.schema.json`
- `contracts/schemas/memory-record.schema.json`
- `contracts/schemas/memory-write-intent.schema.json`
- `contracts/schemas/memory-gate-decision.schema.json`

#### Rust crates or modules

- `elegy-retrieval` or equivalent family for:
  - query rewrite
  - candidate retrieval
  - rerank
  - compression
  - citation packaging
- expanded `elegy-memory` core traits for:
  - memory kinds
  - scopes
  - provenance
  - freshness
  - contradiction hooks
  - async re-embed or consolidation
  - episodic execution or outcome memory kinds
  - verifier result capture
  - promotion-history or experiment-lineage hooks

#### CLI work

- keep `elegy-memory` as a thin operator shell
- expose richer typed JSON results for:
  - search
  - inspect
  - contradictions
  - reembed

### Why this phase matters

The research note identifies two strong trends:

- real RAG systems are retrieval pipelines, not single vector calls
- memory systems are multi-tier and lifecycle-aware, not one generic table

Recent autonomous-improvement research sharpens the same point. Systems like SOAR and AgentFlow improve because they can learn from weak, partial, or verifier-labeled experience over time. That argues for explicit support for execution outcomes and verifier memory as first-class reusable substrate concepts, while still keeping host-specific ranking and orchestration authority outside Elegy.

This phase converts Elegy’s promising memory primitives into a stronger reusable substrate.

### Exit signal

Phase 3 is complete when:

- retrieval stages exist as explicit traits or contracts
- memory kinds and scopes are typed and inspectable
- provenance, freshness, and contradiction semantics are part of the reusable model
- at least one retrieval result package includes structured citations and provenance

## Phase 4: workflow, checkpoints, and replay

### Goal

Turn workflow graphs into a durable reusable execution substrate with explicit checkpoints, lifecycle events, approvals, and replay semantics.

### Scope

- extend the workflow graph toward execution semantics
- define checkpoint and lifecycle event contracts
- define approval and stop-condition contracts
- implement executor traits with replay expectations
- support explicit planner, executor, verifier, and generator role visibility where consumers need that split

### Deliverables

#### Governed contracts

- strengthen `canonical-workflow-graph`
- add:
  - workflow checkpoint schema
  - workflow lifecycle event schema
  - policy stop-condition schema
  - approval decision schema
  - verifier outcome or turn-decision schema

#### Rust crates or modules

- `elegy-workflow` or equivalent family for:
  - executor traits
  - checkpoint persistence interfaces
  - replay helpers
  - compensation hooks
  - typed role or step-kind helpers for planner, executor, verifier, and final synthesis patterns

### Why this phase matters

The research note is explicit that one of the biggest agent-system anti-patterns is letting the LLM be the workflow engine. This phase makes deterministic orchestration and replay a first-class substrate capability without taking business-flow ownership away from hosts.

Recent evidence from AgentFlow strengthens this sequencing: the improvement comes from optimizing planning inside a structured multi-turn system with evolving memory and verifier feedback, not from making orchestration less explicit. Elegy should therefore prioritize typed lifecycle, step-role, and verifier-outcome contracts before any ambition toward broader autonomous loops.

### Exit signal

Phase 4 is complete when:

- a workflow can emit structured lifecycle events
- checkpoints can be recorded and replayed
- retries and approvals are part of the contract model
- at least one conformance example proves replay behavior

## Phase 5: policy gates and bounded adapters

### Goal

Make execution-time policy decisions a governed substrate surface for reusable capability families, starting with lower-risk adapters and explicit side-effect classes.

### Scope

- define side-effect classes
- define policy decision contracts
- define approval and simulation semantics
- align adapter families to deny-by-default behavior

### Deliverables

#### Governed contracts

- `contracts/schemas/policy-decision.schema.json`
- `contracts/schemas/side-effect-class.schema.json`
- `contracts/schemas/simulation-result.schema.json`
- `contracts/schemas/escalation-reason.schema.json`

#### Rust crates or modules

- policy enforcement helpers that integrate with:
  - filesystem adapters
  - HTTP adapters
  - future workflow execution
  - future capability routing

### Why this phase matters

The research note shows that prompt-only safety is too weak for reusable automation. Side-effectful systems need execution-time allow or deny behavior with structured reasons and logs.

### Exit signal

Phase 5 is complete when:

- high-risk execution requests pass through typed policy decisions
- dry-run or simulation results can be returned in structured form
- policy denials and escalations are machine-readable and traceable

## Phase 6: observability and eval-by-contract

### Goal

Make tracing and validation a required part of every substrate family, not a later add-on.

### Scope

- OTel-compatible tracing helpers
- structured execution events
- fixture-based conformance suites
- evaluation harnesses for retrieval, routing, workflow, and policy behavior
- fixed-budget benchmark loops and explicit keep, reject, or escalate outcome recording where applicable

### Deliverables

#### Rust crates or modules

- `elegy-observability` or equivalent helpers for:
  - spans
  - events
  - correlation IDs
  - execution IDs
  - attribute conventions

#### Eval harnesses

- retrieval relevance and citation checks
- capability routing correctness checks
- workflow replay checks
- policy gate enforcement checks
- verifier outcome consistency checks
- bounded experiment-result comparisons for repeated runs of the same capability or workflow slice

### Why this phase matters

The research note treats observability and evaluation as first-class infrastructure. This phase is what prevents contract families from becoming elegant but unprovable abstractions.

Recent autonomous-improvement work reinforces that the loop is only trustworthy when every candidate change is measured against a stable budget and recorded with explicit acceptance or rejection reasons. Elegy does not need to own the optimizer, but it should make that evidence model reusable.

### Exit signal

Phase 6 is complete when:

- all major execution surfaces can emit OTel-compatible span or event data
- fixtures and evals exist for the major substrate families
- regressions in routing, retrieval, replay, or policy can be caught locally

## Phase 7: progressive disclosure surfaces

### Goal

Bake progressive disclosure into capability discovery, memory inspection, workflow inspection, and CLI ergonomics so Elegy supports scalable agent usage without context bloat.

### Scope

- add index, detail, and deep-reference layers
- support explicit expansion operations
- expose size, token, or cost hints where meaningful

### Deliverables

#### Reusable disclosure model

- index contracts for capabilities, memory records, and workflow artifacts
- detail retrieval flows
- deep-reference handles or linked artifact refs

#### CLI or host behavior

- capability listing returns compact metadata first
- memory and workflow surfaces can expand on demand
- CLI help and inspect flows align with the disclosure model

### Why this phase matters

The research note identifies progressive disclosure as one of the highest-leverage reliability and efficiency patterns in modern agent systems. This phase converts that pattern into reusable Elegy behavior instead of leaving it as informal practice.

### Exit signal

Phase 7 is complete when:

- at least capabilities, memory, and workflow inspection follow a staged disclosure model
- consumers can request deeper detail without loading everything by default

## Phase 8: later high-risk adapter family for desktop automation

### Goal

Only after the earlier substrate is stable, add a separately governed desktop and OS automation family for cross-consumer reusable actioning.

### Scope

- intent and action-plan contracts
- desktop adapter traits
- evidence and replay semantics
- high-risk policy and approval integration

### Deliverables

- desktop intent contracts
- desktop actuation contracts
- evidence capture model
- adapter traits with strict policy boundaries

### Why this is later

The research note is explicit that desktop automation is strategically important but high risk. It should be layered on top of already-proven invocation, workflow, policy, and observability semantics rather than introduced early.

### Exit signal

Phase 8 is complete when:

- desktop operations can be expressed through typed intent and actuation contracts
- high-risk actions can be simulated, approved, traced, and replayed
- at least two real consumers justify keeping the family in Elegy

## Cross-phase acceptance checks

The roadmap as a whole is only credible if the following become true over time:

- each new contract family has schemas, fixtures, and compatibility coverage
- each reusable execution family has Rust traits and conformance checks
- CLI surfaces remain thin and machine-stable
- policy, workflow, retrieval, and memory families share correlation and execution IDs
- no phase pulls host-specific orchestration or approval UX into Elegy

## Risks

### Contract sprawl

Too many schemas too early could create governance cost without proven reuse.

### Abstraction mismatch

If capability, workflow, retrieval, and policy models are forced together too early, the resulting contracts may be too generic to be useful.

### Upward scope creep

Pressure to absorb product orchestration into Elegy would violate current boundary guidance.

### High-risk family drag

Desktop automation can distort priorities if introduced before the safer substrate layers are stable.

### Eval debt

If conformance and evals lag behind new families, the repo will accumulate attractive but weakly proven abstractions.

## Recommended immediate next moves

The best next slice after this roadmap is:

1. draft the capability-definition contract family
2. draft the invocation-envelope contract family
3. define the shared CLI machine-mode rules
4. split `elegy-memory` thinking into memory-core versus operator shell
5. reserve contract space for verifier outcomes and bounded experiment-result records so later workflow and eval work does not invent them ad hoc

That sequence creates the smallest meaningful substrate foundation while staying aligned with current repo boundaries.

## References

- [Research: reusable AI substrate patterns for Elegy](../research/reusable-ai-substrate-patterns.md)
- [Ecosystem topology](../architecture/ecosystem-topology.md)
- [Substrate governance](../architecture/substrate-governance.md)
- [MCP, skill, and tooling placement](../architecture/mcp-skill-tooling-placement.md)
- [Elegy-memory V1](../architecture/elegy-memory-v1.md)
- AgentFlow: <https://agentflow.stanford.edu/>
- SOAR: <https://arxiv.org/abs/2407.20635>
- Autoresearch: <https://github.com/karpathy/autoresearch>
