---
title: Retire the central skill registry and resolver
status: accepted
owner: elegy-core
doc_kind: adr
date: 2026-07-20
---

# Retire the central skill registry and resolver

## Context

Elegy plugins own and distribute their Agent Skills. The retired
`elegy-skills` CLI instead compiled a fixed list of repo skills into its binary
and exposed list, search, resolve, get, and validation commands over that
snapshot.

That model could not accurately describe the plugins installed in a particular
host. It could omit separately distributed plugins, advertise unavailable
skills, and drift from the version or projection actually active in the host.
Host-native skill discovery already reads each installed plugin's own skills,
making the central resolver both redundant and less authoritative.

## Decision

Remove the `elegy-skills` Rust crate, binary, plugin package, marketplace entry,
and release surface.

Agent Skills remain co-located with and governed by their owning plugin.
Standalone skill-only packages remain valid plugins. Plugin manifests declare
their skill directories, plugin verification validates the package boundary,
and each host discovers and routes the plugins installed or projected into that
host.

Elegy will not maintain a global skill search index, cross-plugin ranking
algorithm, or embedded catalog of repo skills.

## Consequences

- Skill availability is truthful to the installed host rather than a compiled
  Elegy snapshot.
- Plugin authors own their skill wording, triggers, references, and validation.
- `elegy-plugin-packaging verify --plugin <plugin-root>` is the package-level
  validation path.
- `elegy-skill-authoring` remains the contributor workflow for creating and
  auditing `SKILL.md` content.
- Hosts that want search or ranking may implement it over their own installed
  skill set; that behavior is host-owned and is not an Elegy contract.
- Consumers must not depend on the retired `elegy-skills` executable or its JSON
  output envelopes.

## Rejected alternative

Keeping the CLI as contributor-only tooling was rejected because the same
embedded-catalog limitation would remain, while package verification and direct
inspection already cover the durable validation need.
