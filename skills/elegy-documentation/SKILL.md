---
name: elegy-documentation
description: "Use when a repo needs authority-aware documentation inspection, mapping, objective validation, or non-authoritative llms/bundle exports through the dedicated elegy-documentation CLI."
---

# Elegy Documentation

Use this skill when you need a deterministic tool lane for documentation authority mapping and objective validation rather than prose guidance alone.

## Workflow

1. Inspect the repo's documentation roots and entrypoints.
2. Map documents into current, planning, research, generated, or other classes.
3. Run objective validation only: metadata, dates, statuses, internal links, freshness warnings, and export drift.
4. Export non-authoritative `llms` or bundle outputs only when a derived handoff artifact is needed.

## Use This With

- `skills/elegy-doc-practices/` for doctrine, document-type choice, placement, and templates.
- `elegy-documentation inspect --project <path>` when you need authority-aware repo docs posture.
- `elegy-documentation map --project <path>` when you need a corpus map and reading order.
- `elegy-documentation check --project <path>` when you need objective validation findings.
- `elegy-documentation export llms|bundle --project <path> --output <path>` when a derived handoff output is needed.

## Boundaries

- Source documents remain authoritative.
- `.elegy/docs.yaml` governs repo-local documentation classification and checks.
- Generated `llms` files and documentation bundles are derived outputs only.
- This skill does not score prose quality, rewrite docs, create embeddings, or claim architectural correctness.

## Related Doctrine

- `../elegy-doc-practices/SKILL.md`
- `../../docs/architecture/documentation-practices.md`
- `../../docs/agent-integration.md`
