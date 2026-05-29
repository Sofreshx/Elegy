---
name: elegy-documentation
description: "Derived repo-local skill bridge mirror for Elegy's dedicated documentation authority surface. Use for authority-aware documentation inspect/map/check/export through the dedicated elegy-documentation CLI."
---

# Elegy Documentation

This file is a repo-local, non-authoritative rendered skill bridge mirror.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-documentation.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-documentation.json` is the governed discovery projection derived from that definition.
3. `.agents/skills/elegy-documentation/SKILL.md` and `.github/skills/elegy-documentation/SKILL.md` are repo-local rendered mirrors only.

## When to use

- Prefer the dedicated `elegy-documentation` binary when a repo needs authority-aware docs inspection, corpus mapping, objective validation, or deterministic non-authoritative exports.
- Use `inspect` or `map` to classify current, planning, research, generated, and other document roots.
- Use `check` to validate metadata, statuses, dates, internal links, freshness warnings, and derived export drift without scoring prose quality.
- Use `export llms` or `export bundle` only for derived handoff outputs; source documents remain authoritative.

## Do not use

- Do not treat this skill mirror as documentation authority; governed fixtures and source documents remain the truth.
- Do not infer that generated `llms` or bundle outputs are canonical documentation.
- Do not use this surface to rewrite prose, auto-judge architecture quality, or invent missing documentation governance beyond the configured objective checks.

## Current commands

```text
elegy-documentation init --project <path> [--dry-run]
elegy-documentation inspect --project <path>
elegy-documentation map --project <path>
elegy-documentation check --project <path>
elegy-documentation export llms --project <path> --output <path>
elegy-documentation export bundle --project <path> --output <path>
```

Use `--json` for machine-mode output.

## Surface posture

- This dedicated surface is documentation-authority aware, but it does not move truth out of source documents.
- The umbrella `elegy docs ...` surface remains the existing compatibility path for v1 ADR/spec scaffolding behavior.
- `skills/elegy-doc-practices/` remains the reusable instruction package for documentation doctrine; this surface is the deterministic tool lane.
