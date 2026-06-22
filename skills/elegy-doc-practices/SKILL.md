---
name: elegy-doc-practices
description: Use when an agent or contributor needs to classify, place, scaffold, or validate documentation in the Elegy ecosystem — the central doctrine for ADRs, specs, guides, notes, and roadmap material across Elegy and downstream repos.
---

# Elegy Documentation Practices

> Use when a contributor or agent needs to apply the central Elegy documentation
> doctrine: decide whether a change needs an ADR, spec, guide, note, or roadmap
> entry, scaffold it with the right template, and validate it objectively.

The central doctrine lives in Elegy. Downstream repos adopt it by referencing
this skill and configuring local paths and triggers through
`.elegy/docs.yaml`. The doctrine is durable; the per-repo configuration is local.

## Quick start

1. **Classify the change** — read the [Documentation practices architecture
   doc](../../architecture/documentation-practices.md) and decide whether the
   work needs an ADR, spec, guide, note, or roadmap entry.
2. **Scaffold the artifact** — use `elegy docs new adr --title "..."` or
   `elegy docs new spec --title "..."` to get a file with required frontmatter.
3. **Adopt locally** — copy the snippets in `adoption/` into the consuming
   repo's `.elegy/docs.yaml` and CI workflow.
4. **Validate objectively** — run `elegy-documentation check --project . --json`
   to surface missing frontmatter, invalid statuses, broken links, unparseable
   dates, and freshness warnings.
5. **Treat prose review as human work** — automation covers objective failures
   only. Reasoning, structure, and architecture taste stay with humans.

## Document types

| Type | When to use | Authority root | Status values |
| --- | --- | --- | --- |
| **ADR** | Durable architecture or governance decision with alternatives and consequences. | `docs/adr/` | `proposed`, `accepted`, `superseded`, `rejected` |
| **Spec** | Implementation-facing behavior with acceptance criteria and validation evidence. | `docs/specs/` | `draft`, `active`, `completed`, `superseded`, `deprecated` |
| **Guide** | Recurring contributor or operator instructions. | `docs/architecture/` | `active`, `superseded` |
| **Note** | Narrow local context that does not justify stronger governance. | `docs/architecture/` | `active`, `archived` |
| **Roadmap** | Ordered future work across multiple slices. | `docs/roadmaps/` | `active`, `completed` |

Use the lightest type that captures the work. Do not force every change into
an ADR or spec.

## Required frontmatter

Every current-authority doc must carry non-empty frontmatter for:

- `title`
- `status`
- `owner`

Every spec additionally needs `doc_kind` (one of `adr`, `spec`, `guide`,
`reference`, `planning`, `research`, `generated`, `index`, `system`).
`created` and `updated` are recommended for freshness tracking.

## Related

- [Centralize documentation practices doctrine ADR](../../adr/2026-05-25-centralize-documentation-practices-doctrine.md)
- [Documentation practices architecture doc](../../architecture/documentation-practices.md)
- [Documentation practices skill and CLI spec](../../specs/documentation-practices-skill-and-cli.md)
- [Governed skill fixture: skill.elegy-doc-practices.json (deleted, replaced by this skill)](../../contracts/fixtures/skill.elegy-doc-practices.json)
