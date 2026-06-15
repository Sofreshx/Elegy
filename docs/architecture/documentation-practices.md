# Documentation Practices

## Purpose

This document defines the current central documentation doctrine for ADRs, specs,
guides, notes, and roadmap material in the Elegy ecosystem.

The v1 posture is intentionally lean:

- centralize doctrine in `elegy`
- distribute behavior as a reusable skill package
- use a deterministic CLI only for file creation and objective validation
- keep subjective review with humans

## Current Split

The current split is:

- `skills/elegy-doc-practices/` holds the reusable instruction package, doctrine references, templates, eval fixtures, and adoption examples
- `elegy docs ...` provides deterministic file creation, indexing, and objective validation
- `.elegy/docs.yaml` in a consuming repo defines local ADR/spec/index paths plus narrow overrides

The skill is the instructions layer. The CLI is the tool layer.

## Document Types

- ADR: durable decision plus alternatives and consequences
- Spec: intended behavior plus acceptance criteria and validation
- Guide: recurring contributor or operator instructions
- Note: narrow local context that does not justify stronger governance
- Roadmap: ordered future work across multiple slices

## Placement

- shared doctrine and cross-project conventions belong in `elegy`
- product-specific ADRs and specs belong in the owning repo
- cross-repo decisions belong centrally and should be linked locally from affected repos

Do not duplicate the shared doctrine into every downstream repo.

## CLI Scope

Current commands:

```text
elegy docs init
elegy docs new adr --title <title>
elegy docs new spec --title <title>
elegy docs check
elegy docs index
```

The CLI validates only objective properties:

- config shape
- metadata presence
- supported status values
- filename conventions
- required headings
- broken internal links

The CLI does not judge prose quality or architectural correctness.

## Enforcement Phases

Phase 1:

- PR checklist
- contributor instructions for documentation impact review

Phase 2:

- non-blocking `elegy docs check` in CI

Phase 3:

- blocking CI only for objective failures and explicitly configured high-impact changes

Do not block automatically on subjective document quality.

## Adoption

Consumer repos such as `holon` and `elegy-copilot` should:

1. reference the shared `elegy-doc-practices` skill
2. define only local path and trigger overrides in `.elegy/docs.yaml`
3. keep product ADRs and specs local unless the decision is cross-repo

See `skills/elegy-doc-practices/adoption/` for copy-paste snippets.
