---
created: 2026-06-01
category: architecture
status: current
doc_kind: node
summary: Migration note recording that piloting authority, protocol, and execution ownership have moved from Elegy to the Holon Rust runtime.
tags: [architecture, piloting, migration, elegy, holon]
---

# Piloting Moved To Holon

Piloting authority, protocol, and execution ownership have moved from Elegy to the Holon Rust runtime.

## What Changed

Holon now owns piloting end to end:

- **Protocol DTOs**: `holon-protocol/src/pilot.rs` defines all piloting request/response shapes.
- **PilotAdapterRegistry**: `holon-runtime-core/src/piloting.rs` provides the in-memory registry and all core functions.
- **Native core plugin**: `holon.pilicing` is seeded at runtime initialization with 8 built-in tools.
- **Fail-closed execute**: Only `api` and `file_format` lanes are allowed for live pilot execution.
- **V2-core asset family**: `pilot-adapter` is accepted in `holon-plugin/v2-core` manifests.
- **Bundled plugin**: `piloting.blender.reference` demonstrates the reference adapter.

## What Was Removed From Elegy

The following files have been removed from Elegy as part of this migration:

- `rust/crates/elegy-contracts/src/piloting.rs`
- `contracts/schemas/piloting-*.schema.json` (12 files)
- `contracts/fixtures/piloting-*.minimal.json` (8 files)
- `contracts/fixtures/elegy-plugin-package-v2.piloting-blender.json`
- `contracts/fixtures/blender-piloting-changelog.md`
- `docs/architecture/elegy-piloting-foundation.md`

## What Remains In Elegy

Elegy retains:

- Portable piloting contract shapes and fixtures when portability is the goal (schemas, fixtures for other consumers).
- Memory, planning, skills, configuration, and reusable low-level observation substrate.
- No piloting authority, execution, readiness, or policy.

## Migration Reference

- Holon spec: `specs/holon-native-piloting/spec.md`
- Holon canon: `docs/system/architecture/plugin-packages.md`
- Holon planning: `docs/planning/holon-expert-piloting-platform/holon-and-elegy-split.md`
