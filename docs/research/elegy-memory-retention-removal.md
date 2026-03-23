# Elegy-memory retention and removal guidance

This note is research-oriented guidance for contributors. It is not the canonical source for shipped behavior.

Canonical implemented truth stays in:

- `contracts/`
- `governance/`
- [docs/architecture/elegy-memory-v1.md](../architecture/elegy-memory-v1.md)

## Why retain separate supersede and tombstone actions

The split exists because the local store has two different jobs:

- preserve local lineage when one governed artifact copy locally replaces another
- preserve local withdrawal intent when an artifact copy should no longer appear in the default active view

If both cases collapsed into one action, the local store would lose either the successor relationship or the explicit local withdrawal reason.

## When to supersede

Use `supersede` when all of these are true:

- a successor local record already exists
- you want default active-only browsing to prefer the successor record
- you still want local lineage to show what replaced the older copy

Do not interpret a supersede action as a runtime promotion or host approval decision.

## When to tombstone

Use `tombstone` when any of these are true:

- the local copy should be withdrawn without declaring a successor
- a local withdrawal reason matters for later inspection
- you want the record hidden from default active-only browsing while keeping the artifact history inspectable

Do not interpret a tombstone action as runtime invalidation. A downstream host can still make its own authority decision from its own policy surface.

## Why this does not transfer authority from SAASTools

The local `Elegy` store is intentionally narrower than runtime memory policy in `SAASTools`.

`SAASTools` still owns:

- currentness
- approval
- freshness policy
- retrieval ranking
- runtime validation
- promotion and invalidation

That split keeps portable governed artifacts reusable while preventing repo-local artifact lineage from becoming accidental runtime truth.

## Why the rendered SKILL.md stays non-authoritative

Markdown is useful for contributor routing, but it is not a safe authority format for compatibility and projection rules.

Keeping the chain as governed definition -> governed discovery projection -> rendered markdown preserves:

- a machine-readable source of truth
- a derived discovery surface that can be validated deterministically
- a local contributor affordance that can change wording without quietly redefining runtime authority