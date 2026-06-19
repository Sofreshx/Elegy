# Elegy Plugin Readiness

## Purpose

This document defines how Elegy plugin packages prepare governed plugin
artifacts for host consumption — including Holon, OpenCode, Codex, and other
LLM agent hosts — without turning Elegy into a marketplace, runtime authority,
or host policy engine.

The primary plugin package model is defined in
[elegy-plugin-package-model.md](elegy-plugin-package-model.md). This document
focuses on the readiness receipt, host consumption posture, and the contract
boundary between the portable package and the consuming host.

Piloting authority, protocol, and execution ownership have moved to the Holon
Rust runtime. See [piloting-moved-to-holon.md](piloting-moved-to-holon.md).

## V1 Unification

The `elegy-plugin-package/v1` schema is the **single** portable package
contract. There is no V1/V2 split, no parallel schema files, and no
Holon-specific enum values on the contract surface.

This contract is the post-unification state: a single portable package
contract with no V1/V2 split and no Holon-specific enum values on the
contract surface.

That means a publishable package carries:

- one schema version string: `elegy-plugin-package/v1`
- one schema file: `contracts/schemas/elegy-plugin-package.schema.json`
- component arrays that are actually used by any fixture (skill definitions,
  instruction skills, capability projections, configuration templates,
  configuration profiles, docs, tool requirements)
- optional `publishing` and `extensions` blocks

Holon-specific metadata, when required for Holon publishing, lives in the
optional `publishing` block, **not** in required component fields. Holon is a
consumer of portable packages, not an authority over their shape.

## Package Source Layout

A plugin package is expected to come from a normal Git repository source
tree. The package should contain governed assets such as:

- package metadata under `elegy-plugin-package/v1`
- schemas and fixtures rooted in `contracts/`
- skill definitions and capability projections referenced by package
  components
- supporting docs under `docs/`
- optional helper CLI references when the helper performs validation or
  packaging only

Package component paths resolve relative to the package file itself.

## Readiness Receipt

`elegy plugin verify` and `elegy plugin install-check` both produce a
readiness receipt governed by
`contracts/schemas/elegy-plugin-readiness-v1.schema.json`. The receipt is
the machine-readable answer to "what can this package actually do on this
host right now?"

The receipt carries:

- the package identity and contract bundle version
- referenced skill definitions and capability projections
- side-effect class declarations and subset posture
- tool requirements and probe results
- a verdict: `ready` | `partial` | `blocked`

`elegy plugin verify` checks package consistency. `elegy plugin install-check`
checks declared tool requirements against an install receipt and optional
binary probes.

## Required Manifest Metadata

Publishable packages must carry `publishing` metadata for provenance:

- `sourceRepository`
- `sourceRef`
- `sourceCommit`
- `metadata.license`
- `changelogRef`
- `provenanceRef`
- at least one `signatureRefs` entry
- `compatibility[]` describing supported host ranges (Holon, Codex, etc.)

`marketplaceTarget` and `importMode` are **optional** publishable hints, not
required schema fields. Hosts that need them (for example, the Holon
marketplace) declare them in their consumer-side policy, not in the portable
contract.

This keeps the package portable while still giving each host enough evidence
to make its own trust, install, and approval decisions.

## Asset Kinds

Elegy ships these package asset families:

- `capability-contract`
- `skill`

The package can also reference package-local JSON files through
`manifestRef`.

## Non-Goals

The package is not allowed to become a one-off import path for individual
features of any specific host.

That means:

- no per-feature ad hoc import lane
- no embedded trust decisions
- no embedded approvals
- no embedded live runtime sessions
- no bridge ownership
- no host-local secrets or lease state
- no piloting authority (now owned by Holon runtime)
- no marketplace-specific enum values on required fields

The package is the portable source artifact. The consuming host decides
whether and how to accept, trust, install, approve, and execute it.

## Validation Posture

In this slice, package validation checks only:

- contract and schema shape
- target and surface declaration completeness
- side-effect declaration completeness
- fixture presence and adapter alignment
- publishing metadata completeness
- absence of live actuation fields in a contracts-only package

Live piloting proof is owned by the Holon runtime, not the portable package.
