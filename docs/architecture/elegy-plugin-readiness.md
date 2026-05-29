# Elegy Plugin Readiness

## Purpose

This document defines how Elegy packages prepare targeted piloting plugins for a
future Holon marketplace without turning Elegy into a marketplace or runtime
authority.

## Package Source Layout

A piloting package is expected to come from a normal Git repository source tree.

The package should contain governed assets such as:

- package metadata under `elegy-plugin-package/v2`
- piloting schemas and fixtures rooted in `contracts/`
- adapter manifests and fixture packs referenced by package components
- supporting docs under `docs/`
- optional helper CLI references when the helper performs validation or
  packaging only

Package component paths resolve relative to the package file itself. In the
current governed Blender example, the package lives under `contracts/fixtures/`,
so package-local assets stay beside that file and adapter-manifest schema refs
can point to sibling governed schemas under `../schemas/`.

## Required Manifest Metadata

For Holon-oriented publishing, the package must carry source and provenance
metadata through `publishing` fields on `elegy-plugin-package/v2`.

Required fields for the Holon target:

- `marketplaceTarget: "holon"`
- `importMode: "package"`
- `sourceRepository`
- `sourceRef`
- `sourceCommit`
- package `metadata.license`
- `changelogRef`
- `provenanceRef`
- at least one `signatureRefs` entry
- compatibility metadata describing supported Holon ranges, including at least
  one `compatibility.host: "holon"` entry

This keeps the package portable while still giving Holon enough evidence to make
its own future trust and install decisions.

## Asset Kinds

The first piloting slice allows Elegy to ship these package asset families:

- `capability-contract`
- `skill`
- `eval-pack`
- `resource-pack`
- `tool-adapter` contract
- `bridge-adapter` contract
- optional CLI helper

The package can also include inline piloting adapter manifests and fixture packs
through the governed `pilotingAdapters` and `fixturePacks` component lanes, or
reference package-local JSON files through `manifestRef` and `fixturePackRef`.

Fixture packs now also carry typed `policyDecisions`, `simulationResults`,
`replayCheckpoints`, and `lifecycleEvents` so a downstream host can inspect the
bounded pre-execution evidence model without treating Elegy as the execution
authority.

## Non-Goals

The package is not allowed to become a one-off Holon import path for individual
Elegy features.

That means:

- no per-feature ad hoc import lane
- no embedded trust decisions
- no embedded approvals
- no embedded live runtime sessions
- no bridge ownership
- no host-local secrets or lease state

The package is the portable source artifact. Holon decides whether and how to
accept, trust, install, approve, and execute it.

## Validation Posture

In this slice, package validation checks only:

- contract and schema shape
- target and surface declaration completeness
- side-effect declaration completeness
- fixture presence and adapter alignment
- policy-decision, simulation, replay-checkpoint, and lifecycle-event alignment
- Holon publishing metadata completeness
- absence of live actuation fields in a contracts-only package

Live piloting proof remains a later targeted-plugin phase.
