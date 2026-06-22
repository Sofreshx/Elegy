# Research: reusable AI substrate patterns for Elegy

This note is research-oriented guidance for contributors. It is not the canonical source for shipped behavior.

Canonical implemented truth stays in:

- `contracts/`
- `rust/`
- [docs/architecture/ecosystem-topology.md](../architecture/ecosystem-topology.md)
- [docs/architecture/substrate-governance.md](../architecture/substrate-governance.md)
- [docs/architecture/mcp-skill-tooling-placement.md](../architecture/mcp-skill-tooling-placement.md)
- [rust/features/elegy-memory/docs/architecture/v1.md](../../rust/features/elegy-memory/docs/architecture/v1.md)

## Why this note exists

Elegy already has a strong contract-first and reusable-runtime posture. The open question is not whether Elegy should become an app runtime. The better question is which reusable contracts, traits, CLIs, and policy-bounded execution surfaces should live here so both standard AI apps and Holon-style automation systems can reuse them without dragging host-specific orchestration into this repo.

The current repo guidance already points in the right direction:

- `docs/architecture/ecosystem-topology.md` keeps host-specific orchestration, auth, persistence, and prompt assembly in consuming repos.
- `docs/architecture/substrate-governance.md` keeps governed artifacts authoritative and Rust executable behavior below thin operator surfaces.
- `docs/architecture/skill-core-v1.md` keeps skill truth in governed artifacts and treats markdown projections as non-authoritative.

This note extends that posture with current state-of-the-art patterns for:

- capability contracts and registries
- invocation envelopes
- workflow and DAG execution
- retrieval and RAG pipelines
- memory systems
- progressive disclosure and context loading
- MCP and skill/tool routing
- machine-first CLI design for AI
- policy gates, approvals, and high-risk adapters
- observability and evaluation

## Executive summary

The strongest direction for Elegy is not a monolithic default agent runtime.

The stronger direction is a protocol-first reusable substrate with explicit contracts for what can be invoked, how execution is described, how retrieval and memory work, how workflows checkpoint and replay, how policy gates are applied, and how traces and evals are captured.

That means Elegy should prefer:

1. governed contracts for stable reusable semantics
2. Rust crates for reusable executable behavior
3. thin CLIs and hosts over those crates
4. consumer-local ownership for app orchestration, tenancy, approval UX, and prompt strategy

Recent external research strengthens this direction further. The strongest current autonomous-improvement systems are not winning by making one giant opaque agent loop. They are winning by combining:

- explicit role separation such as planner, executor, verifier, and generator
- evolving memory that records intermediate state and failures
- bounded mutation surfaces instead of unconstrained repo-wide self-editing
- fixed evaluation budgets and keep-or-discard decisions tied to one metric or verifier outcome
- asynchronous improvement loops where expensive learning or upkeep happens outside the critical write path

## A current-state model of the industry direction

Across agent systems, production-grade patterns are increasingly converging on the same shape:

- **declarative capability descriptions** rather than prompt-only tool hints
- **normalized invocation envelopes** rather than ad hoc tool-call payloads
- **workflow engines with checkpoints and replay** rather than letting the model improvise orchestration
- **retrieval pipelines** rather than a single vector-search call
- **multi-tier memory** rather than one undifferentiated memory table
- **progressive disclosure** rather than loading giant prompts and huge tool registries up front
- **policy gates and approvals** rather than guardrails hidden only in system prompts
- **structured traces and eval harnesses** rather than debugging by prose and screenshots

That direction matches Elegy's current burden-of-proof rule better than a large integrated framework would.

Recent examples reinforce the same shape from a different angle:

- **AgentFlow** shows that tool-using agents improve when planning, execution, verification, and final synthesis are separated and coordinated through evolving memory instead of one monolithic policy.
- **SOAR** shows that autonomous improvement gets stronger when semantics are decoupled from execution and when systems learn from weak or imperfect autonomous data without blocking the main loop on heavy supervision.
- **Autoresearch** shows the practical value of sharply bounded editable surfaces, fixed evaluation budgets, and explicit keep-or-reject experiment records.

These examples do not mean Elegy should own autonomous research or host orchestration. They do mean Elegy should prioritize the reusable substrate pieces that such systems repeatedly depend on: execution envelopes, verifier-friendly event models, memory lifecycle contracts, and eval-by-contract.

## Recommended Elegy direction

The best-fit direction for this repo is a reusable substrate with the following families:

1. **Capability contracts and registries**
2. **Invocation and execution envelopes**
3. **Retrieval and memory interfaces**
4. **Declarative workflow or DAG IR plus resumable executor traits**
5. **Policy-gated adapters and approval boundaries**
6. **Machine-first CLI and host surfaces**
7. **Observability and eval-by-contract**

The rest of this document explains each family in detail.

---

## 1. Capability contracts and registries

### What this is

A capability contract is a canonical, machine-readable definition of something invokable.

In Elegy terms, that could unify:

- tools
- skills
- MCP-backed capabilities
- workflow nodes
- retrieval sources
- desktop or OS actions
- policy-gated adapters

This should not erase differences between those families. It should provide a reusable top-level shape for discovery, routing, and governance.

### Why it is good

Without a normalized capability contract, systems usually fall back to one of two weak patterns:

- prompt-only descriptions that are easy for humans to read but hard to validate
- per-runtime tool models that drift from each other and from documentation

A governed capability contract improves:

- **routing quality** because capability metadata becomes explicit
- **policy decisions** because effects and trust can be declared
- **cross-runtime reuse** because multiple consumers can discover the same capability
- **auditability** because the contract becomes part of the execution chain
- **CLI and host interoperability** because everything can be inspected through one model

### Where Elegy would use it

Elegy should use a capability contract as the shared reusable layer above:

- `elegy-mcp` descriptor outputs
- skill generation outputs
- future workflow-node references
- high-risk adapter families such as desktop automation

Holon or other consumers could then resolve and invoke capabilities through a stable Elegy-owned contract without surrendering orchestration ownership.

### Suggested contract fields

A minimal first version should likely include:

- identifier
- display name
- version
- capability family
- tags
- input schema
- output schema
- side-effect class
- trust level
- auth mode
- idempotence hint
- cost and latency hints
- timeout defaults
- observability labels
- policy requirements
- source reference or artifact reference

### State-of-the-art practice

Current agent systems increasingly rely on structured tool definitions with JSON-schema-like inputs and explicit capability metadata. MCP is important here because it standardizes how tools, resources, and prompts are exposed across clients and servers, but it is not by itself a complete internal capability model for workflows, policy, memory, or execution replay.

The useful pattern is:

- keep a normalized internal capability model
- add MCP as an interoperability surface
- avoid making MCP the only abstraction inside the repo

### Risks and anti-patterns

Avoid:

- treating markdown as the authoritative capability definition
- forcing every capability family into an over-generalized lowest-common-denominator model
- hiding important policy facts such as side effects or trust inside free-form prose

### Practical Elegy recommendation

Add a governed capability-definition schema family and keep its projections explicit:

- governed contract in `contracts/schemas/`
- minimal valid fixtures in `contracts/fixtures/`
- Rust registry and resolution crate under `rust/features/`
- markdown materializations only as contributor-facing projections where needed

---

## 2. Invocation and execution envelopes

### What this is

An invocation envelope is the normalized request and response contract for executing a capability.

This is the contract that carries runtime execution context, not just capability shape.

### Why it is good

Most multi-tool AI systems eventually need the same fields:

- correlation IDs
- execution IDs
- timeouts
- caller identity or trust context
- policy context
- input payload
- output payload
- structured failures
- trace references

When these are not normalized, every CLI, host, agent, and adapter invents slightly different formats. That makes replay, audit, policy gating, and cross-runtime invocation much harder than necessary.

An invocation envelope gives Elegy:

- one execution contract across CLIs, MCP hosts, and runtime crates
- stable handoff semantics for downstream apps
- a better base for replay, tracing, and policy enforcement
- clearer integration with workflow engines and task runners

### Where Elegy would use it

Elegy should use an invocation envelope across:

- `elegy` CLI execution surfaces
- `elegy-mcp` and `elegy-skills`
- future capability registry resolution
- workflow node execution
- retrieval stage execution
- high-risk adapter execution

### State-of-the-art practice

Modern workflow and durable-execution systems treat every step as a structured unit of work with explicit inputs, outputs, status, and event history. Temporal is a strong reference for why durable execution, event history, and replay matter in long-running or failure-prone automation flows.

For agent systems specifically, the strongest production pattern is:

- wrap side-effectful operations in explicit typed requests
- record structured outcomes
- attach correlation and trace metadata
- keep retries and compensation at the execution layer, not only in prompt text

### Risks and anti-patterns

Avoid:

- raw string payloads with no machine-readable failure model
- per-tool ad hoc result shapes
- silent retries with no execution record
- treating CLI stdout alone as the canonical execution record

### Practical Elegy recommendation

Add governed schema families for:

- invocation request
- invocation response
- execution event
- structured failure

Then map current CLI and MCP flows onto them in thin Rust layers.

---

## 3. Workflow and DAG IR with resumable execution

### What this is

A workflow IR is a declarative intermediate representation for multi-step execution.

At minimum, it should describe:

- nodes
- edges
- conditions
- retries
- approvals
- checkpoints
- compensation hooks
- timeout and schedule hints

Elegy already has a `canonical-workflow-graph` contract. The opportunity is to evolve that family toward durable execution semantics instead of leaving it as only a structural graph shape.

### Why it is good

The common failure mode in agent systems is letting the LLM act as the workflow engine.

That creates problems:

- no stable state model
- poor replay
- weak pause/resume behavior
- unclear approval boundaries
- fragile retries
- opaque failure analysis

A declarative workflow IR gives:

- deterministic orchestration for control flow
- bounded use of models inside nodes
- reliable checkpoints and recovery
- better policy enforcement points
- a cleaner split between reusable substrate and host-owned orchestration

### Where Elegy would use it

Elegy should use a workflow IR for reusable execution semantics that many consumers need, while leaving product-specific flow ownership in Holon or other hosts.

Good Elegy-owned pieces:

- workflow schema and fixtures
- executor traits
- checkpoint model
- event model
- approval and stop-condition contracts
- replay semantics

Keep consumer-local:

- business workflow definitions tied to app-specific UX
- tenant policy overlays
- operator approval UI
- orchestration state authority where it is product-owned

### State-of-the-art practice

The strongest industry pattern is hybrid:

- **deterministic orchestration** for branching, retries, compensation, and pausing
- **bounded model usage** inside steps for routing, extraction, summarization, or planning

Durability and replay are core ideas in systems like Temporal. Agent workflow frameworks such as LangGraph have also pushed the industry toward more explicit state graphs rather than purely free-form loops. The important architectural lesson is not to copy any one framework wholesale. It is to preserve:

- explicit state
- replayability
- inspectable transitions
- human-in-the-loop pause points

Recent agentic optimization work strengthens this further. AgentFlow's planner, executor, verifier, and generator split is a strong example of why reusable workflow substrates need typed step roles, evolving memory, and verifier-visible lifecycle events instead of a single prompt loop pretending to be orchestration.

### Risks and anti-patterns

Avoid:

- putting broad consumer-specific orchestration into Elegy
- building a giant workflow engine before the contracts and conformance corpus are stable
- overloading workflow nodes with prompt-only semantics and no clear executable boundary

### Practical Elegy recommendation

Build this in phases:

1. strengthen the workflow schema family
2. add typed checkpoint and lifecycle event contracts
3. add executor traits with replay requirements
4. add conformance examples for retries, approvals, and compensation

When this family matures, add conformance examples for:

- verifier-driven retries
- planner-to-executor handoff visibility
- evolving memory snapshots between turns
- final keep, reject, or escalate decisions tied to explicit outcome records

---

## 4. Retrieval and RAG pipeline contracts

### What this is

Retrieval should be modeled as a pipeline, not a single API call.

A useful reusable retrieval pipeline usually has stages like:

- query normalization
- query rewrite or decomposition
- candidate retrieval
- hybrid retrieval across sources
- reranking
- contextual compression
- citation packaging
- freshness and provenance annotation

### Why it is good

Traditional naive RAG usually collapses too many concerns into one retrieval step. That is now widely understood to be weak for real systems.

A pipeline model is better because it allows:

- different retrieval strategies for different query types
- hybrid lexical plus dense retrieval
- metadata filtering
- graph or relationship-aware retrieval where needed
- reranking before final context assembly
- explicit citation guarantees
- measurable evals at each stage

### Where Elegy would use it

Elegy should not own application-specific knowledge bases or search UI. It should own reusable retrieval contracts and traits that consumers can plug into.

Possible Elegy-owned pieces:

- retrieval pipeline schema
- stage traits in Rust
- result package contracts with citations and provenance
- retrieval metrics and eval fixtures

Possible consumer-owned pieces:

- domain-specific retriever implementations
- data-source credentials and tenancy
- ranking policies tied to specific product goals

### State-of-the-art practice

Production RAG has moved beyond “embed chunks and cosine search.” Stronger patterns now include:

- hybrid lexical and semantic retrieval
- hierarchical and specialized indexes
- query decomposition and rewrite
- reranking and compression
- chunk organization that preserves local context
- freshness-aware update strategies
- evaluation of grounding and citation quality

Microsoft’s advanced RAG guidance is a useful practical reference because it highlights ingestion, chunking, alignment, update strategy, hierarchical indexes, and evaluation as distinct concerns rather than one blob of retrieval logic.

Agentic RAG research also points toward adaptive retrieval pipelines that can reformulate questions, pick among sources, and iterate when initial recall is weak.

### Risks and anti-patterns

Avoid:

- equating vector search with RAG maturity
- pushing provider-specific retrieval details into the governed contract too early
- returning context blocks without citation or provenance
- writing retrieval abstractions so broadly that they cannot be evaluated

### Practical Elegy recommendation

Start with a narrow retrieval pipeline trait set:

- query rewrite
- candidate retrieval
- rerank
- compression
- citation packaging

Then add governed result contracts and fixture-based evals before expanding provider support.

---

## 5. Memory as a reusable substrate, not only a local CLI

### What this is

Memory should be treated as a lifecycle and contract problem, not just a storage problem.

The current `elegy-memory` surface already has useful primitives:

- salience gating
- scoped storage
- contradiction listing
- re-embed visibility

The next step is to turn those into a clearer reusable substrate shape.

### Why it is good

Most useful agent systems need more than one kind of memory:

- working or scratch memory
- episodic memory for actions and outcomes
- semantic memory for durable facts and preferences
- task or project memory for plans, decisions, and artifacts

These need different:

- retention rules
- write policies
- trust levels
- retrieval strategies
- consolidation schedules

Elegy is already closer to current best practice than many memory implementations because it emphasizes provenance, salience, contradiction handling, and scoped storage. Those are the right primitives.

### Where Elegy would use it

Elegy should use memory contracts and traits for reusable semantics and storage interfaces. Consumers should keep runtime ranking policy and host-specific currentness decisions when those are product-owned.

This matches the current repo guidance in `rust/features/elegy-memory/docs/architecture/v1.md`, which already keeps some runtime authority outside Elegy.

### State-of-the-art practice

Current memory systems increasingly separate:

- short-lived working memory
- episodic action memory
- durable semantic memory
- background consolidation or re-embedding
- provenance-aware trust distinctions such as user-confirmed versus inferred

Research and production systems increasingly treat memory writes as gated operations and memory upkeep as an asynchronous lifecycle problem. That lines up well with Elegy’s existing salience-gate direction.

Recent autonomous-improvement work adds another important pattern: systems improve faster when they retain structured execution memory, verifier outcomes, failure trails, and candidate-change lineage instead of only storing durable semantic facts. This does not mean storing raw transcripts. It means promoting distilled experiment and execution records into explicit memory kinds with their own retention and retrieval rules.

### Risks and anti-patterns

Avoid:

- storing raw transcripts as if they were durable memory
- a single undifferentiated memory table for all semantics
- treating inferred memory as equal to user-confirmed memory
- blocking writes on heavy embedding or consolidation work

### Practical Elegy recommendation

Split the future memory substrate into explicit reusable layers:

- memory record contracts
- scope and kind contracts
- write-intent and gate-decision contracts
- async consolidation and re-embed hooks
- freshness, contradiction, and provenance metadata

Keep the dedicated CLI, but position it as a thin operator shell over that substrate.

Add room for explicit memory kinds such as:

- semantic memory for durable facts and preferences
- episodic execution memory for actions, outcomes, and traces
- verifier memory for checks, failures, and acceptance decisions
- experiment memory for candidate changes, measured deltas, and promotion history

---

## 6. Progressive disclosure and context loading

### What this is

Progressive disclosure means exposing lightweight indexes, summaries, and handles first, then loading deeper material only when needed.

For AI systems, this applies to:

- skills
- tools
- memory records
- workflow state
- traces
- docs
- artifacts
- retrieval results

### Why it is good

This is one of the highest-leverage patterns for both quality and cost.

Without progressive disclosure:

- the prompt gets bloated
- useful signals get buried
- tool selection becomes noisy
- latency and token cost increase

With progressive disclosure:

- the model sees what exists before paying to fetch it
- the runtime can expose retrieval cost and relevance cues
- downstream systems can keep context compact but expandable

### Where Elegy would use it

Elegy should use progressive disclosure in:

- capability registry listing
- skill discovery
- memory listing and search
- trace inspection
- CLI output design
- future workflow inspection surfaces

### State-of-the-art practice

This pattern is increasingly explicit in agent ecosystems and skill systems. Claude-Mem’s context priming model is a strong concrete example: show a compact index first, then let the agent fetch relevant items on demand. The Supabase agent-skills open-standard work also demonstrates a three-level model:

1. metadata for discovery
2. body for conditional loading
3. references for deep on-demand detail

This is exactly the kind of pattern Elegy should encode into reusable contracts and CLI surfaces rather than leaving as ad hoc prompt advice.

### Risks and anti-patterns

Avoid:

- dumping all available tools or all memory into context at session start
- returning bodies without indexes
- hiding retrieval cost from the caller
- making progressive disclosure a human-only UX feature instead of an agent-usable contract

### Practical Elegy recommendation

Support three reusable disclosure layers:

- **index**: ids, titles, types, tags, token or size hints, risk hints
- **detail**: full contract body or record
- **deep reference**: linked source material, docs, artifacts, or histories

Then expose explicit “expand” operations in CLIs and future APIs.

---

## 7. MCP, skill routing, and tool selection

### What this is

This is the problem of how an agent or host decides what capability to use and how that capability is exposed.

MCP matters because it provides a common interoperability protocol for tools, resources, and prompts across AI clients and servers.

### Why it is good

MCP reduces bespoke connector work and improves portability across ecosystems. That makes it valuable for Elegy, especially given the repo’s interest in reusable bounded tooling.

At the same time, skill and tool routing quality depends on more than protocol compatibility. It also depends on:

- metadata quality
- side-effect classification
- trust and auth information
- cost and latency hints
- availability and scope

### Where Elegy would use it

Elegy should use MCP as:

- an interoperability layer
- an input or output surface for capability discovery
- a reusable host integration point

Elegy should not make MCP the only internal abstraction for:

- workflow execution
- memory semantics
- policy gating
- observability

### State-of-the-art practice

The strongest current pattern is:

- keep internal normalized models
- expose or consume MCP where ecosystem interoperability matters
- drive selection through capability metadata rather than prompt text alone

Anthropic’s MCP launch materials and the current MCP documentation reinforce the main point: standard protocol connectivity is useful and broadly supported, but downstream systems still need stronger internal governance around policies, execution context, and reusable semantics.

### Risks and anti-patterns

Avoid:

- assuming MCP eliminates the need for internal capability contracts
- selecting tools only from prompt wording
- routing to high-risk tools without effect and policy metadata

### Practical Elegy recommendation

Keep the current MCP tooling direction, but add:

- shared capability metadata
- resolver traits
- policy-aware routing hooks
- structured routing-eval fixtures

---

## 8. Machine-first CLI design for AI agents

### What this is

A machine-first CLI is a CLI that is still usable by humans but is deliberately safe and predictable for AI-driven invocation.

### Why it is good

AI systems increasingly use CLIs as bounded local execution surfaces. When a CLI is optimized only for humans, several things break:

- output parsing becomes brittle
- errors are ambiguous
- interactive prompts block automation
- side effects are harder to inspect ahead of time

Machine-first CLI design improves:

- automation reliability
- composability inside workflows
- auditability
- compatibility with model-driven and non-model-driven callers

### Where Elegy would use it

All current and future CLIs:

- `elegy`
- `elegy-memory`
- `elegy-mcp`
- `elegy-skills`

### State-of-the-art practice

The good pattern is consistent across robust automation-focused CLIs:

- `--json` output
- stable exit codes
- deterministic stdout and stderr behavior
- non-interactive flags
- explicit dry-run modes where state changes are possible
- correlation IDs or execution IDs in structured responses

This is especially important if Elegy is meant to support both standard AI apps and advanced automation systems.

Recent autonomous-improvement tooling suggests two additional CLI-friendly patterns that fit Elegy well:

- prefer bounded mutation surfaces over broad implicit edit authority
- prefer fixed-budget, machine-readable evaluation loops with explicit keep-or-discard outcomes

### Risks and anti-patterns

Avoid:

- human-only colorized prose as the sole output mode
- mixed structured and unstructured output in the same stream
- hidden interactive prompts
- inconsistent exit code semantics across sibling CLIs

### Practical Elegy recommendation

Adopt shared CLI rules across all operator binaries:

- required `--json` mode
- stable exit code inventory
- explicit `--non-interactive`
- correlation ID emission
- dry-run where applicable
- stable error object shape

Where a CLI performs analysis, generation, or transformation work that may later be auto-improved, also prefer:

- explicit input and output artifact references
- stable evaluation result objects
- machine-readable promotion or rejection reasons
- deterministic non-interactive operation for repeated benchmark runs

---

## 9. Policy gates, approvals, and high-risk adapters

### What this is

This is the layer that decides whether a requested action should be allowed, denied, simulated, or escalated for approval.

This matters for:

- filesystem access
- network access
- external messaging
- purchases or financial actions
- credential use
- desktop and OS control

### Why it is good

Prompt-only guardrails are weak for side-effectful systems. The more reliable pattern is execution-time policy enforcement with structured decisions and audit logs.

That gives:

- explicit allow or deny semantics
- inspectable reasons
- clearer approval paths
- better replay and compliance

### Where Elegy would use it

Elegy already has bounded filesystem and HTTP adapter directions. That is the right starting point.

Elegy should use policy gates for reusable execution boundaries and decision contracts. Consumers should keep product-specific approval UX and policy overlays where those are host-owned concerns.

Desktop automation, if it grows in Elegy, should remain a separate high-risk adapter family rather than being mixed into generic tool handling.

### State-of-the-art practice

Current AI safety and automation guidance increasingly emphasizes:

- least privilege
- explicit user consent for risky actions
- preview or dry-run for high-risk operations
- structured audit logs
- replay for incident review
- risk classification by action family

NIST AI RMF remains a useful governance baseline, and current industry guidance for AI security consistently reinforces the need for policy enforcement outside the prompt layer.

### Risks and anti-patterns

Avoid:

- treating desktop automation as just another low-risk tool
- open-ended shell generation as a default integration path
- running destructive commands without simulation or approval
- mixing policy truth into free-form docs only

### Practical Elegy recommendation

Define explicit contracts for:

- side-effect class
- policy decision
- approval requirement
- dry-run or simulation result
- stop condition and escalation reason

Keep dynamic CLI tooling as a bounded fallback only, consistent with the existing research note in `docs/research/dynamic-cli-tooling.md`.

---

## 10. Observability, tracing, and eval-by-contract

### What this is

This is the reusable layer for tracing execution, measuring quality, replaying failures, and validating behavior with fixtures and benchmarks.

### Why it is good

Without first-class observability and evals:

- workflow bugs are hard to localize
- retrieval quality drifts silently
- tool routing mistakes look like “model weirdness”
- policy failures are hard to prove

With structured traces and evals:

- each execution step becomes inspectable
- regressions can be caught before rollout
- replay becomes possible
- contract changes can be validated against fixtures

### Where Elegy would use it

Elegy should use observability and evals across:

- capability resolution
- invocation execution
- retrieval stages
- workflow nodes
- policy decisions
- memory writes and consolidation

### State-of-the-art practice

OpenTelemetry is the clearest reusable baseline for tracing concepts such as traces, spans, events, and correlation across multi-step systems. The exact backend can vary, but OTel-compatible instrumentation is the useful stable contract.

LLM and agent observability practice increasingly treats the following as first-class spans:

- model calls
- tool calls
- retrieval steps
- workflow nodes
- approval pauses
- policy decisions

The strongest production pattern is not observability only for debugging. It is observability tied to evaluation:

- retrieval relevance checks
- citation and grounding checks
- routing correctness checks
- workflow replay tests
- policy enforcement tests

### Risks and anti-patterns

Avoid:

- tracing only final answers
- no correlation IDs across execution layers
- eval claims with no fixtures
- relying only on human spot-checks for regressions

### Practical Elegy recommendation

Add reusable observability surfaces:

- OTel-compatible span and event helpers
- execution IDs and correlation IDs in all structured outputs
- fixture-based conformance suites
- eval harnesses for retrieval, routing, workflow replay, and policy gates

---

## 11. Desktop automation as a bounded capability family

### What this is

Desktop and OS automation includes actions such as:

- window and application control
- UI element targeting
- clipboard interactions
- file-open and save flows
- browser or app actuation on behalf of the user

### Why it is good

For advanced automation systems like Holon, this is strategically important because some useful workflows cannot be solved by API integration alone.

### Where Elegy would use it

Only where the capability is genuinely reusable across more than one consumer and can be bounded by explicit policy, audit, and replay semantics.

That means:

- intent and action-plan contracts may belong in Elegy
- reusable desktop adapter traits may belong in Elegy later
- host-owned orchestration and approval UX should stay outside Elegy

### State-of-the-art practice

The strongest safety pattern here is separation of:

- **intent**
- **plan**
- **actuation**
- **evidence**

Useful reusable ideas include:

- selectors or target descriptors
- idempotence hints
- before/after evidence
- approval gates for destructive actions
- replay logs

### Risks and anti-patterns

Avoid:

- introducing desktop automation before policy and approval contracts exist
- treating screenshots or free-form descriptions as sufficient execution truth
- folding desktop actions into generic low-risk tool routing

### Practical Elegy recommendation

Do not make this an early centerpiece.

Make it a later, separately gated adapter family after:

- policy contracts exist
- invocation envelopes exist
- workflow replay exists
- audit and trace semantics exist

---

## 12. A practical phased path for Elegy

### Near-term

Focus on the smallest reusable layers with the strongest evidence:

1. capability-definition contract
2. invocation-envelope contract
3. machine-first CLI rules
4. memory substrate split between contracts or traits and operator shell
5. narrow retrieval pipeline traits
6. OTel-compatible event and trace guidance
7. verifier-friendly execution event and outcome records

### Medium-term

Once the earlier contracts exist and are tested:

1. workflow IR with checkpoints, approvals, and replay semantics
2. capability registry and resolver crate
3. retrieval and routing eval harnesses
4. explicit policy-decision and stop-condition contracts
5. progressive disclosure primitives for indexes, details, and expansion
6. explicit experiment-result and verifier-result contracts for bounded improvement loops

### Later

Only after reuse and governance are proven:

1. separately gated desktop adapter family
2. remote or shared capability registry support
3. richer hybrid workflow execution with bounded model-driven planning nodes
4. more advanced memory consolidation workers and cross-run semantics

---

## 13. What should stay out of Elegy

To stay aligned with current repo guidance, avoid moving these into Elegy unless reuse becomes undeniable and boundaries stay clean:

- product-specific orchestration authority
- approval UX
- tenant-specific auth and policy overlays
- prompt assembly strategies tied to one host
- broad app-specific planner behavior
- control-plane ownership for Holon or similar hosts

This matters because the repo is strongest when it stays a reusable substrate, not when it absorbs host logic.

---

## 14. Sources and useful references

### Elegy internal context

- [docs/architecture/ecosystem-topology.md](../architecture/ecosystem-topology.md)
- [docs/architecture/substrate-governance.md](../architecture/substrate-governance.md)
- [docs/architecture/mcp-skill-tooling-placement.md](../architecture/mcp-skill-tooling-placement.md)
- [docs/architecture/skill-core-v1.md](../architecture/skill-core-v1.md)
- [rust/features/elegy-memory/docs/architecture/v1.md](../../rust/features/elegy-memory/docs/architecture/v1.md)
- [docs/research/dynamic-cli-tooling.md](./dynamic-cli-tooling.md)
- [docs/research/openclaw-orchestration-gap-roadmap.md](./openclaw-orchestration-gap-roadmap.md)
- [rust/README.md](../../rust/README.md)

### External references

- Model Context Protocol introduction: <https://modelcontextprotocol.io/introduction>
- Anthropic MCP announcement: <https://www.anthropic.com/news/model-context-protocol>
- NIST AI Risk Management Framework: <https://www.nist.gov/itl/ai-risk-management-framework>
- OpenTelemetry traces overview: <https://opentelemetry.io/docs/concepts/signals/traces/>
- Temporal workflow execution and replay: <https://docs.temporal.io/workflow-execution>
- Microsoft advanced RAG guidance: <https://learn.microsoft.com/en-us/azure/developer/ai/advanced-retrieval-augmented-generation>
- Agentic RAG survey: <https://arxiv.org/abs/2501.09136>
- Claude-Mem progressive disclosure note: <https://docs.claude-mem.ai/progressive-disclosure>
- Supabase agent-skills progressive disclosure model: <https://deepwiki.com/supabase/agent-skills/2.1-progressive-disclosure-model>
- AgentFlow project page: <https://agentflow.stanford.edu/>
- SOAR paper: <https://arxiv.org/abs/2407.20635>
- Karpathy autoresearch repository: <https://github.com/karpathy/autoresearch>

## Final recommendation

Elegy should aim to be the reusable, governed substrate that standard AI apps and advanced automation systems can both build on.

That means:

- explicit reusable contracts
- Rust-first reusable execution traits
- thin operator-facing CLIs
- strong policy boundaries
- first-class traces and evals

It does **not** mean making Elegy the default owner of host orchestration, app control planes, or prompt strategy.

If the repo follows that shape, it can become the place where reusable AI infrastructure gets stabilized without collapsing product-specific behavior into the substrate.
