---
applyTo: "docs/**,.elegy/docs.yaml,contracts/fixtures/skill.elegy-doc-practices.json,contracts/fixtures/elegy-plugin-package.elegy-doc-practices.json,skills/elegy-doc-practices/**"
---

# Elegy Documentation Practices Instructions

## Use The Shared Doctrine

- Use `skills/elegy-doc-practices/` as the central doctrine for ADR/spec classification and placement. The package is shipped as `contracts/fixtures/elegy-plugin-package.elegy-doc-practices.json` and exposed through `skill.elegy-doc-practices.json`.
- Keep product-specific ADRs and specs in the owning repo unless the decision is cross-repo.
- Prefer updating an existing ADR or spec when the same decision or behavior slice is being extended.
- In this repo, current ADR/spec authority roots are `docs/adr/` and `docs/specs/`, configured by `.elegy/docs.yaml`.

## Objective Validation Boundary

- Use `elegy-documentation inspect/map/check --project . --json` for authority-aware inspection, corpus mapping, and objective validation.
- Use umbrella `elegy docs ...` as the current compatibility path for ADR/spec scaffolding and docs index behavior.
- Do not claim that the CLI proves prose quality or architectural correctness.
- Do not add automatic blocking logic for subjective doc quality.
- If generated docs indexes or bundles are affected, inspect the generated output or documented drift status.

## Local Config

- Use `.elegy/docs.yaml` only for repo-local path, trigger, and exception overrides.
- Keep config repo-relative and minimal. The V2 schema is preferred; the V1-compat loader remains supported.

## Plugin Package Reference

- The doc-practices plugin package is a portable example of the `elegy-plugin-package/v1` contract applied to a central doctrine. It can be verified with `elegy plugin verify --package contracts/fixtures/elegy-plugin-package.elegy-doc-practices.json --json`.
