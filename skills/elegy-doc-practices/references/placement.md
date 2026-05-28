# Placement Rules

## Central Elegy

Put documentation in `elegy` when it defines shared doctrine for multiple repos:

- documentation taxonomy
- ADR versus spec rules
- repo adoption patterns
- central review checklists
- cross-repo architectural decisions

## Owning Repo

Put documentation in the owning product repo when it describes:

- product-specific behavior
- local architecture decisions
- local contributor workflows
- local acceptance criteria
- local deployment or rollout details

## Cross-Repo Decisions

When a decision affects more than one repo:

1. record the shared doctrine or decision centrally in `elegy`
2. link to it from each affected repo
3. keep only local overrides and local consequences in each consumer repo

## Non-Goals

- Do not make `elegy` the home for every product ADR.
- Do not duplicate central doctrine into downstream repos.
- Do not force cross-repo blocking CI on subjective document quality.
