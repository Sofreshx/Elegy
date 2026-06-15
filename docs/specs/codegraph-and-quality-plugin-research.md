---
title: "Codegraph and Quality Plugin — v0 Implementation Spec"
status: active
owner: Elegy
created: 2026-06-13
updated: 2026-06-15
doc_kind: spec
summary: Concrete v0 spec for elegy-codegraph: portable CLI (extract + query) with TypeScript (Compiler API) and Rust (tree-sitter + cargo metadata + SCIP from rust-analyzer) extractors feeding a normalized graph IR stored in redb, with governed contract artifacts.
---

# Codegraph and Quality Plugin — v0 Implementation Spec

## Problem

Agents need better structural evidence about a codebase than repeated full-file reading or expensive end-to-end tests can provide. The proposed `elegy-codegraph` idea could help agents understand modules, symbols, dependencies, tests, docs, patterns, and change impact, but it is not a solved product yet. Code extraction across TypeScript and Rust is especially risky because syntax parsing and semantic understanding require different tools and assumptions.

## Goals

1. Ship a v0 prototype of `elegy-codegraph` as a portable Rust CLI with `extract` and `query` subcommands.
2. Implement a TypeScript extractor that uses the TypeScript Compiler API for files, modules, symbols, calls, exports, test detection, and doc links.
3. Implement a Rust extractor that uses tree-sitter-rust for syntax, cargo metadata for the crate graph, and SCIP from rust-analyzer for calls/references edges.
4. Store the normalized graph IR in redb with entities and edges keyed for fast symbol lookup and neighbor traversal.
5. Publish a governed JSON Schema contract at `contracts/schemas/elegy-codegraph.graph.v0.json` with an example fixture.

## Design Decisions

### v0 Command Scope: extract + query only

The v0 prototype implements `extract` (build the graph index from source) and `query` (symbol, neighbors, impact, summary). The `diff`, `review`, and `validate` commands are deferred to a follow-up spec at [`docs/specs/codegraph-diff-slice.md`](./codegraph-diff-slice.md). This keeps the v0 surface small enough to validate extractor quality, query usefulness, and agent integration before committing to a broader command surface.

### Storage: redb (not sled)

The graph index is stored in **redb 4.1.0**, a pure-Rust embedded KV store with ACID/MVCC transactions and a stable on-disk format. Alternative considered: **sled 0.34.7**.

| Property | redb 4.1.0 | sled 0.34.7 |
|---|---|---|
| Last release | Apr 2026 | Sep 2021 |
| Status | Stable, maintained | "Champagne of beta"; on-disk format will break before 1.0 |
| Transactions | ACID, MVCC, savepoints | Serializable ACID, optimistic |
| On-disk format | Stable with committed upgrade path | Warned to change, manual migration required |
| Runtime deps | `libc` only | crossbeam, parking_lot, fs2, fxhash |
| Cold-open | mmap, single file, near-zero `len()` | Pagecache on log |

**Why redb over sled:** sled's own README states "if reliability is your primary constraint, use SQLite. sled is beta." For a durable code index that agents depend on, a stable on-disk format is non-negotiable. redb's COW B+tree with mmap gives excellent cold-start performance for the per-invocation CLI pattern. The `MultimapTable` type maps naturally to one-to-many graph edges (outgoing/incoming neighbor tables).

**Why not SQLite:** `elegy-memory` already uses SQLite, but graph neighbor traversal (range-scanning adjacency lists) is a poor fit for a relational model. redb's native `MultimapTable` and key-range scan are purpose-built for this access pattern, and reopening SQLite per CLI invocation adds connection overhead that redb's mmap avoids entirely.

### Rust Semantic Source: SCIP from rust-analyzer (not full LSP)

Rust extractors need semantic information beyond syntax (traits, method dispatch, type-directed calls). The v0 prototype uses **SCIP** (SCIP Code Intelligence Protocol) emitted by `rust-analyzer scip`, parsed via the `scip` crate.

**Why SCIP over a full LSP client:**
1. **One-shot artifact:** `rust-analyzer scip` produces a single protobuf file per crate; no server lifecycle to manage across CLI invocations.
2. **Stable format:** SCIP protobuf schema is versioned; LSP messages are an ongoing protocol with negotiation and state.
3. **No keep-warm:** The CLI opens redb, reads SCIP data, writes the index, and exits. An LSP client would need process management, initialization, and teardown per invocation.
4. **Graceful degradation:** When `rust-analyzer` is not on PATH, the extractor falls back to tree-sitter-only and emits a `provenance.warning` field — the extraction doesn't fail, it just has lower confidence.

Tree-sitter-rust and `cargo metadata` remain always-on for syntax-level facts (modules, structs, fns, crate graph). SCIP augments them with exact `calls` and `references` edges at `confidence: "exact"`.

### Contract Governance: Locked Enums in v0

The IR contract at `contracts/schemas/elegy-codegraph.graph.v0.json` locks two enums as closed sets:

**`Confidence`:** `exact | inferred | heuristic`
- `exact`: the extractor has verified this fact from a semantic source (e.g., SCIP `references` edge, TypeScript type-checker resolved call).
- `inferred`: the fact is derived from patterns or conventions (e.g., test detection by file path pattern, doc-link by nearby comment).
- `heuristic`: the fact is a best guess with no direct evidence (e.g., unresolved import marked as probable dependency).

**`SideEffect`:** `fs.read | fs.write | net.http | net.grpc | process.exec | db.read | db.write | env.read | env.write | os.signal`

Every `Entity` and `Edge` must carry a `Provenance` struct with `extractor`, `confidence`, and `evidence_refs`. Absent provenance is invalid. This forces extractors to be explicit about uncertainty rather than silently degrading evidence quality.

## Context Evidence

- `contracts/schemas/elegy-codegraph.graph.v0.json`: governed IR contract schema (Entity, Edge, Provenance, Confidence, SideEffect enums)
- `contracts/fixtures/elegy-codegraph.graph.v0.example.json`: example valid graph for contract validation
- `docs/specs/codegraph-diff-slice.md`: deferred-commands stub spec (diff, review, validate — follow-up after v0)
- `docs/specs/elegy-codegraph-plugin-v0.md`: implementation plan (this spec's companion — TODO: create alongside prototype)

## Requirements

### Product Shape (v0)

The v0 prototype ships as a single Rust binary `elegy-codegraph` in `rust/crates/elegy-codegraph/` with two subcommands:

```
elegy-codegraph extract --lang ts|rust --repo <path> --out <graph.bin> [--use-scip]
elegy-codegraph query   --graph <graph.bin> symbol --name <q> [--lang ts|rust]
elegy-codegraph query   --graph <graph.bin> neighbors --id <entity_id> --direction in|out
elegy-codegraph query   --graph <graph.bin> impact --path <file>
elegy-codegraph query   --graph <graph.bin> summary
```

`--use-scip` is meaningful only for `--lang rust`; TypeScript extraction always uses the Compiler API regardless of the flag.

All `query` subcommands emit compact JSON with `provenance` and `confidence` on every record. The plugin is host-neutral: it reads from the local filesystem, writes to a local redb file, and emits JSON to stdout. No host imports, no MCP adapter in v0.

### TypeScript And Rust Scope

### Concrete TypeScript Extractor Strategy

**Source:** TypeScript Compiler API (`typescript` npm package, invoked programmatically via Node.js subprocess or napi-rs binding). The extractor calls `ts.createProgram`, walks `SourceFile` nodes, and uses the `TypeChecker` for symbol resolution.

**Captured:**
- **Files and modules:** every `SourceFile`, `namespace`, and `module` declaration
- **Symbols:** exported functions, classes, methods, interfaces, type aliases, constants, enums
- **Calls:** via `checker.getResolvedSignature` at call sites, producing `calls` edges with `confidence: "exact"` when the callee is resolvable
- **Exports:** `export` declarations and `export default` produce `exports` edges
- **Tests:** detected by file path patterns (`*.test.ts`, `*.spec.ts`, `__tests__/**`) and conventional test-runner config presence (`vitest.config.*`, `jest.config.*`). Test files produce `tests` edges to the functions they call.
- **Docs:** JSDoc tags and `@elegy-doc` comments produce `documents` edges to documented symbols.

**Known gaps (v0):**
- Cross-package monorepo resolution depends on `tsconfig.json` `paths`/`references`; walked best-effort. Unresolvable imports marked as `confidence: "inferred"`.
- No runtime test-to-source binding (test runners are not executed); test detection is pattern-based.
- Decorators and experimental TC39 proposals are not traced.
- Dynamic `import()` expressions are recorded as `imports` edges when the argument is a literal string; otherwise not traced.

### Concrete Rust Extractor Strategy

The Rust extractor has two layers:

**Layer 1: Always-on syntax (tree-sitter + cargo metadata)**

Implemented in `extractor/rust_lang.rs`. Uses the `tree-sitter-rust` grammar for syntax-level extraction and `cargo metadata --format-version 1` for the crate graph.

**Captured:**
- **Modules:** `mod` declarations, file-based module resolution, inline modules
- **Symbols:** functions (`fn`), structs, enums, traits, impl blocks, type aliases, constants, statics
- **Re-exports:** `pub use` declarations produce `exports` edges to the re-exported symbol
- **Crate graph:** inter-crate dependency edges from `cargo metadata`
- **Tests:** detected by `#[cfg(test)]` modules, `#[test]` attributes, `#[tokio::test]`, and `tests/` directory layout. Test functions produce `tests` edges to the functions they call.
- **Docs:** doc comments (`///`, `//!`) produce `documents` edges to documented items.
- **Macros:** `macro_rules!` and `derive` macros recorded as `kind: "macro"` entities; macro bodies are NOT expanded.

**Layer 2: Opt-in semantic (SCIP from rust-analyzer)**

Implemented in `extractor/rust_scip.rs`. Invoked when `--use-scip` is passed to `extract`. Spawns `rust-analyzer scip` per workspace member, parses the SCIP protobuf via the `scip` crate, and merges results into the tree-sitter graph.

**Augments:**
- `calls` edges with `confidence: "exact"` (method dispatch, trait resolution, generic instantiation resolved by rust-analyzer)
- `references` edges with `confidence: "exact"` (every usage site across the workspace)

**Graceful degradation:** When `rust-analyzer` is not on PATH, the SCIP layer emits a `provenance.warning` on the extraction result. The tree-sitter layer still produces a complete syntax-level graph; only semantic precision is reduced.

**Known gaps (v0):**
- `#[cfg]` branches other than `test` are flattened; conditional compilation is not modeled.
- Trait method dispatch is recorded at the `impl` site, not the trait definition, unless SCIP is available and resolves the trait binding.
- `async fn` is recorded as a regular function; the state machine transform is not traced.
- Macro expansion is not performed; call sites inside macro invocations are invisible to the extractor.
- `build.rs` and proc-macro crates are parsed syntactically but their execution semantics are not modeled.

### Normalized Graph IR

The language-specific extractors feed a language-neutral graph IR defined by the governed schema at `contracts/schemas/elegy-codegraph.graph.v0.json`.

**Entity fields:**

| Field | Type | Description |
|---|---|---|
| `id` | `string` (SHA-1 of qualified name + file + kind) | Stable, content-addressable identifier |
| `kind` | `EntityKind` enum | `file`, `module`, `function`, `class`, `method`, `trait`, `impl`, `interface`, `type`, `constant`, `enum`, `macro`, `test`, `doc` |
| `layer` | `string` | `source`, `test`, `doc`, `build`, `config` |
| `name` | `string` | Short name (e.g. `calculateTotal`) |
| `qualified_name` | `string` | Fully qualified (e.g. `src::math::calculateTotal`) |
| `file` | `string` | Relative path from repo root |
| `span` | `{ start: [line, col], end: [line, col] } \| null` | Source location when known |
| `inputs` | `[{ name: string, type_hint: string \| null }]` | Parameter/input descriptions |
| `outputs` | `[{ type_hint: string \| null }]` | Return/output descriptions |
| `sideEffects` | `SideEffect[]` | Closed enum: `fs.read`, `fs.write`, `net.http`, `net.grpc`, `process.exec`, `db.read`, `db.write`, `env.read`, `env.write`, `os.signal` |
| `dependencies` | `string[]` (entity IDs) | Direct dependency references |
| `tests` | `string[]` (entity IDs) | Test entities that cover this entity |
| `docs` | `string[]` (entity IDs) | Doc entities that document this entity |
| `provenance` | `Provenance` | **Required.** `{ extractor, confidence, evidence_refs }` |

**Edge fields:**

| Field | Type | Description |
|---|---|---|
| `src` | `string` (entity ID) | Source entity |
| `dst` | `string` (entity ID) | Target entity |
| `kind` | `EdgeKind` enum | `imports`, `exports`, `calls`, `references`, `reads`, `writes`, `validates`, `emits`, `owns`, `tests`, `documents` |
| `provenance` | `Provenance` | **Required.** `{ extractor, confidence, evidence_refs }` |

**Confidence enum (locked, closed):**

| Value | Meaning |
|---|---|
| `exact` | Verified from a semantic source (SCIP, type-checker) |
| `inferred` | Derived from patterns or conventions (file-path test detection, JSDoc) |
| `heuristic` | Best guess with indirect evidence (unresolved import, likely dependency) |

**Storage shape in redb:**

```
Table: entities           (id: &str)                    -> Entity (JSON)
Table: entities_by_name   (name: &str)                  -> id (&str)
Table: files              (path: &str)                  -> id (&str)
Table: outgoing           (src_id: &str)                -> MultimapTableValue: (dst_id, edge_kind)
Table: incoming           (dst_id: &str)                -> MultimapTableValue: (src_id, edge_kind)
```

`MultimapTable` is redb's native one-to-many structure, giving O(log n) neighbor lookups via key-range scan. `compact()` is called after bulk extraction to reclaim COW B+tree fragmentation.

### Tool Stack (v0)

| Tool | Role | Why |
|---|---|---|
| **TypeScript Compiler API** | Primary TS semantic source | Native symbol/type/checker; single dep, no IPC |
| **tree-sitter-rust** | Rust syntax extraction | Fast, incremental, covers modules/structs/fns/traits/impls |
| **cargo metadata** | Rust crate graph | Standard, machine-readable dependency graph from Cargo |
| **rust-analyzer (SCIP)** | Rust semantic edges (calls, references) | One-shot protobuf artifact; no LSP server lifecycle |
| **scip crate** | SCIP protobuf parsing | Official Rust SCIP library, MIT licensed |
| **redb** | Embedded graph storage | ACID/MVCC, stable on-disk format, MultimapTable for edges |
| **clap** | CLI argument parsing | Standard Rust CLI framework |
| **serde / serde_json** | Serialization | IR types + JSON output |

**Not used in v0:**
- **ast-grep, Semgrep, CodeQL:** quality/rule-check tools deferred to the `review` and `validate` commands (follow-up spec).
- **Joern / Code Property Graph:** too heavy for v0; remains a research reference.
- **LSP client:** SCIP replaces the need for a full LSP lifecycle.
- **sled:** replaced by redb (see Design Decisions).

### Agent Interface (v0)

All queries emit JSON to stdout. Every result record includes `provenance` and `confidence`.

| Query | Command | Output |
|---|---|---|
| Symbol lookup | `query symbol --name <q> [--lang ts\|rust]` | Matching entities with full metadata |
| Neighbors | `query neighbors --id <id> --direction in\|out` | Array of (entity, edge_kind, provenance) |
| File impact | `query impact --path <file>` | Entities in file + immediate outgoing neighbors |
| Repo summary | `query summary` | Counts by entity kind, edge kind, file count, extractor metadata |

Non-zero exit code + JSON error on not-found. Agents consume via the standard CLI invocation template; no MCP adapter in v0.

## Non-Goals

- Do not implement `diff`, `review`, or `validate` commands in v0 (deferred to codegraph-diff-slice.md).
- Do not implement an MCP adapter in v0 (CLI invocation templates are the integration contract).
- Do not build a full language-independent semantic analyzer from scratch.
- Do not make Joern or a full Code Property Graph mandatory for the first plugin slice.
- Do not implement host-specific dashboards, approvals, UI orchestration, or workflow repair in Elegy.
- Do not claim the graph proves behavior correctness.
- Do not ship broad agent-facing tools until stale-index behavior, provenance, and confidence levels are validated.

## Acceptance Checks (v0)

1. The spec documents a concrete TypeScript extractor strategy (Compiler API) with known gaps.
2. The spec documents a concrete Rust extractor strategy (tree-sitter + cargo metadata + SCIP) with known gaps.
3. A prototype can index at least one TypeScript fixture repo and one Rust fixture repo for files, modules, symbols, imports/exports, and test/doc links (validated by `rust/tests/fixtures/ts-mini/` and `rust/tests/fixtures/rust-mini/`).
4. Query results include `provenance` and `confidence` on every record; absent fields are a validation failure.
5. The plugin boundary stays host-neutral: no host imports in the `elegy-codegraph` crate; CLI emits JSON only.
6. A governed contract artifact exists at `contracts/schemas/elegy-codegraph.graph.v0.json` with a validating example fixture at `contracts/fixtures/elegy-codegraph.graph.v0.example.json`.
7. A stub spec for deferred commands exists at `docs/specs/codegraph-diff-slice.md` capturing the design intent for `diff`, `review`, and `validate`.

## Implementation Links

- `docs/architecture/ecosystem-topology.md`
- `docs/architecture/mcp-skill-tooling-placement.md`
- `docs/specs/host-neutral-plugin-install.md`
- `docs/specs/plugin-tool-availability.md`
- `contracts/schemas/elegy-codegraph.graph.v0.json`
- `contracts/fixtures/elegy-codegraph.graph.v0.example.json`
- `docs/specs/codegraph-diff-slice.md`

## Validation Evidence

- **Extractor strategies:** Concrete TS and Rust strategies documented above with known gaps explicitly listed.
- **Fixture repos:** `rust/tests/fixtures/ts-mini/` and `rust/tests/fixtures/rust-mini/` committed in-tree with expected graph snapshots.
- **Integration tests:** `cargo test -p elegy-codegraph` exercises extract + query against both fixtures; asserts entity counts, expected kinds, tests edges, and provenance presence.
- **Contract validation:** `contracts/fixtures/elegy-codegraph.graph.v0.example.json` validates against `contracts/schemas/elegy-codegraph.graph.v0.json`.
- **Design decisions:** All architectural choices documented in the Design Decisions section above with rationale and alternatives considered.

Full test run output and schema validation output to be linked here once the prototype crate passes CI.

## Deferred Commands

The `diff`, `review`, and `validate` commands are deferred to a follow-up spec. See [`docs/specs/codegraph-diff-slice.md`](./codegraph-diff-slice.md) for the design intent, acceptance criteria template, and integration plan. A TODO in `rust/crates/elegy-codegraph/src/main.rs` marks the deferred command stubs.

## Drift Notes

- This spec was promoted from `draft` (research) to `active` (implementation) on 2026-06-15. The research phase confirmed that existing tools (tree-sitter, SCIP, TypeScript Compiler API) are sufficient for a v0 prototype without building a custom analysis engine. The prototype validates the hypothesis that a language-neutral graph IR with mandatory provenance is useful for agent-driven code exploration.
