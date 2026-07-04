---
title: Centralize documentation practices doctrine
status: accepted
date: 2026-05-25
owner: Elegy
---

# Centralize documentation practices doctrine

## Context

- Elegy needs one shared doctrine for deciding when changes require ADRs, specs,
  guides, notes, or roadmap updates.
- The audit direction for this feature is skill-first and CLI-second: centralize
  doctrine in Elegy, distribute it as a Codex-style skill, and keep local repos
  limited to small config overrides.
- Earlier architecture ideas risked turning this into a broad governance
  platform in v1, which would add scope without proving real value.

## Decision

- Keep the durable doctrine in the central `plugins/doc-practices/`
  package plus repo-visible architecture docs in `elegy`.
- Add a narrow `elegy-documentation ...` CLI only for deterministic file creation,
  objective validation, and docs index generation.
- Require downstream repos such as `holon` and `elegy-copilot` to adopt the
  same central doctrine with only local path, trigger, and exception overrides
  through `.elegy/docs.yaml`.
- Keep subjective quality review with humans instead of automated blocking.

## Alternatives

- Option A: build a broad documentation governance platform in v1.
  Rejected because it adds enforcement and orchestration scope before the core
  doctrine and deterministic checks are proven useful.
- Option B: keep documentation guidance fully local to each repo.
  Rejected because it causes doctrine drift across repos that should share the
  same ADR/spec classification and placement rules.

## Consequences

- Positive: shared documentation doctrine now has a single home and can be
  adopted by multiple repos without copy-pasting policy.
- Positive: the CLI stays objective and deterministic, which makes advisory CI
  and local validation safe to automate.
- Negative: the central skill package is not yet part of the governed skill
  registry, so discovery for this doctrine is repo-local rather than registry-driven.
- Negative: prose quality, correctness of reasoning, and architecture taste
  still require human review and cannot be delegated to automated checks.

## Links

- [Documentation practices architecture doc](../architecture/documentation-practices.md)
- [Documentation practices skill and CLI spec](../specs/documentation-practices-skill-and-cli.md)
