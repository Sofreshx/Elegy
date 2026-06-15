---
title: Codegraph and Quality Plugin Research
status: draft
owner: Elegy
created: 2026-06-13
updated: 2026-06-13
doc_kind: spec
summary: Research contract for portable codebase graph extraction, analyzer rule packs, and agent-facing code quality evidence across TypeScript and Rust.
---

# Codegraph and Quality Plugin Research

## Problem

Agents need better structural evidence about a codebase than repeated full-file reading or expensive end-to-end tests can provide. The proposed `elegy-codegraph` idea could help agents understand modules, symbols, dependencies, tests, docs, patterns, and change impact, but it is not a solved product yet. Code extraction across TypeScript and Rust is especially risky because syntax parsing and semantic understanding require different tools and assumptions.

## Goals

1. Define whether a portable `elegy-codegraph` plugin is viable as an Elegy-owned research/product direction.
2. Handle both TypeScript and Rust directly in the research scope, even if their extractors use different techniques.
3. Reuse existing open-source analyzers and graph ideas before building new analysis engines.
4. Produce a normalized graph that agents and hosts can query for structural evidence.
5. Keep host-specific UI, approvals, policy, secrets, local execution, and runtime evidence out of Elegy.

## Context Evidence

- `docs/architecture/ecosystem-topology.md`: Elegy is a contracts-and-tooling monorepo with governed artifact roots and reusable Rust executable surfaces.
- `docs/architecture/mcp-skill-tooling-placement.md`: shared executable capabilities that multiple consumers should use belong in Rust tooling; host-specific UI, auth, policy, and orchestration stay in the consumer.
- `docs/specs/host-neutral-plugin-install.md`: establishes host-neutral plugin install ergonomics and command shim direction.
- `docs/specs/plugin-tool-availability.md`: records current plugin/tool availability concerns and should be reconciled before exposing new agent tools.
- External adjacent tools:
  - Tree-sitter: incremental syntax parsing across many languages; useful for cheap syntax extraction, but not enough for full semantic meaning.
  - ast-grep: Tree-sitter-based structural search, linting, and code rewrite rules.
  - Semgrep: configurable static analysis and custom project rule packs.
  - CodeQL: deeper query-based correctness/security analysis.
  - Joern / Code Property Graphs: graph-oriented program analysis; closest to the full code-property-graph idea but likely too heavy for v1.
  - SCIP / LSIF and language servers: useful sources for definitions, references, and navigation where available.

## Requirements

### Product Shape

- Research a portable `elegy-codegraph` plugin/package with commands such as:
  - `extract`
  - `diff`
  - `query`
  - `review`
  - `validate`
- Treat the plugin as experimental until extraction quality, performance, and agent usefulness are demonstrated on real TypeScript and Rust repos.
- The plugin should produce structural evidence, not correctness proof. It may reduce blind exploration and improve review/debugging, but it must not replace tests.

### TypeScript And Rust Scope

- TypeScript and Rust must both be first-class research targets.
- TypeScript research should evaluate Tree-sitter, ast-grep, Semgrep, TypeScript compiler or language-server data, and SCIP-style indexes.
- Rust research should evaluate Tree-sitter, rust-analyzer, Cargo metadata, Semgrep/ast-grep feasibility, and any available SCIP/index data.
- Do not pretend Tree-sitter alone gives enough semantic information for Rust traits, macros, cfg flags, or type-directed call relationships.
- The research may deliver uneven extractor depth by language, but both languages must have an explicit extractor strategy, known gaps, and acceptance examples.

### Normalized Graph

- The language-specific extractors should feed a language-neutral graph IR.
- Candidate `Entity` fields:
  - `kind`
  - `layer`
  - `name`
  - `inputs`
  - `outputs`
  - `sideEffects`
  - `dependencies`
  - `tests`
  - `docs`
- Candidate `Edge` kinds:
  - `imports`
  - `exports`
  - `calls`
  - `references`
  - `reads`
  - `writes`
  - `validates`
  - `emits`
  - `owns`
  - `tests`
  - `documents`
- The IR must preserve provenance: file path, span or symbol location when known, extractor name, extractor version, and confidence level.

### Reused Tools And Why

- Tree-sitter is useful for fast syntax extraction and language coverage, especially when only structural facts are needed.
- ast-grep is useful for project convention checks and structural pattern rules over Tree-sitter ASTs.
- Semgrep is useful for portable rule packs that enforce security, correctness, and codebase-specific conventions.
- CodeQL is useful for deeper security/correctness queries when setup and licensing fit the target repo.
- Joern is useful as a research reference for code property graphs, but should not be assumed as the v1 engine until its cost, language fit, and integration overhead are proven.
- rust-analyzer is likely required for higher-quality Rust semantic evidence.
- SCIP or LSIF-style indexes are useful if they can provide definitions/references more reliably than custom extraction.

### Agent Interface

- Candidate agent queries:
  - repository structural summary
  - symbol search
  - callers/callees
  - module dependency graph
  - impact analysis for changed files
  - tests/docs likely related to a symbol or module
  - convention/rule findings relevant to a path
  - duplicate or competing concept names
- Query output must be compact, machine-readable, and cite source files/spans or explicitly mark evidence as inferred.

## Non-Goals

- Do not build a fully language-independent semantic analyzer from scratch.
- Do not make Joern or a full Code Property Graph mandatory for the first plugin slice.
- Do not implement host-specific dashboards, approvals, UI orchestration, or workflow repair in Elegy.
- Do not claim the graph proves behavior correctness.
- Do not ship broad agent-facing tools until stale-index behavior, provenance, and confidence levels are researched.

## Acceptance Checks

- Research produces a concrete TypeScript extraction strategy and a concrete Rust extraction strategy, each with known gaps.
- A prototype can index at least one TypeScript repo and one Rust repo for files, modules, symbols, imports/exports or module relationships, and test/doc links where detectable.
- A graph diff can show structurally relevant changes for a branch or file set.
- Query results include provenance and confidence rather than unsupported summaries.
- The plugin boundary stays host-neutral: Elegy owns portable extraction/rule/query capability, while Elegy-copilot and Holon consume results through their own UI/runtime boundaries.

## Implementation Links

- `docs/architecture/ecosystem-topology.md`
- `docs/architecture/mcp-skill-tooling-placement.md`
- `docs/specs/host-neutral-plugin-install.md`
- `docs/specs/plugin-tool-availability.md`
- External references: `https://tree-sitter.github.io/`, `https://ast-grep.github.io/`, `https://semgrep.dev/docs/writing-rules/overview`, `https://codeql.github.com/docs/`, `https://github.com/joernio/joern`, `https://github.com/scip-code/scip`, `https://rust-analyzer.github.io/`

## Validation Evidence

- Pending research. No extractor, graph IR, or plugin command has been validated yet.

## Drift Notes

- This spec intentionally records an unproven direction. If research shows existing tools already solve enough of the problem, prefer wrapping those tools over building a custom graph engine.
