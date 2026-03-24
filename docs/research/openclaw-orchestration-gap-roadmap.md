# Research: OpenClaw orchestration gap roadmap

Updated: 2026-03-24

This note is research-oriented and non-canonical. It is a target-direction memo for future work, not a source of current-state truth.

OpenClaw is inspiration only. It is not truth, not canon, and not a migration template. The point of using it here is to sharpen what Holon or SAASTools still lacks, not to make OpenClaw normative.

## Current canon truth

- Operator-facing orchestration and control-plane ownership remains with Holon or SAASTools.
- Current SAASTools canon still treats DesktopHost, AppHost, the frontend surfaces, and the APIs as active or transitional parts of the product shape. Do not read this note as saying the APIs are already removed.
- Any later API de-emphasis, consolidation, or pruning is future target direction only.
- Elegy remains bounded to reusable CLI, skill, contract, and runtime surfaces with proven reusable value. It does not own the Holon product runtime or control plane.

## Session status

Status in this session: direction clarified, ownership preserved, first slice identified.

### What was decided in this session

- Holon or SAASTools should continue to own orchestration and control-plane behavior.
- The next major seam should be a replaceable provider or session-manager abstraction above the existing backplane.
- That seam should support Copilot SDK now and raw API providers later.
- Copilot-compatible delegated depth should stay at one bounded child or subagent hop by default.
- Greater depth can be added later for raw API providers, but only behind explicit host policy, budgets, and stop conditions.
- Relevant Elegy CLI capabilities can be integrated where useful, especially `elegy-mcp`, `elegy-skills`, and `elegy-memory`.
- If a capability cannot materially become a bounded CLI plus skill path or a strong reusable shared capability, it should stay in Holon or SAASTools.

### What remains carryover

- define the minimum typed result envelope for orchestrator and delegated outputs
- define typed policy and stop-condition contracts
- define the minimum orchestration event family and execution-tree truth surface
- map the current Copilot SDK backplane into the new provider or session-manager seam without breaking host ownership
- identify which first uses of `elegy-mcp`, `elegy-skills`, and `elegy-memory` are genuinely reusable instead of just convenient to extract

## First practical slice

The recommended first phase is a Holon or SAASTools substrate upgrade, not an OpenClaw feature chase.

- DesktopHost-owned execution-tree substrate with one bounded delegated child hop by default.
- Typed aggregation or result envelope for parent and delegated execution.
- Hard budgets and typed stop conditions enforced by the host.
- Provider or session-manager seam that supports Copilot SDK now and raw API providers later.
- Elegy integrated only as bounded host-managed CLI capabilities or reusable contracts where that boundary is justified.

This first slice matters because the current system already has meaningful host-owned orchestration, but its contract plane is still weaker than its control plane. The goal is to make execution truth, stopping rules, and result reuse more formal before pursuing broader delegation depth or more product breadth.

## Priority gaps to foreground in Holon or SAASTools

These are the important gaps to drive first. They matter more than broad OpenClaw feature comparison.

### 1. Typed result contracts

The host needs typed result envelopes for orchestrator outputs, delegated outputs, aggregation, validation, rejection, and repair. This is the highest-value gap because the runtime already knows how it wants to route work, but it is still weaker at proving what it received.

### 2. Typed policy and stop-condition contracts

Approval posture, budget exhaustion, invalid structured output, capability denial, escalation, and degradation should be described as explicit contracts instead of staying mostly implicit in runtime code and prompts.

### 3. Stronger self-description and capability disclosure

The runtime should be able to say, in a host-truthful way, which capabilities are available, why they are available, what constraints apply, and what execution posture is active for this turn.

### 4. Orchestration event formalization and execution-tree truth surfaces

The system needs a typed event family and execution-tree truth model that can explain routing, delegation, tool use, stop reasons, validation failures, and final aggregation without relying mainly on prose summaries or UI-only inspection.

### 5. Provider or session-manager abstraction above the current backplane

The backplane contract is already useful, but the next step is a higher seam that can manage provider sessions consistently, preserve host policy and execution ownership, and make Copilot SDK versus raw API support a replaceable implementation choice rather than a structural fork.

## Where OpenClaw is useful

OpenClaw is still a useful research input for:

- more explicit operator-facing policy and audit posture
- stronger self-description and capability visibility
- clearer execution or delegation boundaries
- better continuity from setup into runtime control

That said, OpenClaw should influence the questions, not decide the ownership model.

## Elegy fit within this direction

Good candidates for Elegy remain bounded and reusable:

- CLI-managed helpers that a host can invoke deliberately, such as `elegy-mcp`, `elegy-skills`, or `elegy-memory`
- governed contracts for typed result envelopes, policy or stop-condition descriptors, capability disclosure metadata, or event envelopes if multiple consumers need them
- strong reusable runtime helpers only after real cross-consumer proof exists

Keep these local to Holon or SAASTools unless and until reuse is proven:

- runtime control-plane ownership
- execution-tree authority and orchestration state
- operator UX, approval flows, and policy enforcement
- provider routing, session ownership, and prompt assembly
- API lifecycle, de-emphasis, or pruning decisions

## Non-goals and guardrails

- not deleting or declaring removal of the current APIs in this session
- not moving runtime control-plane ownership into Elegy
- not making OpenClaw normative
- not starting with open recursive swarm behavior
- not widening delegation depth before budgets, stop conditions, and execution-tree truth are formalized

## Practical next move

If future work starts from this note, start with the first practical slice inside Holon or SAASTools and treat any Elegy extraction as secondary proof work.

The default bias should be simple:

- keep orchestration local
- formalize results, policies, and execution truth first
- support Copilot SDK immediately through a replaceable provider seam
- leave raw API provider expansion as a planned follow-on
- extract only the parts that are clearly reusable as CLI, skill, contract, or bounded runtime capability