---
name: elegy-doc-practices
description: Use when deciding whether a change needs an ADR, spec, guide, or note; when choosing central versus local doc placement; or when adopting Elegy documentation practices in another repo.
---

# Elegy Documentation Practices

> Use when deciding whether a change needs an ADR, spec, guide, or note; when choosing central versus local doc placement; or when adopting Elegy documentation practices in another repo.

## Quick start

1. Classify: does the change need an ADR (durable decision), a spec
   (behavior contract), a guide (recurring instructions), a note
   (narrow context), or a roadmap (staged future work)?
2. Choose placement: cross-project → Elegy; product-specific → the
   owning repo.
3. Draft from the template in `assets/<type>-template.md`.
4. Run `elegy-documentation check --project .` for objective
   validation: metadata, frontmatter, required headings, broken links.
5. Decide: update an existing ADR/spec when the change extends the same
   decision or behavior; create new when it establishes a distinct
   contract.

## Tool-call guardrails

### Classification (the decision)

- An ADR is for decisions with alternatives and lasting consequences.
  Do not create an ADR for a single-function rename or a config
  value change.
- A spec is for behavior contracts before or during implementation.
  Do not retro-author a spec for finished code unless you are
  capturing the as-built contract for future contributors.
- A note is for narrow context that does not justify a stronger
  artifact. Notes should not contain decisions that should be in
  ADRs.
- A roadmap is for staged future work across slices. Do not put a
  roadmap in a spec; specs describe what, roadmaps describe when.
- Do not create an ADR for naming cleanup that has no alternatives.
  The decision is a done thing, not a choice.

### Placement (where the file lives)

- `docs/adr/` for ADRs. `docs/specs/` for specs.
  `docs/guides/` for guides. `docs/notes/` for notes.
  `docs/roadmaps/` for roadmaps.
- Cross-repo decisions go in `elegy`, then link from affected repos.
  Do not duplicate shared doctrine into every consumer repo.
- Side-effect class: `read_only` (writing docs is a disk write, but
  this skill is doctrine, not the tool).
- Approval posture: `none` (this is guidance, not execution).

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

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| A new ADR was created for a one-line rename. | The author applied "durable decision" too broadly. | Convert to a note or delete. ADRs are for decisions with alternatives and lasting consequences. |
| A spec references a behavior that was removed two releases ago. | The spec was not updated when the behavior changed. | Edit the spec to match current behavior, or mark it superseded and create a new one. |
| `elegy-documentation check` fails on a valid-looking ADR. | The ADR frontmatter is missing `status`, `owner`, or a required heading. | Add the missing field. The check is objective only; it does not validate prose. |
| The same doctrine lives in two repos with conflicting wording. | Each repo independently copied the doctrine rather than linking. | Remove the copies, keep one authoritative source in Elegy, and add links from the consuming repos. |
| A spec was authored after the implementation shipped. | The author is retro-documenting an as-built contract. | This is fine if the spec captures the as-built contract for future contributors. Add a note in the header that the spec documents the shipped behavior, not the intended design. |
| An ADR was updated but the old version is still cited in three other ADRs. | The update did not propagate citation adjustments. | Search for references to the old ADR title/id and update or add a superseded note. |
| `docs/roadmaps/` fills up with abandoned planning artifacts. | Roadmaps were created for short-lived experiments. | Mark abandoned roadmaps with `status: abandoned` and a closing note. Do not delete — the record of the plan is useful even when the plan was cancelled. |

## Examples

### Example 1 — deciding between an ADR and a note

Prompt: "Rename `skill-definition-v2` to `skill` across the repo."

Classification: this is a naming cleanup with no alternatives — the
v2 suffix was noise, and there is no competing convention. It does
not warrant an ADR.

Action: a note in `docs/notes/skill-definition-rename.md` capturing
the rename scope and the files touched. Link from
`docs/architecture/skill-core-v1.md` which reflects the new naming.

### Example 2 — drafting a spec for a new capability

Prompt: "Add `elegy skill audit` that walks the audit checklist and
returns JSON findings."

Classification: this is a behavior contract for a new CLI
capability. It needs a spec.

Action: draft `docs/specs/skill-audit-cli.md` from
`assets/spec-template.md`. Describe the input (skill path or id,
format), the output (finding list with severity), and the acceptance
criteria (every checklist check produces a finding, Critical blocks
the build). Place it in Elegy since it is cross-project tooling.

## References

- `references/taxonomy.md`
- `references/placement.md`
- `references/adr-workflow.md`
- `references/spec-workflow.md`
- `references/adoption.md`
- `references/review-checklist.md`
- `references/examples.md`
- Companion skill: `elegy-skill-authoring` — same pattern for SKILL.md.

## Assets

- `assets/adr-template.md`
- `assets/spec-template.md`
- `assets/docs-index-template.md`
- `assets/repo-adoption-snippet.md`
- `assets/pr-checklist-snippet.md`
