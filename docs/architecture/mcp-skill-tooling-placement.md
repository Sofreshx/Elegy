# MCP, Skill, and Tooling Placement

## Purpose

This document applies the burden-of-proof reset to the features that are easiest to misplace during the current cleanup:

- MCP analysis
- dynamic MCP creation
- skill creation from an MCP slice
- dynamic CLI tools when no better integration surface exists

The goal is to decide where the authoritative contract lives, where executable behavior lives, and when a capability should remain in a consuming repo instead of being centralized in Elegy.

## Placement rule

Use the following order when deciding where a feature belongs:

1. If the feature is mainly a schema, fixture, policy artifact, or compatibility rule, keep it as a governed artifact.
2. If the feature defines canonical semantics that multiple runtimes must consume exactly, keep it in the smallest durable `.NET` authority surface.
3. If the feature is a self-contained executable capability that multiple consumers should use, prefer Rust.
4. If the feature depends on host auth, persistence, product policy, UI orchestration, HTTP endpoints, or composition-root behavior, keep it in the consumer.

## Feature placement

| Feature | Authority surface | Executable lane | Consumer lane | Decision |
|---|---|---|---|---|
| MCP analysis | Governed descriptor and analysis-result contracts under `Contracts`, plus canonical projection semantics | Rust `elegy-mcp`, `elegy-runtime`, and the Rust CLI | Host-specific UX or transport wrappers stay local | Analysis execution is Rust-first; `.NET` keeps contract truth only. |
| Dynamic MCP creation | Descriptor fragments, manifests, or other stable serialized shapes under governed contracts when they need to cross runtime boundaries | Rust tooling or CLI when creation is reusable and self-contained | Product-local server wiring, transport, or auth stays local | Dynamic creation should not become a broad `.NET` runtime package in Elegy. |
| Skill creation from an MCP slice | Canonical `SkillDefinition` and related governed contracts remain authoritative in `.NET` | Rust generation from analyzed MCP slices | App-local post-processing or host-specific registration stays local | The slice-to-skill executable path is Rust-first; only the stable contracts stay authoritative in `.NET`. |
| Dynamic CLI tools | Optional manifest/descriptor contract only if cross-runtime interoperability requires one | Rust CLI or future Rust tooling crate | App-local invocation policies stay local | Treat as a Rust tooling problem, not a `.NET` authority package. |

## MCP analysis

### What stays authoritative

- `McpServerDescriptor`
- `McpAnalysisResult`
- related schemas, fixtures, and compatibility expectations
- canonical skill projection semantics where they must align with `SkillDefinition`

These belong with governed artifacts and canonical contract semantics.

### What should execute in Rust

- analyzer logic
- generator logic
- search and resolve logic
- runtime loading of MCP descriptor resources
- CLI or host flows that expose MCP analysis to operators

The current Rust stack already reflects this direction through `elegy-mcp`, `elegy-runtime`, and `elegy-cli`.

### What should stay local to consumers

- product-specific endpoint wrappers
- UI-driven exploration flows
- tenant-specific access rules
- transport-specific app integration details

## Dynamic MCP creation

Dynamic MCP creation is valid only when the result is still a governed descriptor shape rather than an ad-hoc runtime object.

Recommended split:

- If the output needs to be serialized, versioned, or shared, define the output contract under governed artifacts.
- If the creation path is a reusable operator capability, implement it in Rust tooling or CLI.
- If the creation path depends on app-local runtime context or product transport details, keep it in the consumer.

Do not introduce a broad shared `.NET` runtime package for dynamic MCP creation simply because some consumers are `.NET`.

## Skill creation from an MCP slice

An MCP slice is a bounded subset of a descriptor or analysis result selected for skill generation.

Recommended split:

- keep the authoritative skill contract in `Elegy.Formalization.Skills`
- keep any stable serialized slice contract in governed contracts only if a real cross-runtime need appears
- implement slice selection and skill generation in Rust when the capability is meant to be shared and executable

This keeps `.NET` as the source of truth for what a valid skill is, while Rust owns the reusable execution path that derives those skills from MCP inputs.

## Dynamic CLI tools

Dynamic CLI tools are not the default integration path.

Preferred order of integration is:

1. direct library or package consumption
2. governed file or descriptor consumption
3. MCP or other explicit protocol surface
4. static CLI integration
5. dynamic CLI tooling only when no better stable alternative exists

When dynamic CLI tooling is justified, it should be implemented as Rust-first tooling and kept behind explicit safety rules, not absorbed into shared `.NET` runtime packages.

See the companion research note in `docs/research/dynamic-cli-tooling.md` for the safety and adoption criteria.

## Practical guidance

If a new feature request touches MCP analysis, MCP creation, MCP-to-skill generation, or dynamic tools, ask these questions in order:

1. Does this need a governed artifact or just local behavior?
2. If it needs shared code, is it authority code or executable runtime code?
3. If it is executable runtime code, can it be self-contained and reusable enough to justify Rust?
4. If it depends on host-specific lifecycle or product policy, why is it not consumer-local?

The default answer for new shared executable capabilities in this area should now be Rust.