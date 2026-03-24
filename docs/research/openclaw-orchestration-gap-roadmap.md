# Research: OpenClaw Orchestration Gap Roadmap

This note is research-oriented guidance, not a canonical ownership change. It compares OpenClaw and a small set of adjacent agent products against the current Elegy plus SAASTools split, with emphasis on the gaps that matter for operator-facing setup, safety, policy, and runtime control. The main conclusion is that OpenClaw's biggest practical advantage is not a single feature: it treats onboarding, safety posture, and runtime policy as first-class product surfaces.

## Scope and assumptions

- The primary comparison baseline is Elegy plus SAASTools as they exist today.
- Elegy is intentionally bounded: artifact-first, governed, reusable, and not the owner of host auth, persistence, UI orchestration, runtime registration, or control-plane composition.
- SAASTools is the current runtime/control-plane proof point for desktop orchestration, retrieval and capability shaping, workspace secret refs, Copilot auth runtime state, session continuity, plugin and tool runtime surfaces, and inspector or routing context.
- OpenClaw and peer projects are pattern inputs, not templates to clone.
- If a capability depends on host auth, persistence, approval workflows, UI flows, HTTP endpoints, or composition-root orchestration, it should stay consumer-local.
- A shared Elegy seam needs evidence that at least two consumers benefit from the same governed contract, artifact, or bounded runtime helper.
- Supporting references such as copilot-sdk and GenericInfrastructure matter only where they reinforce the boundary or operating-model discussion.

## Provenance and evidence

- Local workspace and local checkout inputs used directly for this note: `Elegy/README.md`, `Elegy/docs/architecture/elegy-memory-v1.md`, `SAASTools/docs/research/oss/openclaw.md`, `SAASTools/docs/research/openclaw/03-runtime-topology-and-orchestration-model.md`, `SAASTools/docs/research/openclaw/07-security-boundaries-and-trust-policies.md`, `SAASTools/docs/research/openclaw/09-saas-tools-adoption-roadmap.md`, `SAASTools/docs/system/architecture/execution-isolation.md`, `SAASTools/docs/system/architecture/desktop-threat-model.md`, and a local OpenClaw checkout including `README.md`, `docs/start/wizard.md`, `docs/gateway/configuration.md`, `docs/gateway/security/index.md`, `docs/concepts/model-failover.md`, `docs/tools/skills.md`, `src/wizard/onboarding.ts`, `src/wizard/onboarding.gateway-config.ts`, `src/security/audit.ts`, `src/config/config.ts`, plus runtime routing files under `src/gateway` and `src/routing`.
- Web inputs used as upstream pattern evidence: `docs.openclaw.ai/start/wizard`, `docs.openclaw.ai/gateway/configuration`, `docs.openclaw.ai/gateway/security`, `docs.openclaw.ai/concepts/model-failover`, `docs.openclaw.ai/tools/skills`, plus GitHub repo pages for OpenHands, Open Interpreter, Open WebUI, and LibreChat as comparative references.
- The web review reinforced rather than materially changed the local conclusion: keep runtime control-plane ownership, provider execution, and operator enforcement local to the host, while borrowing stronger onboarding, policy visibility, and operational posture patterns.

## Short comparison baseline

- Elegy proves today: governed contracts, schemas, fixtures, manifests, policy assets, and bounded CLI or runtime tooling. It does not prove app-host UX, operator onboarding, runtime registration, secret brokerage, or live control-plane ownership.
- SAASTools proves today: desktop/runtime orchestration, workspace-scoped config, secret references, Copilot OAuth runtime state, retrieval and capability shaping, agentic session orchestration, plugin and tool runtime surfaces, and inspector-backed routing context.
- OpenClaw and peers prove today: operator-first onboarding, stricter self-configuration, explicit policy and audit surfaces, more visible runtime trust posture, per-agent execution policy, and stronger productization of setup-to-runtime continuity.
- OpenHands is the strongest reference for packaging an agent system across SDK, CLI, local GUI, and cloud, but it is not the strongest reference for secure local-first onboarding.
- Open Interpreter is the strongest reference for explicit approval-before-exec and local-computer power, but it is risky if copied without sandbox-first enforcement.
- Open WebUI and LibreChat are strong references for multi-provider UX, resumable streams, admin surfaces, artifacts, and context management, but both are heavier and more multi-user than Elegy should absorb.

## Theme-by-theme roadmap

### Onboarding and self-configuration

- What OpenClaw or peers do: OpenClaw treats onboarding as product surface area, with quickstart versus advanced paths, explicit risk acknowledgement, strict config validation, and runtime config reload behavior. OpenHands and Open WebUI also show the value of reducing the gap between install, configure, and first useful run.
- Gap in our repos: SAASTools has the runtime pieces, but not one coherent operator onboarding lane that sets auth mode, harness, secret posture, capability posture, and safety defaults in one guided flow. Elegy should not become that product shell.
- Recommended direction: Build a host-owned onboarding wizard and config bootstrap path that is fail-closed, explicit about risk tiers, and honest about degraded versus ready state. Separate portable authored intent from machine-local runtime state from the first run.
- Ownership guidance: Likely Elegy candidate = optional portable config-risk descriptors only if reused by multiple consumers. SAASTools/app-local = onboarding wizard, authored config editing, runtime validation, reload behavior, and risk acknowledgement UX. Not shared = install shell, product copy, and host-specific setup flows.

### Secret handling and provider auth UX

- What OpenClaw or peers do: OpenClaw uses auth profiles, secret references, env injection, and session-sticky provider failover. LibreChat and Open WebUI show strong multi-provider selection and credential management patterns.
- Gap in our repos: SAASTools already proves `secretRef` and separate Copilot runtime auth state, but it does not yet present a single end-to-end operator flow for safe provider setup, failover policy, and audited secret use. Elegy has the boundary posture, but not a reusable contract for secret-handle semantics.
- Recommended direction: Use a safe secret-drop flow where the UI stores a secret reference or brokered capability token, the host injects it only at execution time, and the model sees only an indirect handle. Make provider profile validation explicit and make failover sticky and auditable per session.
- Ownership guidance: Likely Elegy candidate = secret-reference descriptor or brokered-capability-token descriptor, with redaction and injection metadata, but only if at least two consumers need it. SAASTools/app-local = secret storage, auth exchange, provider selection, failover policy, and injection runtime. Not shared = raw secret handling, token brokers, and provider-specific setup UX.

### LLM integration, provider abstraction, and prompt ownership

- What OpenClaw or peers do: They treat provider and model choice as runtime policy, not just UI preference, with explicit profiles, validation, failover behavior, and visible differences between model routing, prompt assembly, and tool execution.
- Gap in our repos: SAASTools has the ingredients for provider auth state, `secretRef`, retrieval-driven capability shaping, and session continuity, but it does not yet expose one coherent operator model for provider abstraction, model selection, failover, prompt ownership, and enforcement hooks. Elegy should not become the live provider client or prompt-builder host.
- Recommended direction: Keep provider adapters, model routing, sticky failover, prompt assembly, and execution-time secret injection in the host runtime. The model should never see raw provider credentials; it should see only host-selected context, tools, and indirect handles. Enforcement hooks should live in host-owned config validation, provider-profile selection, pre-prompt assembly, pre-tool dispatch, and audit emission rather than in portable prompt text alone.
- Ownership guidance: Likely Elegy candidate = bounded provider-profile descriptors, model capability metadata, redaction and injection metadata, and policy or audit envelopes only if multiple consumers need the same governed shape. SAASTools/app-local = provider SDK wiring, model selection and failover, prompt assembly, secret resolution, retries, and turn-time enforcement. Not shared = raw keys, provider-specific client logic, or host-local prompt composers.

### Policy enforcement and auditability

- What OpenClaw or peers do: OpenClaw exposes explicit security audit checks, hardened baselines, clear check IDs, and policy-driven trust surfaces instead of relying on prompt wording. Open WebUI and LibreChat also reinforce the value of visible admin policy state.
- Gap in our repos: SAASTools has meaningful guardrails and diagnostics, but not one operator-facing audit surface that says which checks ran, what failed, what degraded, and what remediation is expected. Elegy does not yet offer a stable shared policy-decision or audit envelope.
- Recommended direction: Define policy bundles with explicit check IDs, severity, remediation references, and fail-closed validation. Enforce those policies in host/runtime gates, not just system prompts. Emit auditable findings and degraded-state reasons.
- Ownership guidance: Likely Elegy candidate = policy decision envelopes, audit finding envelopes, report schemas, and baseline metadata. SAASTools/app-local = enforcement engine, audit command, runtime gating, remediation UX, and release gates. Not shared = host-specific approval rules and deployment-specific exceptions.

### Orchestration and control-plane design

- What OpenClaw or peers do: OpenClaw makes startup phases, capability dispatch, config reload, and runtime control surfaces explicit. OpenHands shows how a product can span SDK, CLI, local UI, and hosted paths without hiding the control-plane boundary.
- Gap in our repos: SAASTools proves orchestration in practice, but the operator-facing control plane is still spread across shells, DesktopHost, APIs, config, and inspector surfaces. Elegy must not become an app host or a control-plane product shell.
- Recommended direction: Consolidate control-plane concepts in SAASTools around explicit startup phases, readiness contracts, capability registry metadata, and truthful degraded-state reporting. Keep the product shell and runtime composition local. Use Elegy only for bounded, governed metadata when a real cross-consumer seam exists.
- Ownership guidance: Likely Elegy candidate = capability metadata or control-plane report envelopes only. SAASTools/app-local = control-plane runtime, lifecycle orchestration, reload handling, and operator surfaces. Not shared = composition-root orchestration, DI wiring, HTTP endpoints, and app-host behavior.

### Execution practices and operational enforcement

- What OpenClaw or peers do: They make validation posture, startup phases, readiness, degraded operation, audit loops, and reload behavior explicit operator concerns rather than hidden implementation details.
- Gap in our repos: SAASTools has health checks, inspector surfaces, and policy-aware runtime pieces, but not one clear execution-practices contract that distinguishes startup validation from readiness, degraded from failed, periodic audit from one-time config lint, and hot-reloadable settings from restart-only boundaries. Elegy can define policy artifacts, but it should not become the ops loop.
- Recommended direction: Treat execution practices as host runtime policy. Fail closed on invalid config, policy, and capability registration. Tie readiness to real dispatch, session, and provider prerequisites instead of process-up signals. Emit stable degraded-state reason codes, run explicit audit loops, and document which changes can hot reload versus which require restart. Keep policy definition portable; keep operational enforcement local, observable, and testable.
- Ownership guidance: Likely Elegy candidate = validation result envelopes, audit finding schemas, and bounded metadata about reload eligibility only if reused. SAASTools/app-local = startup gates, readiness checks, degraded-state reporting, audit cadence, reload or restart behavior, and remediation UX. Not shared = process supervision, health probe implementation, deployment exceptions, or incident workflows.

### Execution safety, approvals, and sandbox posture

- What OpenClaw or peers do: OpenClaw emphasizes per-agent sandbox or tool profiles, pairing and allowlists, mention gating, and sub-agent guardrails. Open Interpreter is the clearest proof that approval-before-exec matters for local power, but it also demonstrates how dangerous that surface is without stronger sandboxing.
- Gap in our repos: SAASTools already narrows capability exposure and keeps tool execution host-owned, but it does not yet expose approval posture and sandbox profile selection as first-class operator concepts. Elegy can describe policy, but it should not be the enforcement runtime for host-specific execution.
- Recommended direction: Make approval checkpoints, per-agent tool families, sandbox profiles, and sub-agent inheritance rules explicit host policy. Prefer allowlists, approval boundaries, and fail-closed validation over prompt-only restraint.
- Ownership guidance: Likely Elegy candidate = policy profile descriptors and capability exposure metadata. SAASTools/app-local = approval workflows, process sandboxing, OS integration, runtime enforcement, and escalation handling. Not shared = machine trust decisions, platform-specific sandbox code, and interactive approval UX.

### Context building, retrieval, and capability shaping

- What OpenClaw or peers do: OpenClaw shows explicit skill precedence, gating, and env injection. LibreChat and Open WebUI show stronger operator control over model, tool, artifact, and context surfaces. OpenHands shows the value of productized context routing across multiple surfaces.
- Gap in our repos: SAASTools already proves retrieval-driven capability shaping, but the explainability contract for why a capability was exposed, denied, degraded, or excluded is still mostly host-local. Elegy-memory is intentionally bounded and should not absorb live retrieval authority.
- Recommended direction: Keep retrieval ranking, live memory selection, and turn-time capability shaping in SAASTools. Consider governed metadata only for exposure decisions, shortlist rationale, and redacted context-shaping summaries if two or more hosts need the same explainability contract.
- Ownership guidance: Likely Elegy candidate = context-shaping metadata or capability-exposure decision envelopes. SAASTools/app-local = retrieval pipelines, ranking, memory authority, prompt assembly, and live tool gating. Not shared = prompt budgets, store ownership, and runtime planner heuristics.

### Session continuity, routing, and resumability

- What OpenClaw or peers do: OpenClaw emphasizes canonical session keys, route precedence, and deterministic session handling. LibreChat proves resumable streams and artifact-aware continuity. OpenHands shows the value of continuity across local and hosted surfaces.
- Gap in our repos: SAASTools already proves reopen, continue, history, routing context, and orchestration summaries, but it does not yet expose one portable continuity contract that could be reused across multiple hosts. Elegy should not own live session stores or routing engines.
- Recommended direction: Harden a host-owned session envelope and route precedence contract, then expose continuity states consistently across UI, diagnostics, and persisted summaries. If a reusable continuity envelope emerges across consumers, formalize only the metadata shape.
- Ownership guidance: Likely Elegy candidate = continuity metadata envelope only if at least two consumers need it. SAASTools/app-local = session stores, history, route resolution, resumability, and persisted orchestration truth. Not shared = live transport, reconnection logic, and host persistence models.

### Operator diagnostics, health, and config lifecycle

- What OpenClaw or peers do: OpenClaw makes config health, degraded state, audit posture, and reload behavior visible. Open WebUI and LibreChat show the value of explicit operator/admin diagnostics, even when their multi-user scope is broader than ours.
- Gap in our repos: SAASTools has diagnostics, state inspection, and inspector surfaces, but not one unified operator view of authored config, effective runtime config, audit posture, degraded reasons, and reload outcomes. Elegy should not become the dashboard host.
- Recommended direction: Treat config lifecycle as product surface. Add host-owned config linting, effective-versus-authored diff views, degraded-state reason codes, reload events, and health snapshots that map cleanly to policy and capability decisions.
- Ownership guidance: Likely Elegy candidate = redacted diagnostics or audit report envelopes, plus config-diff metadata only if shared. SAASTools/app-local = runtime health, reload logic, diagnostics UI, state inspection, and support bundles. Not shared = dashboards, support tooling workflows, and environment-specific health probes.

## Good ideas worth copying

- Make operator onboarding a first-class surface instead of assuming config files and scattered docs are enough.
- Separate portable authored intent from machine-local runtime state and from secrets from day one.
- Use explicit capability allowlists, approval checkpoints, and sandbox profiles instead of prompt-only restrictions.
- Give every important policy check a stable check ID, severity, and remediation path.
- Treat degraded state as a first-class runtime outcome, not an embarrassing edge case to hide.
- Keep session routing and resumability semantics deterministic and visible.
- Expose why capabilities were surfaced, denied, or degraded so operators can reason about runtime posture.
- Prefer safe indirect secret handles over ever showing raw secrets to the model.

## Bad ideas, gotchas, and anti-patterns to avoid

- Turning Elegy into an app host, admin shell, or control-plane product.
- Treating system prompts as the real policy layer.
- Letting the model see raw secrets, OAuth tokens, or provider credentials.
- Copying Open Interpreter-style local execution power without a sandbox-first posture.
- Pulling heavy multi-user admin breadth from Open WebUI or LibreChat into Elegy just because those projects do it well.
- Building a mega-orchestrator in SAASTools that collapses the existing runtime boundaries.
- Assuming a validated plugin manifest is the same thing as a safe runtime capability.
- Allowing silent fallback that broadens capability exposure or hides degraded security posture.

## Candidate library or contract seams

### Strong Elegy candidates

- Policy decision envelopes: stable allow or deny or degraded decisions with check IDs, subjects, targets, rationale, and remediation references.
- Capability exposure descriptors: the governed shape for tool or capability class, required approval, sandbox profile, allowlist source, and redaction expectations.
- Secret-reference descriptors: opaque handles only, never raw secret values, with provider scope, injection intent, and redaction metadata.
- Audit finding and report envelopes: machine-readable findings, severity, evidence hooks, and summary rollups that hosts can produce and consume.
- Context-shaping metadata: redacted shortlist rationale, exclusion reasons, capability provenance tags, and budget class metadata, but only if multiple consumers need the same contract.

### Maybe later, only with proof

- Config-risk profile descriptors for onboarding and hardened baseline selection.
- Continuity metadata envelopes for reopen, continue, resume, and historical-state semantics across more than one host.
- A bounded validator or renderer for shared audit or policy artifacts if contract-only sharing proves insufficient.
- Capability registry projections that multiple hosts actually consume, not just one repo's local runtime.

### Keep consumer-local

- Onboarding wizards, approval flows, and operator/admin UX.
- Secret storage, auth exchange, token brokerage, provider failover, and runtime env injection.
- HTTP endpoints, composition-root orchestration, control-plane lifecycle code, and app-host startup behavior.
- Live retrieval ranking, prompt assembly, session stores, route resolution, and resumability implementation.
- Process sandboxing, OS integration, pairing flows, mention gating, and machine-trust policy.

## Suggested phased roadmap

### Near term

- In SAASTools, add a coherent onboarding and self-configuration flow that chooses harness, auth mode, secret posture, and safety defaults in one path.
- In SAASTools, add a safe secret-drop flow where the stored artifact is a `secretRef` or brokered handle and the model only sees an indirect reference.
- In SAASTools, formalize host-owned provider abstraction, model selection, prompt-assembly ownership, and session-sticky failover with enforcement hooks before prompt assembly and tool dispatch.
- In SAASTools, introduce explicit policy check IDs, a minimal audit report surface, and truthful degraded-state reporting.
- In SAASTools, add fail-closed startup validation, readiness gates, degraded reason codes, and explicit hot-reload versus restart-only boundaries.
- In SAASTools, make approval posture, sandbox profile, and capability allowlist state visible in diagnostics and inspector surfaces.
- In Elegy, draft only the smallest candidate schemas: policy decision envelope, audit finding/report envelope, and secret-reference descriptor, and stop unless a second consumer appears.

### Medium term

- In SAASTools, add auth profiles with session-sticky failover and explicit runtime validation or hot reload semantics.
- In SAASTools, formalize a capability registry with operator-visible metadata for approval, sandboxing, audit, and route ownership.
- In SAASTools, harden one host-owned session envelope and route precedence model across desktop, workflow, and future remote surfaces.
- In Elegy, stabilize only the shared artifacts that now have real multi-consumer proof, plus fixtures and validators.

### Later

- In SAASTools or other consumer apps, expand operator/admin surfaces for health, config lifecycle, audit posture, and recovery workflows.
- In SAASTools or other consumer apps, add richer pairing, mention gating, trust delegation, and policy-pack management if the product direction justifies it.
- In Elegy, add bounded runtime helpers only if contracts alone are not enough and at least two consumers need the same implementation behavior.
- Do not turn Elegy into the runtime control plane at any phase.

## Open questions

- Which proposed Elegy seam has a believable second consumer besides SAASTools?
- Do we need one shared continuity envelope across desktop, workflow, and future remote execution paths, or are host-local models still sufficient?
- What is the smallest audit surface that materially improves operator behavior rather than generating more diagnostics noise?
- Which approval classes should always block on user confirmation, and which can be policy-preapproved?
- How far should session-sticky provider failover go before it becomes misleading or operationally dangerous?
- Which parts of config lifecycle truly need hot reload, and which should remain restart-only for safety?
- How much of the multi-provider and multi-user admin pattern from LibreChat or Open WebUI is actually relevant to our current local-runtime direction?
- Can we keep context-shaping metadata useful without leaking too much about internal scoring or sensitive runtime state?