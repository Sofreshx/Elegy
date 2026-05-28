# Codex Plugin Projection

## Purpose

This document describes the current Codex projection slice implemented in Elegy.

The goal is to project governed portable package metadata into a conservative
Codex plugin folder without promoting Codex files into authority roots.

## Authority chain

The authority chain is one-way:

1. `contracts/schemas/elegy-plugin-package-v1.schema.json` and `contracts/schemas/elegy-plugin-package-v2.schema.json` define the portable package contracts.
2. `contracts/fixtures/elegy-plugin-package-v1.minimal.json`, `contracts/fixtures/elegy-plugin-package-v2.minimal.json`, and real package instances are the governed package inputs.
3. Generated Codex plugin files are derived projections only.

Codex plugin files do not become authored truth for Elegy behavior, package
metadata, skill authority, connector ownership, or host policy.

## Current implemented slice

The current implementation adds `elegy generate codex-plugin` on the umbrella
CLI as lower-level contributor tooling.

It currently generates:

- `.codex-plugin/plugin.json`
- `skills/<projected-id>/SKILL.md`

The generated `skills/` directory is built from:

- embedded governed skill definitions in the portable package
- skill definitions loaded from `definitionRef` when the package points at local files
- instruction-skill files loaded from package-relative `instructionSkills[].path` when those files exist locally
- fallback instruction-skill placeholders when the package only carries instruction-skill metadata and not the original markdown body

Generated skill directory names are intentionally stable and non-lossy. They are
derived from the fully qualified skill identity for governed skills and from the
declared relative instruction-skill path for instruction skills, using a
case-safe encoded form instead of a lossy basename.

## What is intentionally not generated yet

The current slice does not generate:

- `.mcp.json`
- `.app.json`
- `hooks/hooks.json`
- marketplace metadata such as `.agents/plugins/marketplace.json`

Reason:

- `elegy-plugin-package/v1` and `elegy-plugin-package/v2` carry portable MCP projection metadata, and v2 additionally carries configuration components, but neither contract yet carries enough Codex-runnable MCP launch information to emit a truthful `.mcp.json`.
- Connector identity, auth, state, trust, and install/runtime UX remain host-owned and are therefore outside the first derived projection slice.
- Hook packaging and execution policy are also host/runtime concerns and remain out of scope for this first projection pass.

## Current command

```text
elegy generate codex-plugin --package <path> --output-dir <dir> [--force]
```

When `--force` is used, the generator replaces the existing plugin root for that
projected plugin name before writing the fresh output so stale generated files
do not survive across reruns.

This is contributor tooling, not a claim that Elegy ships a Codex plugin runtime,
plugin marketplace, or connector-management product surface.

## Generated manifest posture

The generated `.codex-plugin/plugin.json` is intentionally conservative.

It currently projects:

- plugin identity from package identity
- description, homepage, license, and tags when the portable package provides them
- `skills: "./skills/"`
- minimal Codex `interface` metadata when the portable package contains enough descriptive fields

It does not currently claim bundled apps, MCP servers, or hooks.

## Validation posture

The current evidence for this slice is:

- reusable generation logic in `rust/crates/elegy-tooling`
- umbrella CLI coverage in `rust/crates/elegy-cli`
- focused tooling and CLI tests for `generate codex-plugin`

If future work adds `.mcp.json`, `.app.json`, or marketplace output, update the
portable package contract, generator behavior, and docs together.
