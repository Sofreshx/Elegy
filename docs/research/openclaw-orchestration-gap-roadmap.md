# Research: OpenClaw orchestration gap roadmap

Updated: 2026-03-25

This note is research-oriented and non-canonical. It is a target-direction memo for future work, not a source of current-state truth.

OpenClaw is inspiration only. It is not truth, not canon, and not a migration template. The point of using it here is to sharpen what Holon or SAASTools still lacks, not to make OpenClaw normative.

## Current canon truth

- Operator-facing orchestration and control-plane ownership remains with Holon or SAASTools.
- Current SAASTools canon still treats DesktopHost, AppHost, the frontend surfaces, and the APIs as active or transitional parts of the product shape. Do not read this note as saying the APIs are already removed.
- Any later API de-emphasis, consolidation, or pruning is future target direction only.
- Elegy remains bounded to reusable CLI, skill, contract, and runtime surfaces with proven reusable value. It does not own the Holon product runtime or control plane.

## Session status

Status in this session: direction clarified, ownership preserved, shipped baseline acknowledged, next seam narrowed.

### What was decided in this session

- Holon or SAASTools should continue to own orchestration and control-plane behavior.
- The delegated typed-output first slice is already shipped in DesktopHost for one bounded delegated child hop.
- A narrow local delegated pre-start run policy seam has started in code and stops execution for missing persisted success criteria, unresolvable subagent type, and unregistered delegated output schema.
- The most meaningful immediate next step is to surface those stop conditions and lifecycle outcomes explicitly instead of reopening the already-shipped typed-output slice.
- A replaceable provider or session-manager abstraction above the existing backplane remains an important later gap. It should support Copilot SDK now and raw API providers later, but it is no longer the immediate next seam in this sequencing.
- Copilot-compatible delegated depth should stay at one bounded child or subagent hop by default.
- Greater depth can be added later for raw API providers, but only behind explicit host policy, budgets, and stop conditions.
- Relevant Elegy CLI capabilities can be integrated where useful, especially `elegy-mcp`, `elegy-skills`, and `elegy-memory`.
- If a capability cannot materially become a bounded CLI plus skill path or a strong reusable shared capability, it should stay in Holon or SAASTools.

### What remains carryover

- extend typed result envelopes beyond the shipped delegated baseline
- surface the current local delegated pre-start stop conditions through explicit lifecycle and execution-tree truth
- define broader typed policy and stop-condition contracts
- define the minimum orchestration event family and execution-tree truth surface
- map the current Copilot SDK backplane into a later provider or session-manager seam without breaking host ownership
- identify which first uses of `elegy-mcp`, `elegy-skills`, and `elegy-memory` are genuinely reusable instead of just convenient to extract

## Shipped baseline and immediate follow-on

The first practical slice is no longer hypothetical. DesktopHost already ships the bounded delegated typed-output baseline, and the next immediate follow-on is to make the new stop conditions visible as orchestration truth rather than leaving them mostly local to runtime code.

- DesktopHost-owned execution-tree substrate with one bounded delegated child hop by default.
- Typed delegated output baseline with host-owned named schemas, persisted accepted delegation intent, and host-side JSON validation.
- Structured delegated pre-start run policy that blocks missing persisted success criteria, unresolvable subagent type, and unregistered output schema.
- Immediate follow-on: explicit stop-condition and lifecycle surfacing for that started policy seam.
- Later important seam: provider or session-manager abstraction that supports Copilot SDK now and raw API providers later.
- Elegy integrated only as bounded host-managed CLI capabilities or reusable contracts where that boundary is justified.

This sequencing matters because the current system already has meaningful host-owned orchestration, but its contract plane is still weaker than its control plane. The next useful move is to make the new stopping rules visible and inspectable before widening delegation depth, broadening provider seams, or chasing more product breadth.

## Priority gaps to foreground in Holon or SAASTools

These are the important gaps to drive first. They matter more than broad OpenClaw feature comparison.

### 1. Explicit stop-condition and lifecycle surfacing for the started delegated policy seam

DesktopHost now has local delegated pre-start stop codes for missing persisted success criteria, unresolvable subagent type, and unregistered output schema. The immediate gap is that those stops are still mostly local runtime outcomes instead of clear execution-tree or lifecycle truth.

This is the most meaningful next step because the seam has started in code and now needs explicit state instead of only failure text.

### 2. Broader typed policy and stop-condition contracts

The local delegated seam is only a start. Approval posture, budget exhaustion, invalid structured output, capability denial, escalation, and degradation should still be described as explicit contracts instead of staying mostly implicit in runtime code and prompts.

### 3. Broader typed result contracts beyond the shipped delegated baseline

The delegated one-hop typed-output slice is now shipped. The remaining result gap is extending typed envelopes for orchestrator outputs, aggregation, validation, rejection, and repair beyond that bounded delegated path.

### 4. Stronger self-description and capability disclosure

The runtime should be able to say, in a host-truthful way, which capabilities are available, why they are available, what constraints apply, and what execution posture is active for this turn.

### 5. Orchestration event formalization and execution-tree truth surfaces

The system needs a typed event family and execution-tree truth model that can explain routing, delegation, tool use, stop reasons, validation failures, and final aggregation without relying mainly on prose summaries or UI-only inspection.

### 6. Provider or session-manager abstraction above the current backplane

The backplane contract is already useful, and a higher seam is still important later. It can manage provider sessions consistently, preserve host policy and execution ownership, and make Copilot SDK versus raw API support a replaceable implementation choice rather than a structural fork, but it is no longer the immediate next seam in the sequencing above.

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

If future work starts from this note, start from the shipped delegated baseline and the started local policy seam inside Holon or SAASTools, then treat any Elegy extraction as secondary proof work.

The default bias should be simple:

- keep orchestration local
- keep the shipped delegated typed-output baseline strict
- surface explicit stop conditions and lifecycle truth next
- broaden results, policies, and execution truth before widening provider seams
- keep the provider or session-manager seam as a later important gap
- leave raw API provider expansion as a planned follow-on
- extract only the parts that are clearly reusable as CLI, skill, contract, or bounded runtime capability