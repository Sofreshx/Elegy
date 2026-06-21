# Elegy Terminology

## Purpose

This glossary defines the terms that Phase 1 treats as canonical across the Elegy umbrella repo.

These definitions exist to prevent later phases from overloading the same words in incompatible ways.

## Glossary

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

### Fixture

A concrete artifact used to validate a schema, contract, or compatibility rule.

Fixtures are governed evidence, not informal examples.

### Conformance artifact

A published artifact used to prove that a consumer or sibling repo is interpreting a shared contract correctly.

Examples include compatibility manifests, compatibility matrices, and governed fixtures.

### Capability

A governed operation concept described by a skill or other source.

A capability is what a host invokes — it includes an identity, input/output contracts, execution metadata, and governance posture. Capabilities may be projected as tools (CLI, MCP, function calling, HTTP). Capability is the operational concept; a tool is one surface through which it is invoked. It should not be used as a synonym for a single runtime representation.

### Skill

A formalized capability definition with identity, metadata, and execution-oriented semantics.

In Elegy, a skill is a governed capability contract. It is a portable package/contract bundle — not inherently tied to a specific LLM vendor, prompt engine, runtime host, or MCP transport. A skill declares capabilities and their constraints; it does not own host-side execution decisions.

### Dynamic skill

A skill representation or activation path that is derived or materialized at runtime rather than declared only as a static artifact.

Dynamic does not mean ungoverned. The inputs and outputs still need formal contracts.

### Tool

A callable operation boundary exposed to an agent, model, or runtime.

A tool is not the original capability — it is a surface projection through which a capability is invoked. The same governed capability may be projected as multiple tools across different host surfaces (CLI, MCP, OpenAI function calling, HTTP).

### Tool projection

A callable view of a capability for a specific host surface: CLI, MCP, OpenAI function calling, HTTP, or another runtime.

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

In the current topology, the Rust subtree under `rust/` is the primary runtime family for MCP-oriented execution concerns.

### Authority surface

The package family or artifact set that is allowed to define canonical truth for a concept.

In Elegy, authority surfaces are the governed artifact roots under `contracts/` and `policies/`. Rust implements operational behavior that consumes those artifacts but does not replace their canonical truth.

### Governance

The metadata, rules, and enforcement posture that define how artifacts are versioned, validated, allowed, or constrained.

Governance is broader than security policy. It includes compatibility, conformance, and contract-change discipline.

### Forge

The generation and materialization layer responsible for deterministic derived outputs.

Forge is not the same thing as the human-facing CLI. It is the subsystem that emits or materializes governed artifacts or derived projections from them.

### CLI

The human-facing `elegy` command surface.

The CLI is a consumer-facing wrapper around public package capabilities. It is not the umbrella name for every generation or command-related concern.

### InvocationResponse

The canonical machine result envelope for any capability invocation.

An `InvocationResponse` wraps the result of invoking a capability: a status (`completed`, `failed`, `cancelled`), the structured output payload, and an optional `StructuredFailure`. It is the standard response shape for CLI, MCP, and other tool projections — hosts can rely on it to route, validate, and log results.

### StructuredFailure

The canonical machine-readable failure contract across all governed execution, CLI, and adapter surfaces.

A `StructuredFailure` includes an error code, message, category (e.g. `invalidInput`, `policy`, `timeout`), retryability flag, and optional correlation ID, details, and cause chain. Capability failures must use `StructuredFailure` rather than unstructured stdout or prose, so hosts can act on failures programmatically.