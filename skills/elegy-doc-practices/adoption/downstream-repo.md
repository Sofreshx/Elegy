# Adoption: minimal `.elegy/docs.yaml` for a downstream repo

Copy this file to the consuming repo at `.elegy/docs.yaml`. Adjust the paths
and triggers to match the consuming repo's docs layout.

```yaml
schemaVersion: elegy-documentation/v2
authorityRoots:
  current:
    - docs/adr
    - docs/specs
  planning: []
  research: []
  generated: []
entrypoints:
  - README.md
derivedSurfaces:
  sidebars: []
  manifests: []
  llms: []
  bundles:
    - docs/docs-index.md
requiredFrontmatter:
  - title
  - status
  - owner
freshnessWarnings:
  currentDays: 120
  planningDays: 45
  researchDays: 90
localExceptions: []
```

## GitHub Copilot instruction snippet

```markdown
---
applyTo: "docs/**,.elegy/docs.yaml"
---

# Documentation Practices

- Use the central `elegy-doc-practices` doctrine from Elegy.
- Classify new work as ADR, spec, guide, note, or roadmap.
- Run `elegy-documentation check --project . --json` for objective validation.
- Do not block on subjective doc quality.
```

## CI workflow snippet

```yaml
- name: Validate documentation
  run: |
    cargo install elegy-documentation --locked
    elegy-documentation check --project . --json
```

## Phase guidance

- **Phase 1** — start with a PR checklist that asks contributors to confirm
  documentation impact.
- **Phase 2** — add `elegy-documentation check` as a non-blocking CI job.
- **Phase 3** — promote to blocking CI only for objective failures on
  high-impact paths (ADRs, specs).
