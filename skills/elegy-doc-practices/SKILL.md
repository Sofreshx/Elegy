---
name: elegy-doc-practices
description: "Use when deciding whether a change needs an ADR, spec, guide, or note; when choosing central versus local doc placement; or when adopting Elegy documentation practices in another repo."
---

# Elegy Documentation Practices

Use this skill when a task changes architecture, durable decisions, implementation behavior, contributor guidance, or cross-repo conventions.

## Workflow

1. Classify the documentation need.
2. Choose the document type: ADR, spec, guide, note, or roadmap.
3. Choose placement: central Elegy doctrine or the owning product repo.
4. Draft a new document or update the existing one.
5. Run `elegy docs check` for objective validation when repo-local docs config exists.

## Classification

- Use an ADR for a durable decision with alternatives and lasting consequences.
- Use a spec for intended behavior before or during implementation.
- Use a guide for recurring contributor or operator instructions.
- Use a note for narrow context that does not justify a stronger artifact.
- Use a roadmap for staged future work across multiple slices.

## Placement

- Put cross-project doctrine and shared conventions in `elegy`.
- Put product-specific ADRs and specs in the owning repo.
- Put cross-repo decisions in `elegy`, then link from affected repos.
- Do not duplicate shared doctrine into every consumer repo.

## Review

- Prefer updating an existing ADR or spec when the change extends the same decision or behavior.
- Create a new ADR or spec when the change establishes a distinct durable decision or a distinct behavior contract.
- Keep docs checks objective: metadata, filenames, required headings, and internal links.
- Leave prose quality and architectural correctness to human review.

## References

- `references/taxonomy.md`
- `references/placement.md`
- `references/adr-workflow.md`
- `references/spec-workflow.md`
- `references/adoption.md`
- `references/review-checklist.md`
- `references/examples.md`

## Assets

- `assets/adr-template.md`
- `assets/spec-template.md`
- `assets/docs-index-template.md`
- `assets/repo-adoption-snippet.md`
- `assets/pr-checklist-snippet.md`
