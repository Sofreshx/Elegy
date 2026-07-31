---
title: Elegy Terminology
status: active
owner: elegy-core
doc_kind: reference
---

# Elegy Terminology

## Purpose

This glossary defines the canonical terms used across the Elegy umbrella repo.

These definitions exist to prevent later phases from overloading the same words in incompatible ways.

## Glossary

### Plugin

A reusable adapter that lets an agent operate against a concrete data source,
database, platform, API, local-system, or executable/CLI boundary.

A plugin is not a general name for packaged business logic. Domain analysis,
scoring, reports, and product workflows remain libraries, applications,
Automation Packs, or product-local commands. A manifest proves package shape,
not plugin eligibility or usability.

### Readiness

The evidence-backed stage of one distributed surface: `concept`, `implemented`,
`usable`, or `production`.

Only `usable` and `production` are agent-routable. Source tests, fixture
conformance, compilation, schemas, archives, and generated projections can
support `implemented` or `conformance`; they never establish a live proof or
usability by themselves.

### Implemented

Evidence that source behavior and its local tests exist. `implemented` is not a
claim that a package can be installed, connected to a real system, or routed to
an agent.

### Conformance

Evidence that a governed schema, fixture, or contract consumer interprets a
surface correctly. Conformance is stronger than a shape check but weaker than
an installed task against a real system.

### Live proof

Evidence from a clean packaged installation completing a non-fixture task in a
declared environment. A live proof is required before a surface can qualify as
`usable`; it does not imply production durability or broad host support.

### Routable

A host may offer a surface for agent selection only when its readiness is
`usable` or `production` and the declared interface has passed the required
validation. Routable does not bypass host policy, approvals, credentials, or
side-effect gates.

### Substrate

The lowest reusable authority layer in the repo.

It includes governed schemas, fixtures, manifests, policy artifacts, support metadata, and the rules that shape their exports and validation. The substrate is where shared publishable artifacts are defined without provider, framework, or host ownership.

### Contract

A stable public agreement that another package, repo, or tool can depend on.

Contracts are represented here as governed schemas, fixtures, manifests, support metadata, or other authored artifacts. A contract is stronger than a local implementation detail.

### Monorepo

The single main Elegy repository that contains both the neutral governed artifact roots and the first-party Rust runtime family.

Monorepo does not mean that every language surface has the same authority. It means they are versioned and governed together.

### Schema

A machine-readable structural definition for a serialized artifact.

Schemas describe shape. They do not automatically define runtime ownership, orchestration behavior, or host lifecycle semantics.
They also do not establish readiness or justify a consumer dependency.

### Fixture

A concrete artifact used to validate a schema, contract, or compatibility rule.

Fixtures are governed evidence, not informal examples.

### Conformance artifact

A published artifact used to prove that a consumer or sibling repo is interpreting a shared contract correctly.

Examples include compatibility manifests, compatibility matrices, and governed fixtures.

### Capability

A governed operation or addressable-resource concept described by the capability
catalog and optionally guided by a skill.

A capability is what a host invokes — it includes an identity, input/output
contracts, execution metadata, and governance posture. In v2 each entry names
one concrete interface (`cli`, `mcp-resource`, or `mcp-tool`). The capability is
the governed concept; a host projection is a derived installation or transport
surface.

### Skill

Reusable agent instructions plus optional references, scripts, templates, and
assets for a focused workflow.

A skill may explain how to use an adapter or tools already available to a host.
It is not executable product evidence, a typed capability contract, or a
connector by itself. Native hosts may distribute skills in installable plugin
bundles; Elegy still classifies the subject as a `skill` unless a real adapter
boundary is present.

### Portable plugin core

The host-neutral plugin package and capability catalog from which host-specific
install surfaces are derived.

The portable plugin core owns capability identity, invocation contracts,
dependencies, and distribution metadata. It does not contain a Codex plugin
directory, a Holon package manifest, a native workflow graph, client
credentials, or deployment state.

### Host projection

A derived installation or discovery surface for one supported agent host.

For example, the Codex `.codex-plugin` tree is a host projection of a portable
Elegy plugin core. A host projection may adapt layout and host metadata, but it
must not become a second authority for capability behavior.

### Capability binding

A reference from a higher-level system to a governed Elegy capability it may
invoke.

An Automation Pack can use capability bindings for assisted or agent-runner
steps without becoming an Elegy plugin. The binding identifies a capability
and its required contract; the executing host still owns selection, policy,
approval, credentials, and runtime evidence.

### Automation Pack

An independently versioned and signed distribution for a reusable business
automation outcome.

An Automation Pack can contain native target workflows, data policy,
configuration slots, target projections, Care metadata, and optional Elegy
capability bindings. It is not an Elegy plugin and it does not make a native
target workflow part of the Elegy substrate. The incubating Pack and delivery
contracts are owned by Automation Forge until promotion criteria are met.

### Target adapter

A target-specific implementation that materializes, inspects, or controls an
Automation Pack projection in an automation runtime such as n8n.

The adapter consumes Pack and delivery contracts. It does not own the portable
Pack, client approval, credentials, or deployment state, and successful support
for one target does not imply support for another.

### Agent-runner binding

A Pack-declared binding from an assisted automation step to a governed Elegy
capability or host tool projection.

The binding describes the required capability boundary. The selected agent
host remains responsible for compatible projection, invocation policy,
approval, and evidence. An agent-runner binding does not grant an Automation
Pack ambient agent or tool access.

### Automation deployment

A client-specific installed instance of a verified Automation Pack projection
on a selected target.

Deployment state binds Pack and target identity with local configuration,
opaque credential references, approvals, receipts, and operational status.
Those client-local facts belong to the operating host or control plane, not to
the portable Elegy plugin core or the Pack distribution.

### Capability kind

A single concrete interface discriminator in the
`elegy-capability-catalog/v2` catalog. Current kinds are `cli` (local
executable invocation), `mcp-resource` (addressable MCP resource), and
`mcp-tool` (typed MCP tool call). Each capability entry has exactly one kind;
publishing the same behavior through more than one interface requires separate
entries and evidence. The v1 `mcp` and `app-binding` values are compatibility
metadata only. `provider-adapter` is deferred until a real AI-provider
consumer exists.

### App binding

A legacy capability-catalog value describing an external-service capability.
Its `connector` is a portable service name, not a host app ID. It is meaningful
only when a native Codex app connection binding exists in the host projection;
otherwise it is not an active or routable interface. Connection requirements
and opaque host app IDs remain the authentication authority.

### Connection requirement

A portable declaration that a plugin needs a verified account/service
connection. It has a plugin-local identity, service identity, required state,
and human description. It contains no credentials and does not imply that
installing the plugin connects the account.

### Connection binding

A host-projection mapping from a portable connection requirement to a
host-registered integration. For Codex, the target is an opaque app ID in
`.app.json`; it is never inferred from a service slug.

### Connection provider

A credential-owning component that implements
`elegy-connection-control/v1`. It owns human authentication UX, secure storage,
verification, refresh, and revocation. Hosts consume sanitized connection state
and opaque references through a signed, credential-free control boundary.

### Fallback

Legacy descriptive metadata for a possible alternative surface. Elegy has no
active runtime consumer that selects or executes fallback entries. Fallback
does not create a second capability kind, alter routing, or establish
readiness; new runtime code must not branch on it.

### Dynamic skill

A skill representation or activation path that is derived or materialized at runtime rather than declared only as a static artifact.

Dynamic does not mean ungoverned. The inputs and outputs still need formal contracts.

### Tool

A callable operation boundary exposed to an agent, model, or runtime.

A tool is a callable interface through which a capability is invoked. In the
catalog, `cli`, `mcp-resource`, and `mcp-tool` are first-class concrete
interfaces; a projection or wrapper must not silently change the declared kind.

### Tool projection

A derived callable view of a capability for a specific host surface. CLI,
MCP-resource, and MCP-tool entries are first-class catalog interfaces; Codex,
skills, and other host layouts are projections of those entries.

Each tool projection declares its projection kind, input/output schemas, invocation envelope, side-effect classification, dependency requirements, and provenance from the source capability. The projection is derived — it is not the canonical authority for the underlying contract.

### Function calling

One model-facing projection of a tool with strict input arguments.

Function calling describes how a model invokes a capability through a typed interface. It is a projection target, not the execution authority. Policy, retries, approvals, tool allowlists, and execution decisions remain host responsibilities, not contract-layer concerns.

### Structured output

Output required when downstream code, workflow state, approval, or another agent depends on the result.

Structured output is governed by a JSON Schema reference. Every machine-invokable capability must declare an `output.schemaRef` so hosts can validate, chain, approve, or freeze results without relying on unstructured stdout.

### Frozen tool

A promoted deterministic capability with schema, provenance, inputs, validation evidence, policy, and fallback behavior.

A frozen tool has been validated with known inputs and expected outputs. It carries evidence of past correct behavior and declared fallback instructions. Freezing is a host-level promotion — it does not change the underlying capability contract.


### Descriptor

A structured description of a runtime-facing thing such as a tool, server surface, or generated artifact.

Descriptors should remain descriptive. They should not silently absorb transport execution behavior.

### Manifest

A governed document that describes package, schema, fixture, or compatibility state.

Manifests are used for coordination and validation, not as a substitute for the source contracts themselves.

### Projection

A derived representation of an underlying model.

Examples include Mermaid output, MCP tool lists, CLI command surfaces, and OpenAI function-calling descriptors. A projection is not the authority for the original model. See also Tool projection for the specific kind of projection that exposes a callable tool surface.

### Slice

A bounded subset of a larger descriptor, analysis result, or capability set that is used to derive a narrower artifact.

A slice is useful for generation and runtime selection, but it is not automatically a new authority surface. If a slice needs a stable serialized contract, that contract should live with governed artifacts rather than inside a runtime helper.

### Adapter

A framework-specific, host-specific, or environment-specific integration layer that consumes public Elegy contracts.

Adapters belong above the substrate. They should not define the core contract model.

### Host

The application or runtime environment that executes or composes capabilities.

Hosts are consumers of Elegy abstractions. They are not the place where substrate contracts should be invented.

### Runtime

The concrete execution context for behavior, composition, or transport.

Runtime concerns include lifecycle, invocation, transport, environment binding, and operational behavior. Runtime ownership is distinct from formal contract ownership.

### Runtime family

The set of implementation packages or crates that own behavior-heavy execution concerns such as transport, filesystem, HTTP, host integration, and CLI orchestration.

In the current topology, the crate trees under `hosts/`, `plugins/`, and `shared/` are the primary runtime family for MCP-oriented execution concerns.

### Authority surface

The package family or artifact set that is allowed to define canonical truth for a concept.

In Elegy, authority surfaces are the governed artifact roots co-located in each plugin's directory (e.g., `plugins/<name>/schemas/`, `plugins/<name>/fixtures/`). Rust implements operational behavior that consumes those artifacts but does not replace their canonical truth. Operational policy lives at `docs/governance/`.

### Governance

The metadata, rules, and enforcement posture that define how artifacts are versioned, validated, allowed, or constrained.

Governance is broader than security policy. It includes compatibility, conformance, and contract-change discipline.

### Forge

The generation and materialization layer responsible for deterministic derived outputs.

Forge is not the same thing as the human-facing CLI. It is the subsystem that emits or materializes governed artifacts or derived projections from them.

### Automation Forge

The separately owned delivery toolchain that authors, validates, packages,
signs, materializes, and controls Automation Pack projections through public
delivery and adapter contracts.

Automation Forge is not the generic Elegy forge subsystem, an automation
runtime, or a client control plane. Its Pack contracts remain outside Elegy
while they incubate.

### CLI

The human-facing `elegy` command surface.

The CLI is a consumer-facing wrapper around public package capabilities. It is not the umbrella name for every generation or command-related concern.

### InvocationResponse

The canonical machine result envelope for any capability invocation.

An `InvocationResponse` wraps the result of invoking a capability: a status (`completed`, `failed`, `cancelled`), the structured output payload, and an optional `StructuredFailure`. It is the standard response shape for CLI, MCP, and other tool projections — hosts can rely on it to route, validate, and log results.

### StructuredFailure

The canonical machine-readable failure contract across all governed execution, CLI, and adapter surfaces.

A `StructuredFailure` includes an error code, message, category (e.g. `invalidInput`, `policy`, `timeout`), retryability flag, and optional correlation ID, details, and cause chain. Capability failures must use `StructuredFailure` rather than unstructured stdout or prose, so hosts can act on failures programmatically.
