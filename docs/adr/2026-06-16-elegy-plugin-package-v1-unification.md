---
title: Elegy Plugin Package V1 Unification
status: accepted
date: 2026-06-16
owner: Elegy
supersedes: specs/neutral-package-consolidation/spec.md
---

# Elegy Plugin Package V1 Unification

## Context

The Elegy plugin package contract started as two parallel schema families:
`elegy-plugin-package/v1` (basic identity, skill definitions, capability
projections, docs) and `elegy-plugin-package/v2` (superset adding
configuration, tool requirements, host compatibility, publishing,
adapter contracts, eval/resource packs, piloting, fixtures, and helpers).

This split was unnecessary. The "V2" components were always optional, and
the V1 shape was an arbitrary subset, not a stable minimum. Maintaining
two parallel schemas with a merge-on-import path added cognitive load
for no practical gain.

Additionally, V2 carried Holon-specific enums (`marketplaceTarget:
"holon"`) and concepts that no longer applied after piloting authority
moved to the Holon Rust runtime.

## Decision

**Unify to a single `elegy-plugin-package/v1` schema.** The schema file
at `contracts/schemas/elegy-plugin-package.schema.json` carries all
useful capabilities and uses `schemaVersion: "elegy-plugin-package/v1"`
as the const version string. No V1/V2 split, no parallel schema files.

The unified schema:
- Removes Holon-specific enum values.
- Removes V1/V2 split and the now-deleted `elegy-plugin-package-v1.schema.json`
  and `elegy-plugin-package-v2.schema.json` files.
- Keeps all component arrays that are actually used by any fixture.
- Adds `id` pattern constraints for AI-agent readability.

## Consequences (good)

- One schema file, one const version string, one truth.
- AI agents and human developers read exactly one package contract.
- Removing unused component arrays (capabilityContracts, evalPacks,
  resourcePacks, toolAdapterContracts, bridgeAdapterContracts, cliHelpers,
  assets, pilotingAdapters, fixturePacks, hostCompatibility) simplifies
  the schema from 19 to 9 component arrays.

## Consequences (bad)

- The unified schema is no longer a strict superset of the old V1 + V2.
  Some V2 component arrays were removed entirely because no fixture ever
  used them. A consumer that relied on those arrays would need to add
  them back via a future schema revision.
- Hosts that referenced the old V2-specific schema filename
  (`elegy-plugin-package-v2.schema.json`) need to update their references.

## Related

- [Architecture: Plugin Package Model](../architecture/elegy-plugin-package-model.md)
- [Spec: Plugin Tool Availability](../specs/plugin-tool-availability.md)
- [Spec: Neutral Package Consolidation (historical)](../../specs/neutral-package-consolidation/spec.md)
- [Schema: elegy-plugin-package.schema.json](../../contracts/schemas/elegy-plugin-package.schema.json)
