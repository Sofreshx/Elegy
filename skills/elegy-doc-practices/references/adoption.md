# Adoption Guidance

## Goal

Adopt one shared documentation doctrine across repos while keeping only local path and policy differences in each consumer repo.

## Minimal Adoption Package

1. Reference this central skill from repo guidance.
2. Add `.elegy/docs.yaml` with local doc paths and triggers.
3. Add a PR checklist item for docs impact.
4. Run `elegy docs check` in advisory CI first.

## Local Overrides

Use local config only for:

- ADR path
- spec path
- docs index path
- required doc triggers
- narrow local exceptions

Do not fork the doctrine unless the central rules are truly wrong for all adopters.

## Holon And Elegy-Copilot

Both should consume the same central skill and references.

They should only diverge in repo-local paths, triggers, and exceptions.
