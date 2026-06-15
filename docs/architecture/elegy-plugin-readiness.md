# Elegy Plugin Readiness

## Purpose

This document defines how Elegy packages prepare governed plugin artifacts for
a future Holon marketplace without turning Elegy into a marketplace or runtime
authority.

Piloting authority, protocol, and execution ownership have moved to the Holon
Rust runtime. See [piloting-moved-to-holon.md](piloting-moved-to-holon.md).

## Package Source Layout

A plugin package is expected to come from a normal Git repository source tree.

The package should contain governed assets such as:

- package metadata under `elegy-plugin-package/v2`
- schemas and fixtures rooted in `contracts/`
- adapter manifests and fixture packs referenced by package components
- supporting docs under `docs/`
- optional helper CLI references when the helper performs validation or
  packaging only

Package component paths resolve relative to the package file itself.

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

Elegy can ship these package asset families:

- `capability-contract`
- `skill`
- `eval-pack`
- `resource-pack`
- `tool-adapter` contract
- optional CLI helper

The package can also reference package-local JSON files through `manifestRef`
and `fixturePackRef`.

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
- no piloting authority (now owned by Holon runtime)

The package is the portable source artifact. Holon decides whether and how to
accept, trust, install, approve, and execute it.

## Validation Posture

In this slice, package validation checks only:

- contract and schema shape
- target and surface declaration completeness
- side-effect declaration completeness
- fixture presence and adapter alignment
- Holon publishing metadata completeness
- absence of live actuation fields in a contracts-only package

Live piloting proof is now owned by the Holon runtime.
