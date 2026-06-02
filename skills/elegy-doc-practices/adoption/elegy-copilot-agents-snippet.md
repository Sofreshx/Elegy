# Elegy-Copilot Adoption Snippet

Add this to `elegy-copilot` repo guidance:

```md
- Use the shared `elegy-doc-practices` skill instead of copying doctrine locally.
- Use `elegy-planning` for durable planning state (goals, roadmaps, plans, todos, issues, review points) and `elegy-skills-discovery` for governed skill catalog lookups.
- Use `elegy-obsidian` only for read/write/search against a local Obsidian vault; it is a non-authoritative mirror and must not shadow `elegy-planning` state.
- Keep only path and trigger overrides in `.elegy/docs.yaml`.
- Cross-repo decisions belong in Elegy and should be linked locally.
```
