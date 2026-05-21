# MCP, Skill, and Tooling Placement

## Purpose

This document applies the burden-of-proof rule to the features that are easiest to misplace during the current cleanup:

- MCP analysis
- dynamic MCP creation
- skill creation from an MCP slice
- portable plugin-package metadata
- dynamic CLI tools when no better integration surface exists

The goal is to decide where neutral artifact authority lives, where Rust executable behavior lives, and when a capability should remain in a consuming repo instead of being centralized in Elegy.

The contributor-navigation overlays under `src/Elegy-mcp` and `src/Elegy-skills` are pointer shells only. They are not repo centers, authority layers, implementation centers, or release surfaces.

For contributor-facing CLI use in these lanes, prefer the dedicated `elegy-mcp` and `elegy-skills` binaries for their bounded paths. Keep `elegy` as the general/compatibility surface.

## Placement rule

Use the following order when deciding where a feature belongs:

1. If the feature is mainly a schema, fixture, manifest, policy artifact, or compatibility rule, keep it in the governed artifact roots.
2. If the feature defines canonical semantics that multiple runtimes or consumers must consume exactly, keep that truth in neutral governed artifacts and docs, not in a language-specific runtime surface.
3. If the feature is a self-contained executable capability that multiple consumers should use, prefer the Rust workspace.
4. If the feature depends on host auth, persistence, product policy, UI orchestration, HTTP endpoints, or composition-root behavior, keep it in the consumer.

## Feature placement

| Feature | Authority surface | Executable lane | Consumer lane | Decision |
|---|---|---|---|---|
| MCP analysis | Governed descriptor and analysis-result artifacts under `contracts/`, plus documented projection semantics | Rust crates such as `elegy-mcp`, `elegy-runtime`, and the Rust CLI | Host-specific UX or transport wrappers stay local | Analysis execution is Rust-first; neutral artifacts keep the stable shape. |
| Dynamic MCP creation | Descriptor fragments, manifests, or other stable serialized shapes under governed artifacts when they need to cross runtime boundaries | Rust tooling or CLI when creation is reusable and self-contained | Product-local server wiring, transport, or auth stays local | Dynamic creation should not become a broad shared runtime surface in Elegy. |
| Skill creation from an MCP slice | Governed skill artifacts such as `skill-definition` and related discovery outputs | Rust generation from analyzed MCP slices, typically through `elegy-tooling`, `elegy-skills`, and the general `elegy` compatibility surface | App-local post-processing or host-specific registration stays local | The slice-to-skill executable path is Rust-first; only the stable artifacts stay authoritative. |
| Portable plugin package | `elegy-plugin-package/v1` schema and fixtures under `contracts/` | Validation and derived projection export support in Rust | Install state, policy, approvals, secrets, runtime execution, and evidence stay local to the host | The package is a governed bundle contract, not an Elegy plugin runtime. |
| Dynamic CLI tools | Optional manifest/descriptor contract only if cross-runtime interoperability requires one | Rust CLI or future Rust tooling crate | App-local invocation policies stay local | Treat as a Rust tooling problem, not a neutral authority artifact. |

## MCP analysis

The longer-range MCP target narrative remains REST/OpenAPI definition ingestion, governed operation-catalog projection, and dynamic MCP generation from API specs through governed artifacts plus Rust tooling.

### What stays authoritative

- governed MCP descriptor and analysis-result schemas, fixtures, and manifests under `contracts/`
- compatibility expectations and version policy under `governance/`
- canonical skill projection semantics where MCP analysis feeds governed skill outputs

These belong with governed artifacts and canonical contract semantics.

### What should execute in Rust

- analyzer logic
- generator logic
- search and resolve logic
- runtime loading of MCP descriptor resources
- CLI or host flows that expose MCP analysis to operators

The current Rust stack already reflects this direction through `elegy-mcp`, `elegy-tooling`, `elegy-runtime`, and `elegy-cli`.

`elegy-mcp` is now a shipped thin dedicated CLI surface for descriptor authoring and descriptor analysis, and it is the preferred bounded CLI path for that work. That does not imply that REST/OpenAPI ingestion or hosted runtime execution is already implemented.

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

Do not introduce a broad shared runtime package for dynamic MCP creation just because a downstream consumer uses another language.

## Skill creation from an MCP slice

An MCP slice is a bounded subset of a descriptor or analysis result selected for skill generation.

Recommended split:

- keep the authoritative skill contract in governed skill artifacts under `contracts/`
- keep any stable serialized slice contract in governed contracts only if a real cross-runtime need appears
- implement slice selection and skill generation in Rust when the capability is meant to be shared and executable

This keeps neutral artifacts as the source of truth for what a valid skill is, while Rust owns the reusable execution path that derives those skills from MCP inputs.

`elegy-skills` is now a shipped thin dedicated CLI surface for governed skill-registry search, resolve, inspect, and validation. Lower-level MCP-to-skill generation remains on the shared tooling path rather than as the main `elegy-skills` product story. That does not imply autonomous authoring or runtime-side registration.

## Portable plugin package

`elegy-plugin-package/v1` is the governed cross-host package envelope for
combining skill definitions, optional instruction skill files, MCP projection
metadata, docs, and assets. It exists so hosts can ingest one package surface
without making `SKILL.md`, wrapper folders, or MCP projection files into
authority roots.

The package contract remains portable. It must not include host workspace ids,
approval decisions, secret refs, runtime sessions, adapter handles, or local
trust state. A consuming host owns those concerns after import.

Elegy V1 support currently includes contract validation plus conservative
derived projection export such as `elegy generate codex-plugin`. Do not add a
broad Elegy plugin runtime for this lane without a separate placement decision.

Current Codex projection support is intentionally narrow: generated
`.codex-plugin/plugin.json` and `skills/` remain derived outputs, while
`.app.json`, `.mcp.json`, connector auth/state, hooks policy, and install UX
remain outside the current portable-package projection slice unless the
governed package contract grows the required neutral metadata.

## Dynamic CLI tools

Dynamic CLI tools are not the default integration path.

Preferred order of integration is:

1. direct library or package consumption
2. governed file or descriptor consumption
3. MCP or other explicit protocol surface
4. static CLI integration
5. dynamic CLI tooling only when no better stable alternative exists

When dynamic CLI tooling is justified, it should be implemented as Rust-first tooling and kept behind explicit safety rules, not absorbed into a neutral authority layer.

See the companion research note in `docs/research/dynamic-cli-tooling.md` for the safety and adoption criteria.

## Practical guidance

If a new feature request touches MCP analysis, MCP creation, MCP-to-skill generation, or dynamic tools, ask these questions in order:

1. Does this need a governed artifact or just local behavior?
2. If it needs shared code, is it authority code or executable runtime code?
3. If it is executable runtime code, can it be self-contained and reusable enough to justify Rust ownership?
4. If it depends on host-specific lifecycle or product policy, why is it not consumer-local?

The default answer for new shared executable capabilities in this area should now be Rust, while neutral artifact authority stays rooted in `contracts/`, `governance/`, `schemas/`, and `policies/`.
