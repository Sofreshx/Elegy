# Elegy Terminology

## Purpose

This glossary defines the terms that Phase 1 treats as canonical across the Elegy umbrella repo.

These definitions exist to prevent later phases from overloading the same words in incompatible ways.

## Glossary

### Substrate

The lowest reusable package layer in the umbrella repo.

It includes core, contracts, serialization, validation, governance, and projection support. The substrate is where shared primitives and publishable artifacts are defined without provider, framework, or host ownership.

### Contract

A stable public agreement that another package, repo, or tool can depend on.

Contracts can be represented as public .NET types, JSON schemas, compatibility manifests, or other governed artifacts. A contract is stronger than a local implementation detail.

### Monorepo

The single main Elegy repository that contains both the authoritative .NET formalization families and the first-party Rust runtime family.

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

A reusable unit of behavior or affordance that may later be expressed through a skill, tool descriptor, or generated output.

Capability is the broad concept. It should not be used as a synonym for a single runtime representation.

### Skill

A formalized capability definition with identity, metadata, and execution-oriented semantics.

In Elegy, a skill is not inherently tied to a specific LLM vendor, prompt engine, runtime host, or MCP transport.

### Dynamic skill

A skill representation or activation path that is derived or materialized at runtime rather than declared only as a static artifact.

Dynamic does not mean ungoverned. The inputs and outputs still need formal contracts.

### Descriptor

A structured description of a runtime-facing thing such as a tool, server surface, or generated artifact.

Descriptors should remain descriptive. They should not silently absorb transport execution behavior.

### Manifest

A governed document that describes package, schema, fixture, or compatibility state.

Manifests are used for coordination and validation, not as a substitute for the source contracts themselves.

### Projection

A derived representation of an underlying model.

Examples include Mermaid output and MCP-derived or generation-derived representations. A projection is not the authority for the original model.

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

In Elegy, governed schemas, fixtures, compatibility manifests, and canonical skill contracts remain authority surfaces in the .NET package families even when Rust implements the operational behavior that consumes them.

### Governance

The metadata, rules, and enforcement posture that define how artifacts are versioned, validated, allowed, or constrained.

Governance is broader than security policy. It includes compatibility, conformance, and contract-change discipline.

### Forge

The generation and materialization layer responsible for deterministic derived outputs.

Forge is not the same thing as the human-facing CLI. It is the subsystem that emits or materializes governed artifacts.

### CLI

The human-facing `elegy` command surface.

The CLI is a consumer-facing wrapper around public package capabilities. It is not the umbrella name for every generation or command-related concern.