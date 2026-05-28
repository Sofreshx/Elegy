---
applyTo: "docs/**,.elegy/docs.yaml,skills/elegy-doc-practices/**"
---

# Elegy Documentation Practices Instructions

## Use The Shared Doctrine

- Use `skills/elegy-doc-practices/` as the central doctrine for ADR/spec classification and placement.
- Keep product-specific ADRs and specs in the owning repo unless the decision is cross-repo.
- Prefer updating an existing ADR or spec when the same decision or behavior slice is being extended.

## Objective Validation Boundary

- Use `elegy docs check` for metadata, status, filename, heading, and internal-link checks.
- Do not claim that the CLI proves prose quality or architectural correctness.
- Do not add automatic blocking logic for subjective doc quality.

## Local Config

- Use `.elegy/docs.yaml` only for repo-local path, trigger, and exception overrides.
- Keep config repo-relative and minimal.
