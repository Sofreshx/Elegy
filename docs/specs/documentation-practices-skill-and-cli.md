---
title: Documentation practices skill and CLI
status: active
owner: Elegy
---

# Documentation practices skill and CLI

## Problem

- Elegy needs a practical way to apply the new documentation doctrine without
  relying on vague contributor memory or subjective automation.
- The repo also needs a repeatable path for downstream adoption in `holon` and
  `elegy-copilot` without turning every consumer into a fork of the doctrine.

## Goals

- Provide a central `skills/elegy-doc-practices/` package with compact workflow
  instructions, deeper references, templates, and adoption snippets.
- Add `elegy docs init`, `elegy docs new adr`, `elegy docs new spec`,
  `elegy docs check`, and `elegy docs index`.
- Validate objective properties only: config shape, metadata, statuses,
  filename rules, required headings, and broken internal links.
- Support repo-local adoption through `.elegy/docs.yaml` path and trigger
  overrides.

## Non-Goals

- Do not auto-score prose quality or architectural soundness.
- Do not build a multi-agent documentation governance runtime in v1.
- Do not force every change into an ADR or spec when a normal note is enough.

## Behavior

- `elegy docs init` creates `.elegy/docs.yaml`, seed README files for ADR/spec
  lanes, and a docs index if they do not already exist.
- `elegy docs new adr` creates `docs/adr/YYYY-MM-DD-slug.md` using a compact ADR
  template and supported ADR statuses.
- `elegy docs new spec` creates `docs/specs/slug.md` using a compact spec
  template and supported spec statuses.
- `elegy docs check` succeeds on empty repos with no docs config and reports
  only objective failures when docs are present.
- `elegy docs index` rewrites the configured docs index file from discovered ADR
  and spec documents.
- Local config stays repo-relative and supports only ADR path, spec path, index
  path, required doc triggers, and narrow local exceptions.

## Acceptance Criteria

- [x] A central `skills/elegy-doc-practices/` package exists with `SKILL.md`,
  doctrine references, assets, eval fixtures, and adoption examples.
- [x] `elegy docs init/new/check/index` are implemented on the umbrella CLI.
- [x] Objective validation catches invalid metadata, invalid statuses, filename
  mismatches, missing required headings, and broken internal links.
- [x] Repo-local `.elegy/docs.yaml` overrides work for non-default ADR/spec/index paths.
- [x] Repo docs include phase-based enforcement guidance: PR checklist first,
  advisory CI second, blocking only for objective failures later.

## Validation

- `cargo test -p elegy-tooling`
- `cargo test -p elegy-cli docs`
- `cargo fmt --all`
- `cargo test -p elegy-cli`
- `cargo run -p elegy-cli -- --project .. docs check --json`

## Links

- [Centralize documentation practices doctrine ADR](../adr/2026-05-25-centralize-documentation-practices-doctrine.md)
- [Documentation practices architecture doc](../architecture/documentation-practices.md)
