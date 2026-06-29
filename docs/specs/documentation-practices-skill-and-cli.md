---
title: Documentation practices skill and CLI
status: active
owner: Elegy
---

# Documentation practices skill and CLI

## Problem

- Documentation commands and docs config must stay aligned with the binaries and
  present repo shape.
- The repo needs objective docs validation without preserving stale prose.

## Goals

- Keep a compact docs skill and deterministic docs CLI.
- Provide `elegy-documentation init/new/check/index`.
- Validate objective properties only: config shape, metadata, statuses,
  filename rules, required headings, and broken internal links.
- Support repo-local adoption through `.elegy/docs.yaml`.

## Non-Goals

- Do not auto-score prose quality or architectural soundness.
- Do not preserve obsolete migration prose as current authority.

## Behavior

- `elegy-documentation init` creates `.elegy/docs.yaml`, seed README files for ADR/spec
  lanes, and a docs index if they do not already exist.
- `elegy-documentation new adr` creates `docs/adr/YYYY-MM-DD-slug.md` using a compact ADR
  template and supported ADR statuses.
- `elegy-documentation new spec` creates `docs/specs/slug.md` using a compact spec
  template and supported spec statuses.
- `elegy-documentation check` succeeds on empty repos with no docs config and reports
  only objective failures when docs are present.
- `elegy-documentation index` rewrites the configured docs index file from discovered ADR
  and spec documents.
- Local config stays repo-relative and supports only ADR path, spec path, index
  path, required doc triggers, and narrow local exceptions.

## Acceptance Criteria

- [x] `elegy-documentation init/new/check/index` are implemented.
- [x] Objective validation catches invalid metadata, invalid statuses, filename
  mismatches, missing required headings, and broken internal links.
- [x] Repo-local `.elegy/docs.yaml` overrides work for non-default ADR/spec/index paths.
- [x] Repo docs point at current binaries and current authority surfaces.

## Validation

- `cargo test -p elegy-documentation`
- `cargo fmt --all`
- `cargo run -p elegy-documentation -- check --project .`

## Links

- [Centralize documentation practices doctrine ADR](../adr/2026-05-25-centralize-documentation-practices-doctrine.md)
- [Documentation practices architecture doc](../architecture/documentation-practices.md)
