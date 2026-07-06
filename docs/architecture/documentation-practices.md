# Documentation Practices

## Purpose

This document defines the current documentation doctrine for Elegy.

## Doctrine

- Keep present-state docs short and current.
- Keep durable decisions in ADRs.
- Keep implementation-facing behavior in specs.
- Delete migration notes once the migration is complete.
- Do not preserve wrong current-state docs for historical comfort.

## Document Types

- ADR: durable decision plus alternatives and consequences
- Spec: intended behavior plus acceptance criteria and validation
- Guide: recurring contributor or operator instructions
- Note: narrow local context that does not justify stronger governance
- Roadmap: active future work only

## Placement

- Root `README.md` is the public entrypoint.
- `docs/architecture/` holds current-state architecture guidance.
- `docs/adr/` holds durable decisions.
- `docs/specs/` holds implementation-facing specs.
- Historical material should be removed once it is no longer current or required.

## CLI Scope

Current commands:

```text
elegy-documentation init
elegy-documentation new adr --title <title>
elegy-documentation new spec --title <title>
elegy-documentation check
elegy-documentation index
```

The CLI validates objective properties only:

- config shape
- metadata presence
- supported status values
- filename conventions
- required headings
- broken internal links

The CLI does not judge prose quality or architectural correctness.

## Review Rules

- Prefer one canonical current-state doc over multiple overlapping summaries.
- Prefer links over repeated policy text.
- Remove stale paths, stale binary names, and stale migration framing in the
  same change that makes them obsolete.
- Run `elegy-documentation check --project .` after documentation changes.
