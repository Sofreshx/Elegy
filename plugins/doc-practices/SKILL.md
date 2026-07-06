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
   doc](../../../architecture/documentation-practices.md) and decide whether the
   work needs an ADR (durable decision), a spec (implementation-facing behavior),
   a guide (recurring instructions), a note (narrow local context), or a
   roadmap entry (multi-slice future work).
2. **Scaffold the artifact** — use `elegy docs new adr --title "..."` or
   `elegy docs new spec --title "..."` to get a file with required frontmatter.
3. **Adopt locally** — copy the snippets in
   `plugins/doc-practices/adoption/` into the consuming repo's
   `.elegy/docs.yaml` and CI workflow.
4. **Validate objectively** — run `elegy-documentation check --project . --json`
   to surface missing frontmatter, invalid statuses, broken links, unparseable
   dates, and freshness warnings. The CLI does not score prose quality.
5. **Treat prose review as human work** — automation covers objective failures
   only. Reasoning, structure, and architecture taste stay with humans.

## Document types

| Type | When to use | Authority root | Status values |
| --- | --- | --- | --- |
| **ADR** | Durable architecture or governance decision with alternatives and consequences. | `docs/adr/` | `proposed`, `accepted`, `superseded`, `rejected` |
| **Spec** | Implementation-facing behavior with acceptance criteria and validation evidence. | `docs/specs/` | `draft`, `active`, `completed`, `superseded`, `deprecated` |
| **Guide** | Recurring contributor or operator instructions. | `docs/architecture/` (current) | `active`, `superseded` |
| **Note** | Narrow local context that does not justify stronger governance. | `docs/architecture/` (current) | `active`, `archived` |
| **Roadmap** | Ordered future work across multiple slices. | `docs/roadmaps/` | `active`, `completed` |

Use the lightest type that captures the work. Do not force every change into
an ADR or spec.

## Placement

- **Shared doctrine and cross-project conventions** belong in Elegy
  (`docs/architecture/`, `docs/adr/`, `docs/specs/`).
- **Product-specific ADRs and specs** belong in the owning repo.
- **Cross-repo decisions** belong centrally and are linked locally from
  affected repos.

Do not duplicate the shared doctrine into every downstream repo.

## Required frontmatter

Every current-authority doc must carry non-empty frontmatter for:

- `title`
- `status`
- `owner`

Every spec additionally needs `doc_kind` (one of `adr`, `spec`, `guide`,
`reference`, `planning`, `research`, `generated`, `index`, `system`).
`created` and `updated` are recommended for freshness tracking.

## Local adoption

Downstream repos such as `holon` and `elegy-copilot` should:

1. Reference this skill from their `.github/instructions/` or equivalent.
2. Define only local path, trigger, and exception overrides in
   `.elegy/docs.yaml`.
3. Keep product ADRs and specs local unless the decision is cross-repo.

See `plugins/doc-practices/adoption/` for copy-paste snippets.

## Validation

```bash
elegy-documentation inspect --project . --json
elegy-documentation map --project . --json
elegy-documentation check --project . --json
elegy-documentation export llms --project . --output .elegy/llms.txt
elegy-documentation export bundle --project . --output .elegy/docs-bundle.json
```

The CLI covers only objective failures. Prose quality, architecture taste, and
correctness of reasoning require human review.

## Enforcement phases

- **Phase 1** — PR checklist; contributor instructions for documentation impact.
- **Phase 2** — non-blocking `elegy-documentation check` in CI.
- **Phase 3** — blocking CI only for objective failures and explicitly
  configured high-impact changes.

Do not block automatically on subjective document quality.

## Companion skills

- `elegy-skills` — for governed skill-registry work that produces a skill
  definition or package.
- `elegy-planning` — for durable planning state (goals, roadmaps, plans).
- `elegy-mcp` — for MCP descriptor authoring and analysis.

## Related

- [Centralize documentation practices doctrine ADR](../../../adr/2026-05-25-centralize-documentation-practices-doctrine.md)
- [Documentation practices architecture doc](../../../architecture/documentation-practices.md)
- [Documentation practices skill and CLI spec](../../../specs/documentation-practices-skill-and-cli.md)
