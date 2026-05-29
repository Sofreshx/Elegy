# Elegy Piloting Foundation

## Purpose

This document defines the first Elegy piloting slice as a portable foundation for
targeted software-control plugins, not as a general desktop-control runtime.

The foundation is intentionally contracts first.

Elegy owns:

- portable observation, action, and readiness contracts
- adapter authoring SDK conventions at the package and contract layer
- fixture-pack and replay conventions for targeted validation
- Holon-compatible plugin packaging source metadata
- schema and CLI validation for those governed contracts

Elegy does not own:

- marketplace trust or install state
- approval workflows
- leases or bridge-session ownership
- live runtime authority over arbitrary software
- mouse, keyboard, browser, process, or UI automation in this slice
- evidence collection policy after a host imports a package

Those concerns stay in downstream hosts such as Holon.

## Boundary

`elegy-piloting` is a package-and-contract foundation.

It is not a finished runtime product surface. In this first slice, the umbrella
CLI only validates governed manifests, fixtures, and Holon-oriented package
metadata:

```bash
elegy piloting validate-adapter <path>
elegy piloting validate-fixtures <path>
elegy piloting package --target holon <path>
```

The commands validate schemas and contracts only. They do not actuate desktop,
browser, UIA, mouse, keyboard, or process-control lanes.

## Contract Set

The first governed piloting family is aligned with common AI tool practice:

- `TargetDescriptor` for target application identity, version range, platform,
  and launch or attach hints
- `SurfaceDescriptor` for targeted UI, API, browser, or desktop surfaces plus
  selectors or semantic anchors
- `ObservationFrame` for timestamped observed state with redaction, source,
  confidence, and evidence references
- `ActionIntent` for abstract requested operations plus input schema,
  side-effect class, and confirmation posture
- `ActionResult` for structured success, failure, refusal, or retry outcomes
- `ReadinessReport` for dependency status, blocked reasons, and drift posture
- `AdapterManifest` for targeted plugin metadata, contracts, fixtures, and
  allowed side-effect classes
- `FixturePack` for recorded observations, allowed actions, and expected result
  checks
- `PolicyDecision` for typed allow, deny, simulate, or escalate decisions over
  declared side-effect classes
- `SimulationResult` for dry-run or predicted outcomes without live actuation
- `ReplayCheckpoint` for before and predicted-after state references used in
  bounded replay review
- `LifecycleEvent` for typed intent, policy, simulation, checkpoint, and result
  event sequencing

These contracts are rooted in `contracts/schemas/` and shipped through the
standard exported contracts bundle.

The current piloting slice now validates these prerequisite policy and replay
artifacts through fixture packs, but it still does not execute desktop or OS
actions.

## Packaging Role

Piloting packages reuse `elegy-plugin-package/v2` rather than inventing a second
package family.

That package now carries piloting-oriented components such as:

- `capabilityContracts`
- `evalPacks`
- `resourcePacks`
- `toolAdapterContracts`
- `bridgeAdapterContracts`
- `pilotingAdapters`
- `fixturePacks`
- `cliHelpers`
- Holon-oriented `publishing` metadata

This keeps portable package truth in the existing governed package surface while
avoiding a parallel plugin-runtime contract.

## Host Split

Holon remains the downstream runtime host for:

- marketplace install and trust
- approvals
- leases
- bridge sessions
- execution
- host evidence handling

Elegy remains reusable for non-Holon consumers because the piloting contracts are
portable and host-neutral. Holon compatibility is a package-target convention,
not a repo-center takeover.

## Examples

The first governed example in this slice is intentionally targeted and
non-general:

- `blender.piloting`

Future examples such as Excel- or browser-specific adapters should only be added
once their governed schemas, fixtures, and package examples ship together.

Each example must declare exact target software and surfaces. None should claim
general desktop control.
