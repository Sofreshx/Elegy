Add the following guidance to the consuming repo's `AGENTS.md` or equivalent contributor instructions:

```md
- Use the central `elegy-doc-practices` skill for ADR/spec classification, placement, and review.
- Keep only repo-local path and trigger overrides in `.elegy/docs.yaml`.
- Run `elegy docs check` for objective docs validation when ADRs or specs change.
```
