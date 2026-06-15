---
created: 2026-04-25
updated: 2026-05-28
category: integration
status: active
doc_kind: roadmap
---

# AI Agent Integration Roadmap

## Purpose

This roadmap captures the current integration audit for making Elegy easy, safe,
and efficient for AI agents to consume from any downstream host.

It focuses on design gaps and implementation direction. Holon is an important
consumer example, but it is not the center of the Elegy architecture. The same
contracts, CLIs, MCP surfaces, memory primitives, policy decisions, and
distribution paths should be usable from any product, local agent runtime,
editor extension, automation shell, or hosted control plane that wants to
integrate AI-agent tools.

## Evidence checked

The audit is based on the current repository structure, architecture docs,
contracts, Rust crates, skill fixtures, and these live checks:

- `cargo run -p elegy-cli -- --version --json`
- `.\target\debug\elegy.exe skills list --json`
- `.\target\debug\elegy.exe skills describe --skill-id desktop --json`
- `.\target\debug\elegy.exe desktop click --x 1 --y 1 --dry-run --json`
- `cargo test -p elegy-contracts -p elegy-host-mcp -p elegy-cli -p elegy-desktop -p elegy-observe -p elegy-memory`

The targeted Rust test set passed. The current gaps are mostly contract,
integration, policy, and agent-ergonomics gaps rather than basic build failures.

## Status note, 2026-05-28

Phase 1 is no longer just planned work.

- Shared CLI machine-output and structured-failure helpers now back
  `elegy-skills`, `elegy-configuration`, and `elegy-memory`.
- Real-output conformance tests now validate those dedicated CLI surfaces
  against governed invocation and structured-failure contracts.
- `elegy-host-mcp` now returns recognized Elegy machine envelopes as structured
  MCP results instead of flattening them to text, and host-generated denials or
  timeouts are also structured.

Phase 2 is actively in progress.

- Built-in Skill capabilities now project into real
  `CapabilityDefinition` payloads.
- `elegy skills capability` and `elegy-skills capability` now emit that
  contract directly.
- `elegy agent discover` now routes profile filtering and search through the
  shared `SkillRegistry` search/filter path instead of its own local scorer.

Remaining near-term work is still real.

- Normalize the dedicated `elegy-mcp` CLI onto the shared top-level `failure`
  envelope and add real-output conformance coverage there.
- Add governed projected-capability fixture or contract-level conformance
  coverage without introducing an `elegy-contracts` to `elegy-skills` crate
  cycle.
- Finish the remaining `elegy agent` helper consolidation around shared
  profile/selection data.
- Propagate invocation context and install-metadata executable resolution
  through MCP subprocess dispatch.

## Current read

Elegy is a reusable contracts-and-tooling substrate. Its durable authority lives
in governed schemas, fixtures, manifests, and policy artifacts. Its executable
surface lives in Rust crates and thin CLIs.

The current agent-facing surface is real:

- `elegy skills list/search/describe --json` exposes Skill discovery with
  progressive disclosure.
- `elegy run` hosts the built-in capabilities through MCP.
- `elegy desktop` provides bounded desktop automation primitives with dry-run
  support.
- `elegy observe` provides desktop and OS observation primitives.
- `elegy-memory` implements governed local memory with summary-only records,
  provenance, salience gates, lifecycle states, keyword search, and optional
  embedding-backed retrieval.
- Contract schemas already exist for capability definitions, invocation
  requests, invocation responses, execution events, and structured failures.

The current docs also make the intended boundary clear: downstream hosts own
operator-facing orchestration, control-plane behavior, approval UX, provider
session management, product policy, and host lifecycle. Elegy owns reusable
contracts, bounded primitives, and machine-readable execution surfaces.

## Host boundary decision

Host desktop applications and host CLIs are not Elegy repo centers. Based on the
current Elegy docs, they should be treated as downstream host surfaces that
consume Elegy rather than surfaces that Elegy should absorb.

Desktop hosts such as Holon Desktop should exist because local agent work needs
a high-trust product control plane: human approval, execution-tree visibility,
stop conditions, desktop evidence, provider/session state, local app lifecycle,
and recovery flows. Elegy should provide reusable desktop and observation
primitives, not product-specific operator UX.

Host CLIs should exist because products need local bootstrap and operations
seams: pinning an Elegy release, installing selected assets, running smoke
tests, invoking host-specific tasks, and exposing product-specific automation
without polluting Elegy with product assumptions.

Elegy should stay small and reusable: contracts, registry/discovery,
invocation envelopes, policy decisions, memory/retrieval primitives, MCP
hosting, and bounded adapters.

## Non-goals

- Do not move downstream desktop apps, downstream CLIs, or host orchestration
  into Elegy.
- Do not introduce a monolithic agent runtime in Elegy.
- Do not make MCP the only internal abstraction.
- Do not expand desktop automation without stronger policy, evidence, and
  dry-run semantics.
- Do not reintroduce .NET or package-feed-centered distribution as the primary
  integration lane.

## Main gaps

### P1: Contracts exist, but runtime invocation does not yet flow through them

The governed schemas for `capability-definition`, `invocation-request`,
`invocation-response`, `execution-event`, and `structured-failure` exist and
have conformance tests. The executable surfaces do not yet consistently use
those contracts as the runtime path.

Examples:

- `elegy` emits an `elegy.cli/v1` envelope.
- Dedicated surfaces such as `elegy-memory` use narrower JSON shapes.
- The MCP host returns subprocess stdout as text content instead of validating
  or normalizing it into an invocation response.
- Policy denials and subprocess failures are often plain text rather than
  structured failures.

Direction:

- Add a shared invocation/output module or crate used by `elegy`,
  `elegy-memory`, `elegy-mcp`, and `elegy-skills`.
- Map every machine command to `InvocationRequest` and `InvocationResponse`
  where practical.
- Preserve CLI-specific convenience envelopes only as projections of the shared
  contract.
- Emit structured failures for policy denials, validation failures, subprocess
  failures, unsupported platforms, and timeouts.

Acceptance:

- Every shipped CLI can emit a shared machine envelope with status,
  correlation, diagnostics, and structured failures.
- At least one MCP tool call round-trips through invocation request/response
  fixtures.
- Conformance tests validate real CLI output, not only static fixtures.

### P1: Machine JSON is close, but not uniform enough for agents

The `elegy` CLI has a useful JSON envelope, but the current behavior leaves
friction for automated agents:

- `correlationId` is emitted as an empty string when the caller does not provide
  one.
- Some desktop error paths can return an error summary without the same
  structured diagnostic shape used elsewhere.
- Dedicated CLIs do not share the same status, diagnostics, correlation, and
  schema fields.
- `--non-interactive` exists on the umbrella CLI but does not yet represent a
  universal behavior contract across all surfaces.

Direction:

- Define one machine-output contract for all Rust CLIs.
- Either generate a correlation ID when absent or omit it when absent; do not
  emit blank IDs.
- Standardize exit codes and diagnostic codes.
- Ensure JSON stdout is deterministic and stderr is reserved for human/log
  noise.
- Make `--non-interactive` mean no prompts, bounded execution, and structured
  failure on missing required inputs.

Acceptance:

- Golden tests cover success, validation failure, policy denial, unsupported
  platform, and subprocess failure for each shipped CLI family.
- All machine outputs include or clearly omit correlation IDs according to one
  rule.
- Agents can reliably parse status and failure causes without string matching.

### P1: Side-effect policy is too coarse for high-risk tools

The MCP host blocks side-effecting tools unless dry-run is requested or
`--allow-side-effects` is enabled. That is the right first guard, but it is too
coarse for product-grade local automation.

Current gaps:

- Side-effect classes, approval requirements, trust levels, and policy refs are
  described in contracts but not uniformly enforced at invocation time.
- Dry-run is detected from tool inputs and subprocess templates, not from a
  typed action contract.
- Denials are returned as plain MCP text, not a structured policy decision.
- Desktop automation is marked `approvalRequirement: none` while also carrying
  `riskLevel: high`; this is defensible for local CLI dry-runs but too weak as
  the default semantics for hosted automation.

Direction:

- Introduce a reusable policy decision shape with `allow`, `deny`,
  `requiresApproval`, reason codes, matched policy refs, and redaction metadata.
- Apply the same policy decision path in CLI and MCP invocation.
- Treat high-risk non-dry-run desktop actions as approval-required by default in
  host integrations, even if the low-level primitive can execute locally.
- Keep the approval UX in downstream hosts, but make the policy contract
  reusable in Elegy.

Acceptance:

- MCP and CLI both return structured policy denials.
- Desktop non-dry-run actions can be represented as approval-required before
  execution.
- Dry-run, preview, approved execution, and denied execution are distinguishable
  in machine output.

### P1: Capability discovery is good, but capability governance is split

The Skill registry is a strong agent-facing discovery surface. Separately,
the capability-definition schema has richer governance and execution metadata.
Those two layers are not yet a single governed capability registry.

Direction:

- Define the skill-to-capability projection as a first-class contract.
- Add conformance tests proving every built-in Skill capability can project
  into `capability-definition` without losing required governance fields.
- Expose an agent-friendly capability view that includes side-effect class,
  idempotence, auth mode, trust level, cost/latency hints, and observability
  labels.

Acceptance:

- Built-in skill definitions project into capability-definition fixtures.
- Agents can inspect a capability without loading the full skill definition.
- The MCP host uses the same normalized capability metadata as CLI discovery.

### P2: MCP subprocess dispatch needs stronger context and result handling

The MCP host correctly builds tools from built-in skill definitions and has
useful side-effect guards. The current subprocess bridge remains thin.

Current gaps:

- No invocation context propagation for project path, correlation ID, execution
  ID, or policy context.
- Subprocess stdout is returned as MCP text content without schema validation.
- Dedicated executables rely on PATH unless the current process can stand in for
  `elegy`.
- Tool failures are not normalized into structured failures.

Direction:

- Pass invocation context explicitly to subprocess tools.
- Parse and validate JSON outputs when a capability advertises a machine schema.
- Return structured MCP content for invocation responses and structured
  failures.
- Resolve executable paths from an install receipt or manifest instead of only
  PATH/current executable heuristics.

Acceptance:

- MCP tool calls can carry correlation IDs through to CLI outputs.
- A bad subprocess exit produces a structured failure with exit code, command
  family, timeout/output-limit status, and safe diagnostic text.
- Installed downstream layouts can be validated before the host advertises
  tools that depend on missing binaries.

### P2: Desktop automation is a primitive, not yet a host-grade action model

The low-level desktop crate has sensible dry-run support for click, type, key,
and window actions. It should remain a reusable primitive. Desktop hosts need a
higher-level action model before relying on it for autonomous workflows.

Current gaps:

- No typed desktop intent/action-plan contract.
- No explicit before/after evidence contract.
- Dry-run evidence is thinner for some window-targeting paths.
- Title matching is described as strict but implemented as unambiguous
  case-insensitive substring matching.
- There is no reusable replay or verification model for desktop actions.

Direction:

- Define a desktop intent contract with target, action, risk, dry-run preview,
  required evidence, and expected verification.
- Keep actual approval, UI, and recovery loops in downstream desktop hosts.
- Make Elegy desktop outputs evidence-rich enough for a host to review and log.
- Clarify matching semantics in skill docs and tests.

Acceptance:

- Dry-run output can show the intended action and target evidence without
  mutating state.
- Host integrations can require approval on the intent before calling the
  primitive executor.
- Window matching docs, skill definitions, CLI help, and implementation agree.

### P2: Memory is strong, but retrieval integration needs a shared package

`elegy-memory` has a serious local memory core: summary-only records,
provenance, salience gates, lifecycle state, SQLite storage, FTS, and optional
embeddings. The integration gap is not the core model; it is how agents consume
retrieval results consistently.

Current gaps:

- Dedicated memory CLI output does not use the same envelope as `elegy`.
- Retrieval results do not yet have a shared agent package with query metadata,
  fallback mode, provider status, score breakdown, and safe snippets.
- Provider fallback can be graceful, but downstream agents need an explicit
  signal when semantic retrieval degraded to keyword retrieval.

Direction:

- Define a retrieval-result contract for agent consumption.
- Include provider status and fallback mode in search responses.
- Keep the no-raw-transcript rule as a hard contract.
- Add bridge examples showing downstream prompts consuming retrieved memory
  without storing raw conversation text.

Acceptance:

- Agents can tell whether a result set is keyword-only, semantic, or degraded.
- Memory CLI output follows the shared machine-output contract.
- Tests prove raw transcripts are rejected across import, storage, and
  projection paths.

### P2: Distribution is documented, but downstream proof should be explicit

The distribution docs now describe release-asset consumption and a downstream
quick start. The remaining risk is proof that a host can pin, install, discover,
and invoke Elegy without repo-local assumptions.

Direction:

- Add a downstream fixture or smoke script that installs from local artifacts,
  runs `skills list`, validates contracts, and invokes one safe dry-run tool.
- Record the minimum files a downstream CLI needs to vendor or pin.
- Keep package-feed and sibling-repo assumptions deprecated.

Acceptance:

- A clean downstream fixture can install selected Elegy surfaces from local
  artifacts and from a release tag.
- Smoke output proves binaries, contracts, wrappers, and manifests are aligned.
- `docs/issues/unresolved-goals.md` can retire the distribution verification
  item.

### P3: Observability exists as contracts, but not as a runtime trace

Execution-event schemas exist, and CLI envelopes carry useful command/status
facts. There is not yet a unified event stream for agent runs.

Direction:

- Emit execution events from CLI and MCP host paths.
- Include policy decisions, dry-run previews, subprocess starts/exits,
  output-limit truncation, and structured failures.
- Keep event shape reusable; let downstream hosts decide storage, UI, and
  retention.

Acceptance:

- A single agent action can produce a bounded event sequence linked by
  correlation ID and execution ID.
- A downstream host can render the sequence without understanding internal Rust
  implementation details.

## Phased plan

### Phase 0: Correct the documented baseline

Owner: Elegy.

Work:

- Mark old roadmap sections that describe existing contract schemas as future
  work as superseded by current implementation.
- Add a concise host/Elegy boundary note to the architecture docs if downstream
  confusion continues.
- Record the live CLI/test evidence used by this audit.

Exit:

- Docs distinguish implemented contracts from remaining runtime wiring.
- Desktop apps and product CLIs are described as downstream host surfaces, not
  Elegy repo centers.

### Phase 1: Unify machine output and invocation contracts

Owner: Elegy.

Status, 2026-05-28:

- Complete for shared machine-envelope and structured-failure adoption in
  `elegy-skills`, `elegy-configuration`, and `elegy-memory`.
- Complete for real-output conformance on those dedicated CLIs.
- Complete for structured MCP result handling in `elegy-host-mcp` when
  subprocess stdout is a recognized Elegy machine envelope.
- Remaining: dedicated `elegy-mcp` CLI normalization and any still-uncovered
  runtime paths that bypass the shared failure shape.

Work:

- Add shared Rust types/helpers for machine output, diagnostics, structured
  failures, and invocation response projection.
- Normalize `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills` JSON
  behavior.
- Fix blank correlation ID behavior.
- Add golden tests for parseable output and failure modes.

Exit:

- AI agents can invoke every shipped CLI with predictable JSON semantics.
- Failures no longer require string matching.

### Phase 2: Build the normalized capability registry

Owner: Elegy.

Status, 2026-05-28:

- In progress.
- Done so far: built-in capability-definition projection, direct capability
  inspection output from both `elegy` and `elegy-skills`, shared typed MCP tool
  bindings, and the first `elegy agent discover` dedup onto shared registry
  filtering/search.
- Remaining: governed projected-capability fixtures or contract-level
  conformance, remaining `elegy agent` helper consolidation, and dedicated
  `elegy-mcp` capability-model alignment.

Work:

- Project skill definitions into capability definitions.
- Add validation that built-in capabilities carry required governance and
  execution metadata.
- Use normalized capability metadata in CLI discovery and MCP tool listing.

Exit:

- Agents can cheaply inspect capability metadata without loading every full
  skill definition.
- MCP, CLI, and contracts agree on side effects and governance metadata.

### Phase 3: Harden MCP invocation

Owner: Elegy.

Work:

- Route MCP `tools/call` through invocation request/response handling.
- Add structured policy denials.
- Propagate correlation/execution/project context.
- Validate subprocess output when a schema is advertised.
- Resolve executables through install metadata.

Exit:

- MCP calls are as machine-stable as direct CLI calls.
- Missing binaries, policy denials, validation failures, and subprocess failures
  have structured, test-covered outputs.

### Phase 4: Define the desktop action contract

Owner: Elegy for primitive contracts and executor output; downstream hosts for
approval UX, orchestration, replay UI, and recovery.

Work:

- Define desktop intent, dry-run preview, execution evidence, and verification
  contracts.
- Align skill docs, CLI help, and implementation around window matching.
- Make high-risk desktop actions approval-required in host policy by default.

Exit:

- A downstream desktop host can review a desktop intent before execution and
  show evidence after execution.
- Elegy remains a bounded primitive provider.

### Phase 5: Package memory retrieval for agents

Owner: Elegy for retrieval contracts and memory CLI output; downstream hosts
for prompt assembly and user-facing memory controls.

Work:

- Add retrieval-result contracts with score metadata and fallback mode.
- Normalize memory CLI JSON output.
- Add examples for summary-only retrieval consumption.

Exit:

- Agents can consume memory results efficiently and know when retrieval degraded.
- Raw transcript storage remains impossible through public import/storage paths.

### Phase 6: Prove downstream distribution

Owner: Elegy with downstream host validation input.

Work:

- Add a downstream install smoke fixture.
- Validate contracts, selected binaries, wrapper archives, and dry-run
  invocation from a clean tools directory.
- Document the minimal downstream CLI pin/install/invoke sequence.

Exit:

- A downstream host can consume Elegy through release assets without sibling
  checkout or package-feed assumptions.

### Phase 7: Add execution events and evaluation hooks

Owner: Elegy for event contracts and emission; downstream hosts for UI/storage.

Work:

- Emit execution events from CLI and MCP host paths.
- Add bounded traces for policy checks, dry-run previews, subprocess execution,
  and failures.
- Add fixture-based eval cases for agent discovery and invocation flows.

Exit:

- Downstream hosts can render and evaluate agent tool usage from reusable Elegy
  events.
- Elegy can prove agent ergonomics by contract and fixture, not only docs.

## Immediate next work

1. Normalize the dedicated `elegy-mcp` CLI onto the shared top-level
   `failure` envelope and add real-output conformance coverage.
2. Add governed projected-capability fixture or contract-level conformance
   coverage without reintroducing an `elegy-contracts` to `elegy-skills` crate
   cycle.
3. Finish the remaining `elegy agent` helper consolidation around shared
   profile/selection data.
4. Propagate invocation context through MCP tool calls and replace workspace
   fallback executable resolution with install-metadata validation.
5. Clarify desktop window matching semantics in skill docs and CLI help.
6. Add downstream distribution smoke coverage for a clean generic host install.
7. Add a shared retrieval-result package for agent-facing memory consumption.

## Decision summary

The project is directionally well set up for AI-agent integration: discovery is
machine-readable, contracts are present, the Rust surfaces build and test, and
host-specific orchestration is mostly kept out of Elegy.

The highest-leverage correction is to make the existing governed contracts the
actual runtime contract across CLI, MCP, memory, desktop, policy, and
distribution. Once that is done, downstream desktop apps, CLIs, and hosted
control planes can stay focused on product control-plane work while consuming
Elegy as a stable, low-context, agent-friendly substrate.
