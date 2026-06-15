# Agent Skill Bridge Mirrors

## Purpose

This document defines the current repo-local mirror policy for rendered `SKILL.md`
outputs.

The goal is to support host-facing skill loading and contributor-facing routing
without promoting either markdown lane into an authority root.

## Authority chain

The authority chain remains one-way:

1. Governed skill definitions under `contracts/fixtures/skill.*.json` are the discovery authority.
2. Governed discovery projections under `contracts/fixtures/skill-discovery-index.*.json` are derived contract outputs over those definitions.
3. Repo-local `SKILL.md` files are rendered mirrors only.

Neither `.agents/skills/**` nor `.github/skills/**` becomes an authority root.

## Current mirror lanes

The repo currently keeps two repo-local rendered markdown lanes for dedicated
skill surfaces:

- `.agents/skills/<skill-id>/SKILL.md` is the repo-local host-facing derived mirror referenced by governed discovery `vaultRef` entries.
- `.github/skills/<skill-id>/SKILL.md` is the repo-local contributor-routing derived mirror used for repository navigation and contributor guidance.

These lanes should stay byte-for-byte aligned for the same skill when they are
intended to describe the same dedicated surface.

Wrapper-local bridges under `src/Elegy-*/skills/**` remain separate derived
outputs for packaged wrapper consumption. They are not replaced by either
repo-local mirror lane.

## Why `.agents/skills`

Elegy already models `vaultRef` as a derived content location rather than as a
governed authority path. Using `.agents/skills/**` for the governed discovery
projection gives the repo one explicit host-facing mirror path without changing
the authority roots under `contracts/`.

This keeps `.github/skills/**` free to remain contributor-facing repo guidance
instead of pretending that GitHub-oriented layout is the portable host contract.

## Validation posture

The mirror policy is:

- governed fixtures may reference `.agents/skills/**` through `vaultRef`
- `.agents/skills/**` and `.github/skills/**` stay non-authoritative
- canonical-output validation must fail when the paired mirror files drift

This is intentionally a repository validation rule, not a portability promise
for the contracts bundle. The contracts bundle continues to ship governed JSON,
not repo-local markdown mirrors.

## Non-goals

- Do not move skill authority out of governed fixtures.
- Do not make wrapper-local `skills/<surface>/SKILL.md` files the discovery authority.
- Do not treat Codex plugin files, `.mcp.json`, `.app.json`, or future package outputs as authored truth.
- Do not infer host install, approval, auth, trust, or connector state from any `SKILL.md` mirror.
