---
spec_id: plugin-tool-availability
title: Plugin Tool Availability
status: draft
type: contract
owner: Elegy
created: 2026-06-04
updated: 2026-06-04
doc_kind: spec
summary: Contract for how elegy-plugin-package/v2 capability projections are verified against skill hostProjection, how the resulting tool availability is projected for hosts, and how the elegy-planning pilot package proves the rules. Defines the verify-only posture, the readiness receipt shape, and the Codex projection conservatism rules.
---

# Plugin Tool Availability

## Problem

Hosts need a deterministic, machine-readable answer to "what can this plugin
package actually do on disk right now?" without Elegy becoming a host,
marketplace, or runtime authority. Today there is no single contract that
covers tool availability, verify-only projection, and readiness receipts.

## Goals

This spec pins down a single, host-facing concept — **tool availability** — and
the verify-only flow that produces it. The goal is to make it cheap and
deterministic for any host (Elegy-Copilot, Holon, OpenCode, Codex) to ask "what
can this plugin package actually do on disk right now?" and get a machine-readable
answer, without Elegy becoming a host, a marketplace, or a runtime authority.

Three durable definitions and three durable rules cover that goal:

- **Plugin package** is the portable bundle. `elegy-plugin-package/v2` is the
  authority.
- **Skill definition v2** is the capability contract. `skill` is
  the authority, and `hostProjection` is the part a host cares about.
- **App/connector** is host-owned. It is not an Elegy package-owned runtime and
  never appears in a portable package as an executable contract.
- **Tool availability** is the verifiable projection of a package's capabilities
  onto the CLI/MCP/runtime surfaces that are actually installed and runnable on
  the host.
- **Readiness receipt** is the machine-readable answer. It reports identity,
  verified skills, projected tools, missing binaries, side-effect classes, and
  one of `ready` | `partial` | `blocked`.

The verify-only posture is the trust boundary: Elegy verifies and projects; the
host owns install, auth, approvals, and runtime enablement.

## Context Evidence

- `contracts/schemas/elegy-plugin-package-v1.schema.json` and
  `contracts/schemas/elegy-plugin-package-v2.schema.json` define the portable
  package contract. v2 adds configuration components on top of v1, but
  `components.capabilityProjections` already exists in v1.
- `contracts/schemas/skill.schema.json` defines the unified skill
  contract. `hostProjection` (lines 181-232) carries per-capability function
  names and side-effect overrides that are the contract authority for
  function-calling surfaces.
- `contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json` is the pilot
  package. Its `components.capabilityProjections` (13 entries, lines 29-186)
  must remain consistent with
  `contracts/fixtures/skill.elegy-planning.json`'s
  `hostProjection.capabilityProjections`.
- `contracts/fixtures/skill.elegy-planning.json` (lines 1928+)
  carries `hostProjection.cliName: "elegy-planning"`, `outputContractId:
  "elegy-planning-v1"`, and `defaultSideEffectClass: "disk_write"` — these
  are the install-receipt-resolvable identifiers the verifier must use.
- `docs/architecture/elegy-plugin-readiness.md` already declares the
  contracts-only posture: "Elegy packages prepare governed plugin artifacts
  for a future Holon marketplace without turning Elegy into a marketplace or
  runtime authority" and "The package is the portable source artifact. Holon
  decides whether and how to accept, trust, install, approve, and execute it."
- `docs/architecture/codex-plugin-projection.md` defines the current
  conservative Codex projection slice (`.codex-plugin/plugin.json` and
  `skills/`), and explicitly states `.mcp.json`, `.app.json`, `hooks/hooks.json`,
  and marketplace metadata are not generated yet. The rules in this spec keep
  that posture honest.
- `scripts/install-distribution.ps1` (lines 880-923) writes an
  `install-receipt.json` after extracting the contracts bundle, CLI assets, and
  wrapper assets. The receipt is the on-disk proof the verifier must read for
  installed-binary resolution.
- `docs/distribution.md` (line 103) describes that receipt as carrying the
  request, source, host target, installed assets, and verification evidence —
  the same facts the readiness receipt summarizes for hosts.
- `docs/architecture/elegy-plugin-readiness.md` and
  `docs/architecture/piloting-moved-to-holon.md` make install/auth/runtime
  ownership explicit. Piloting authority is now Holon's; this spec does not
  move that line.

## Behavior

### R1. Definitions

The spec fixes the following terms for every later reference inside Elegy
contracts, fixtures, CLI output, and codex projections.

- **Plugin package** — A portable bundle whose identity, components, and
  publishing metadata are declared under `elegy-plugin-package/v2`. The
  package carries metadata, schemas, fixtures, and references; it does not
  own install, auth, approval, or runtime.
- **Tool availability** — The verifiable projection of a package's
  capabilities onto the CLI/MCP/runtime surfaces that are actually installed
  and runnable for a given host. It is a property of `(package, install
  receipt)`, not a property of the package in isolation. The same package
  yields different availability against different install receipts.
- **App/connector** — A host-owned integration. It binds an external
  service, account, or workspace to a host, owns auth and refresh, and lives
  entirely outside the portable package. A portable package MAY reference
  apps conceptually (for example, to hint at a required connector), but it
  MUST NOT carry app launch, auth, secrets, lease state, or runtime sessions.
- **Capability projection** — Per `elegy-plugin-package/v2`, an entry in
  `components.capabilityProjections[]` that re-states one skill capability in
  host-projection terms (lane, function name, MCP tool name, side-effect
  class, dry-run support).
- **Host projection** — Per `skill`, the `hostProjection` block
  on a skill definition. It carries `cliName`, `outputContractId`,
  `defaultSideEffectClass`, and `capabilityProjections[]` of
  `(capabilityId, functionName, sideEffectClass, isDeterministic)`.
- **Install receipt** — A JSON document written to the install destination
  root by the Elegy generic installer. It records the request, source, host
  target, installed assets, and verification evidence. The readiness receipt
  consumes this document for binary resolution.
- **Readiness receipt** — The machine-readable answer emitted by the
  verification flow. It carries package identity, verified skills, projected
  tools, missing binaries, side-effect classes, host-policy hints, and one
  status of `ready` | `partial` | `blocked`.

### R2. Package consistency rules for `components.capabilityProjections`

`elegy-plugin-package/v2.components.capabilityProjections` MUST be derivable
from one of two declared sources. A package that violates this rule fails
contract validation with a precise finding, not a generic schema error.

**R2.1 Derived source.** Every entry in `components.capabilityProjections[]`
MUST be derivable, by id, from a `hostProjection.capabilityProjections[]`
entry on a skill definition referenced through
`components.skillDefinitions[].definitionRef` (or inline `definition`).

The derivation is per-entry and per-field:

| Package field | Source field on the skill definition |
|---|---|
| `capabilityProjections[].id` | `<skill-namespace>.<skill-name>-<capability-id>-<lane-suffix>` — must be unique and stable across the package |
| `capabilityProjections[].skill` | `identity.namespace + "." + identity.name` of the referenced skill |
| `capabilityProjections[].capability` | `hostProjection.capabilityProjections[].capabilityId` (which MUST also exist in `capabilities[].id`) |
| `capabilityProjections[].projection.functionName` | `hostProjection.capabilityProjections[].functionName` |
| `capabilityProjections[].sideEffectClass` | `hostProjection.capabilityProjections[].sideEffectClass` if set, else `hostProjection.defaultSideEffectClass` |
| `capabilityProjections[].lane` | MUST be `subprocess` when `implementation.executionType` is `subprocess`; MUST be `mcp` when `executionType` is `mcp`; otherwise `api` or `plugin` is allowed only if the underlying surface supports it |
| `capabilityProjections[].supportsDryRun` | `true` only when the capability's `execution.hasSideEffects` is `false` OR the skill explicitly declares a dry-run path. The package MUST NOT advertise `supportsDryRun: true` for a side-effecting capability without a host-resolvable dry-run path |

**R2.2 Deliberate subset source.** A package MAY carry a `capabilityProjections[]`
that omits capabilities present in the referenced skill. When it does, the
package MUST set `metadata.subsetOf: { skill: "<namespace>.<name>", version:
"<version>", omitted: ["<capability-id>", ...], reason: "<non-empty>" }` at
the package level. A subset without the marker is a contract violation. The
marker is intentionally at package level, not per-entry, so reviewers see the
whole omission set in one place.

**R2.3 Inverse rule.** Every `hostProjection.capabilityProjections[].capabilityId`
on a referenced skill MUST either appear in `components.capabilityProjections[]`
or be listed under `metadata.subsetOf.omitted`. Otherwise the package
under-reports its capabilities and the contract fails.

**R2.4 No phantom projections.** A `components.capabilityProjections[]` entry
whose `capability` does not resolve to a real `capabilities[].id` on any
referenced skill fails with `PROJECTION-CAPABILITY-UNKNOWN`. A projection
whose `skill` does not resolve to a referenced skill identity fails with
`PROJECTION-SKILL-UNKNOWN`.

**R2.5 Side-effect class override is allowed only downward.** A
package-level projection MAY tighten a side-effect class only when the
underlying capability is verifiably read-only. It MUST NOT loosen a class.
Loosening fails with `PROJECTION-SIDE-EFFECT-LOOSENED`.

For the purpose of this rule, the `sideEffectClass` enum is ordered from
least to most invasive:

```
none  <  read_only  <  disk_read  <  disk_write  <  network_outbound  <  process_spawn  <  desktop_ui
```

- "Tightening" means moving to a class that is strictly less invasive in
  this order, AND the underlying capability truly carries that
  reduction. For example, a `disk_write` capability may be tightened to
  `disk_read` only if the capability does not in fact write.
- "Loosening" means moving to a class that is strictly more invasive
  in this order, OR asserting a class the underlying capability does
  not have. For example, `disk_read` to `disk_write`, or
  `disk_write` to `network_outbound`, are both loosening.
- "Equivalent" means the same class, which is always allowed.

This order is the contract authority for the loosening rule. Hosts MAY
extend it locally, but they MUST NOT relax the contract.

### R3. Verification flow

The flow is read-only, deterministic, and runs in this order. Every step
emits findings the next step can consume; the final step emits the readiness
receipt.

**R3.1 Schema and reference validation.** Validate the package against
`contracts/schemas/elegy-plugin-package-v2.schema.json`. For every entry in
`components.skillDefinitions[]`:

- If `definitionRef` is set, load the JSON and validate it against
  `contracts/schemas/skill.schema.json`.
- If `definition` is inline, validate the inline object against the same
  schema.
- If both are set, they MUST be equal; otherwise fail with
  `SKILL-DEFINITION-INLINE-AND-REF-MISMATCH`.

**R3.2 Projection consistency validation.** Apply R2.1-R2.5. Emit one
finding per rule violation; the readiness status degrades accordingly.

**R3.3 Binary resolution.** Build the set of required CLI names from the
referenced skills. For each skill, `hostProjection.cliName` is the
authoritative name. For each entry in `components.capabilityProjections[]`,
the package's `projection.lane == "subprocess"` (or `cli`) implies the
binary on disk; `lane == "mcp"` implies the MCP server descriptor is
reachable through the host's MCP tool discovery; `lane == "api"` and
`lane == "plugin"` are out of scope for binary resolution in this spec and
MUST be reported as `unsupported` unless the host provides a resolver.

For each required binary, look it up in the install receipt:

- `install-receipt.json.installedAssets[]` carries
  `surface`, `target`, `installPath`, and `executablePath` per CLI surface.
- Match by `surface` (e.g. `elegy-planning`) and by `executablePath` if
  available, else by `installPath/<binaryName>`.
- If the receipt is absent, the verifier MUST treat all binaries as
  `unknown` and downgrade the overall status to `partial` with a
  `READINESS-NO-INSTALL-RECEIPT` finding.
- A path resolver MAY also accept `--bin-dir <path>` to override the
  receipt lookup for cases where the binary exists but the receipt was
  not written (for example, `cargo install`).

- If a required binary is not found via the install receipt's
  `installedAssets[]` (or, when `--bin-dir` is passed, not found at
  `<bin-dir>/<binaryName>`) and the receipt is present, mark the
  binary as `missing` and emit a `BINARY-MISSING` finding. This is a
  non-blocking condition that degrades the overall status to
  `partial`.

**R3.4 Machine-output probe.** For every binary that resolved to a path
in R3.3 (status is not `missing` or `unknown`), run a bounded probe:

- Command: `<binary> --version` followed by `<binary> --json --help` when
  the binary advertises JSON support.
- Timeout: 5 seconds per command.
- If the binary exits 0, the binary is `present`.
- If the binary exits non-zero, times out, or emits invalid machine
  output, the binary is `broken`; emit a `BINARY-BROKEN` finding. This is a
  blocking condition.

A binary whose resolved path does not exist on disk (e.g., the receipt
lists an `executablePath` that is absent) is still `missing` and MUST NOT
reach the probe. The verifier MUST NOT attempt to probe a `missing` binary.

The probe is read-only and does not touch user state. The probe is skipped
entirely when `--skip-probe` is passed, in which case the verifier reports
`unprobed` and the status is `partial` with `READINESS-PROBE-SKIPPED`.

**R3.5 Status derivation.** Compute the readiness status from the
finding set:

- `ready` — no blocking findings; every required binary is `present`; the
  package is fully consistent.
- `partial` — no blocking findings; at least one binary is `missing`,
  `unprobed`, or the receipt is `unknown`; the package is still
  verifiable, and a host that can fill the gaps locally can still install
  the missing surface.
- `blocked` — at least one blocking finding: schema error, projection
  inconsistency, skill definition invalid, or a binary that is `broken`.

The exact list of blocking findings lives in R5 (output envelope); the
status derivation is the only place status is assigned.

### R4. CLI command proposal

The canonical command shape is:

```text
elegy configuration package-verify \
    --package <package.json> \
    --install-receipt <install-receipt.json> \
    [--bin-dir <path>] \
    [--skip-probe] \
    --json
```

**R4.1 Surface placement.** The command lives on the dedicated
`elegy-configuration` binary. This keeps it next to the existing
`elegy configuration list|show|apply|verify` flow, which already owns
deterministic materialization and drift verification. It does not introduce
a new runtime, new distribution lane, or new CLI archive.

**R4.2 Alternate placement.** If a future host needs the same contract from
the agent-facing surface, the umbrella `elegy` CLI MAY expose
`elegy agent package-check --package <package.json> --install-receipt
<install-receipt.json> --json` that delegates to the same library. The
output envelope MUST be byte-equal across both surfaces.

**R4.3 Inputs.** All inputs are local file paths; no network calls. The
command MUST refuse to run if `--package` is missing or not a JSON file
that validates against `elegy-plugin-package/v2`. The command MUST NOT
write to disk.

**R4.4 Exit codes.** `0` for `ready`, `1` for `partial`, `2` for `blocked`.
The JSON envelope is always written to stdout; the exit code is the only
machine signal the status.

**R4.5 Non-goal.** The command is not an install, upgrade, repair, or
discovery flow. It does not modify the install receipt, the contracts
bundle, the package, or any system state.

### R5. Output envelope (readiness receipt)

The JSON envelope is the readiness receipt. Its shape is the contract
authority for downstream hosts; later schemas MUST keep this layout.

```json
{
  "schemaVersion": "elegy-plugin-readiness/v1",
  "command": ["elegy", "configuration", "package-verify"],
  "status": "ready" | "partial" | "blocked",
  "package": {
    "packageId": "elegy.planning-plugin",
    "name": "elegy-planning",
    "version": "0.1.0",
    "displayName": "Elegy Planning Plugin",
    "schemaVersion": "elegy-plugin-package/v2"
  },
  "verifiedSkills": [
    {
      "namespace": "elegy",
      "name": "planning",
      "version": "0.1.0",
      "definitionRef": "contracts/fixtures/skill.elegy-planning.json",
      "lifecycleState": "active",
      "cliName": "elegy-planning",
      "outputContractId": "elegy-planning-v1",
      "defaultSideEffectClass": "disk_write"
    }
  ],
  "projectedTools": [
    {
      "id": "planning-goal-create-cli",
      "skill": "elegy.planning",
      "capability": "planning-goal-create",
      "functionName": "planning_goal_create",
      "lane": "subprocess",
      "sideEffectClass": "disk_write",
      "supportsDryRun": false,
      "binary": {
        "surface": "elegy-planning",
        "status": "present" | "missing" | "broken" | "unprobed" | "unknown",
        "path": "tools/elegy/bin/elegy-planning/elegy-planning.exe",
        "probe": {
          "version": "elegy-planning 0.1.0",
          "jsonHelpSupported": true,
          "exitCode": 0
        }
      }
    }
  ],
  "omittedCapabilities": [],
  "missingBinaries": [],
  "unsupportedCapabilities": [],
  "sideEffectSummary": {
    "none": 0,
    "read_only": 0,
    "disk_read": 7,
    "disk_write": 6,
    "network_outbound": 0,
    "process_spawn": 0,
    "desktop_ui": 0
  },
  "hostPolicyHints": {
    "sideEffectClass": "disk_write",
    "requiresApproval": false,
    "policyTags": ["planning-authority", "scope-aware", "sqlite-backed"]
  },
  "findings": [
    {
      "code": "READINESS-NO-INSTALL-RECEIPT",
      "severity": "warning",
      "message": "No install receipt was provided; binary resolution is unknown.",
      "subject": { "kind": "package", "id": "elegy.planning-plugin" }
    }
  ],
  "subset": null
}
```

**R5.1 Status fields.**

- `status` is one of `ready`, `partial`, `blocked` (R3.5).
- `findings[]` is the canonical finding list. Every finding has `code`,
  `severity` (`info` | `warning` | `error` | `blocking`), `message`,
  and `subject` (a structured reference to the package, skill, capability,
  or binary that produced the finding).
- `missingBinaries[]` is a denormalized list of `{"cliName", "skill",
  "capabilityIds", "subject": "binary"}` for fast host consumption.
- `unsupportedCapabilities[]` lists capabilities whose `lane` could not be
  resolved by the verifier (e.g. `api`, `plugin`) so the host knows what
  needs its own resolver.
- `omittedCapabilities[]` mirrors `metadata.subsetOf.omitted` when the
  package declares a deliberate subset.
- `subset` echoes the full `metadata.subsetOf` block when present, else
  `null`.

**R5.2 Blocking findings.** The following codes are `blocking` and force
`status: blocked`:

- `PACKAGE-SCHEMA-INVALID`
- `SKILL-DEFINITION-INVALID`
- `SKILL-DEFINITION-INLINE-AND-REF-MISMATCH`
- `PROJECTION-CAPABILITY-UNKNOWN`
- `PROJECTION-SKILL-UNKNOWN`
- `PROJECTION-SIDE-EFFECT-LOOSENED`
- `PROJECTION-DRY-RUN-OVERSTATED`
- `PROJECTION-LANE-MISMATCH`
- `SUBSET-MISSING-MARKER`
- `BINARY-BROKEN`

**R5.3 Non-blocking findings.** The following codes are `warning` and only
degrade the status to `partial` if the relevant section is otherwise empty:

- `READINESS-NO-INSTALL-RECEIPT`
- `READINESS-PROBE-SKIPPED`
- `BINARY-MISSING`
- `BINARY-UNPROBED`

**R5.4 `info` findings.** A package may emit advisory `info` findings
(for example, "package declares a subset of 3 capabilities") that never
affect the status.

**R5.5 No finding implies approval.** A `ready` status is a statement of
verification, not a statement of host-side trust, approval, or policy.
Hosts MUST treat the readiness receipt as one input among many, and
MUST keep their own approval, auth, and policy decisions.

### R6. Codex projection conservatism

`elegy generate codex-plugin` MUST keep its current conservative
posture and MUST NOT silently widen it. The existing rules from
`docs/architecture/codex-plugin-projection.md` are restated here as
contract rules; any future widening is a contract change and not a
generator tweak.

**R6.1 Files emitted today.** The generator continues to emit
`.codex-plugin/plugin.json` and `skills/<id>/SKILL.md`. No other files are
emitted by default.

**R6.2 No `.mcp.json` emission until future MCP launch schema.** The
generator MUST NOT emit `.mcp.json`. The current governed
`mcp-server-descriptor` schema only proves descriptor and tool metadata;
it does not carry truthful Codex-runnable launch metadata such as
`command`, `args`, `cwd`/`env` policy, or transport details. Emission
of `.mcp.json` stays blocked until a future schema revision explicitly
adds those launch fields. When that schema lands, update this rule, the
Non-Goals entry, AC10, and the Validation Evidence block together. Do
not widen the generator alone.

**R6.3 Apps and connectors never emitted.** The generator MUST NOT emit
`.app.json`, `apps/`, marketplace metadata, hooks, or any artifact that
implies connector identity, auth, secrets, workspace IDs, or runtime
sessions. A package that wants to express "this capability needs a
connector" MUST do so by carrying an honest
`hostPolicyHints.policyTags` entry such as `requires-connector:<id>`
and letting the host do the binding.

**R6.4 No `subset` widening through projection.** A package that declares
`metadata.subsetOf` MUST project only the non-omitted capabilities into
Codex. Omitted capabilities MUST NOT appear in generated `skills/`.

**R6.5 Provenance and license surface.** Generated `.codex-plugin/plugin.json`
MUST include the package's `metadata.license`, `homepage`, and
`documentationUri` when present. Provenance fields (`sourceRepository`,
`sourceRef`, `sourceCommit`) MAY be omitted; if present, they MUST match
the package's `publishing.*` fields exactly.

### R7. Pilot: `elegy-planning`

The pilot package is
`contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json`. The
pilot exercises the contract without changing the governed skill
contract surface.

**R7.1 Keep one broad governed skill definition.** The pilot does not
split `skill.elegy-planning.json`. The current definition
carries the full durable surface (goal/roadmap/plan/todo/issue/review/validation/health/project-render/scope/project-run/work-point).

**R7.2 Do not split the CLI capability contract unless the runtime
surface changes.** The pilot does not split
`components.capabilityProjections[]` across multiple package fixtures.
The current 13 entries stay together. If a future pilot needs fewer
projections, it MUST declare `metadata.subsetOf` on the same package
rather than emit a new package fixture with a phantom skill.

**R7.3 Codex-facing instruction skills MAY be split if usage triggers
are too broad.** A future slice MAY split the repo-local
`.agents/skills/elegy-planning/SKILL.md` into narrower Codex-facing
instruction skills along the trigger boundaries:

- `elegy-planning-authoring` — creating goals, roadmaps, plans, todos,
  issues, review points.
- `elegy-planning-leases` — project-run claim/activate/release/evidence.
- `elegy-planning-graph` — work-point next-runnable and work-graph
  inspection.
- `elegy-planning-validation` — validate, health, render, scope
  context.

A split is gated on real trigger data; a hypothetical split is not
allowed. The governed skill definition and the package's
`components.capabilityProjections[]` stay unchanged across the split;
only the instruction-skill mirror changes.

**R7.4 Pilot acceptance bar.** The pilot passes when:

- The pilot package is updated to declare
  `metadata.subsetOf.skill = "elegy.planning"`,
  `metadata.subsetOf.version = "0.1.0"`,
  `metadata.subsetOf.omitted = [<the 33 capability ids not currently in
  components.capabilityProjections[]>]`, and
  `metadata.subsetOf.reason = "<non-empty rationale>"`. The
  `elegy-plugin-package/v2` schema revision that adds
  `metadata.subsetOf` is a sibling change to this spec and MUST land
  before the verifier can run against the pilot.
- `elegy configuration package-verify --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json
  --install-receipt <fixture> --json` returns `status: ready` against
  a fixture install receipt that lists `elegy-planning` in
  `installedAssets[]`.
- The same command returns `status: blocked` when the fixture receipt
  lists a binary that is `broken` (e.g. wrong `executablePath`).
- The same command returns `status: partial` when the receipt is
  absent.
- `elegy generate codex-plugin --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json
  --output-dir <tmp> --force` continues to emit only
  `.codex-plugin/plugin.json` and `skills/`; no `.mcp.json`,
  `.app.json`, or `hooks/hooks.json` is emitted because the package
  has no truthful MCP launch metadata.

## Non-Goals

- **No new runtime, marketplace, or install authority.** The verifier is
  read-only. Install, upgrade, repair, auth, approvals, lease state, and
  runtime session lifecycle stay with the host.
- **No app/connector support in the portable package.** Apps and
  connectors are host-owned; the package MUST NOT carry launch, auth,
  secrets, workspace IDs, or runtime sessions.
- **No `.mcp.json` emission.** The generator does not emit `.mcp.json`
  because the current MCP descriptor schema lacks Codex-runnable
  launch fields (command, args, cwd/env policy, transport). Emission
  is blocked until a future schema revision adds them.
- **No widening of the `subset` rule.** A package cannot become a "fake
  full" by claiming `subsetOf` is a one-time declaration; the rule is
  per-package-version and must be re-asserted on every change.
- **No loosening of side-effect classes.** A package may tighten (e.g.
  `disk_write` → `disk_read`) only when the underlying capability truly
  is read-only. It MUST NOT loosen.
- **No proliferation of capability projection surfaces.** The current
  `lane` enum is `api`, `mcp`, `plugin`, `cli`, `subprocess`. New lanes
  require a schema revision and a contract change, not a generator
  tweak.
- **No auto-score of prose quality or architecture soundness.** The
  verifier checks objective facts only — shape, references, side-effect
  classes, binary presence, exit codes. Subjective quality stays out.
- **No Codex marketplace, hooks, or app metadata without a contract
  change.** Any of these needs an ADR or a spec revision, not a
  generator-only addition.
- **No dual-write of the readiness receipt.** The receipt lives in
  stdout and the host's downstream log; it is not mirrored into
  `install-receipt.json` or the contracts bundle.

## Acceptance Criteria

Each item is observable and machine-checkable.

- **AC1** A package whose
  `components.capabilityProjections[].capability` does not resolve to a
  real `capabilities[].id` on any referenced skill fails the verifier
  with `PROJECTION-CAPABILITY-UNKNOWN` and `status: blocked`.
- **AC2** A package whose
  `components.capabilityProjections[].sideEffectClass` loosens the
  underlying `hostProjection` class fails the verifier with
  `PROJECTION-SIDE-EFFECT-LOOSENED` and `status: blocked`.
- **AC3** A package that omits a `hostProjection.capabilityProjections[].capabilityId`
  without `metadata.subsetOf` fails the verifier with
  `SUBSET-MISSING-MARKER` and `status: blocked`.
- **AC4** A package that declares `metadata.subsetOf` with a non-empty
  `omitted[]` and a non-empty `reason` passes projection consistency
  even when fewer entries are present in
  `components.capabilityProjections[]`.
- **AC5** `elegy configuration package-verify --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json
  --install-receipt <fixture> --json` returns `status: ready` against
  a fixture install receipt that lists `elegy-planning` in
  `installedAssets[]`. This AC depends on R7.4 — the pilot package MUST
  declare `metadata.subsetOf` covering the 33 hostProjection entries it
  does not project, and the `elegy-plugin-package/v2` schema MUST
  support the field, before this AC can be exercised.
- **AC6** The same command returns `status: blocked` with a
  `BINARY-BROKEN` finding when the receipt points to an executable
  path that exists on disk but the binary probe exits non-zero,
  times out, or emits invalid machine output.
- **AC6b** The same command returns `status: partial` with a
  `BINARY-MISSING` finding when the receipt does not list a matching
  installed asset for a required subprocess-lane capability, or when
  the receipt lists an `executablePath` that does not exist on disk.

- **AC7** The same command returns `status: partial` with
  `READINESS-NO-INSTALL-RECEIPT` when the receipt is absent.
- **AC8** The same command returns `status: partial` with
  `READINESS-PROBE-SKIPPED` when `--skip-probe` is passed.
- **AC9** The readiness receipt's
  `sideEffectSummary` matches the per-capability
  `sideEffectClass` counts; a `disk_read` capability shows up under
  `disk_read` and never under `disk_write`.
- **AC10** `elegy generate codex-plugin --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json
  --output-dir <tmp> --force` emits only `.codex-plugin/plugin.json`
  and `skills/`. No `.mcp.json`, `.app.json`, `apps/`,
  `hooks/hooks.json`, or marketplace metadata is emitted.
- **AC11** The generated `.codex-plugin/plugin.json` does not claim
  bundled apps, MCP servers, or hooks. Its `skills` field points at the
  generated `skills/` directory.
- **AC12** A package whose `metadata.subsetOf.omitted[]` is non-empty
  has fewer `skills/<id>/SKILL.md` directories emitted than its
  referenced skill's `hostProjection.capabilityProjections[]` entries.
- **AC13** A package whose `components.capabilityProjections[]`
  contains an entry with `lane: "api"` or `lane: "plugin"` emits
  `unsupportedCapabilities[]` containing that capability id and the
  verifier does not try to resolve a binary for it.
- **AC14** `cargo test -p elegy-configuration` (or the dedicated test
  crate that hosts the verifier) includes the contract and CLI tests
  described in R7.4 plus the negative fixtures in the Test Plan below.

## Links

- Pilot package:
  `contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json`
- Pilot skill definition:
  `contracts/fixtures/skill.elegy-planning.json`
  (specifically the `hostProjection` block at line 1928)
- Pilot readiness architecture:
  `docs/architecture/elegy-plugin-readiness.md`
- Pilot codex projection rules:
  `docs/architecture/codex-plugin-projection.md`
- Pilot install receipt source:
  `scripts/install-distribution.ps1` (lines 880-923) and
  `docs/distribution.md` (line 103)
- Code-level authority for the verifier (planned):
  `rust/crates/elegy-configuration/` (new
  `package-verify` command) or a new `rust/crates/elegy-readiness/`
  module if the verifier is reused beyond `elegy-configuration`. The
  command is read-only, so it is safe to add to either surface.
- Schemas that the verifier consumes:
  `contracts/schemas/elegy-plugin-package-v1.schema.json`,
  `contracts/schemas/elegy-plugin-package-v2.schema.json`,
  `contracts/schemas/skill.schema.json`.
- Companion roadmap:
  `docs/roadmaps/ai-agent-integration-roadmap.md` (P2 "MCP subprocess
  dispatch needs stronger context and result handling" — explicitly
  calls for "Resolve executable paths from an install receipt or
  manifest instead of only PATH/current executable heuristics").

## Validation

The following are the minimum validation commands for the pilot slice;
they MUST stay green on `main` once the verifier is implemented.

- `node scripts/validate-specs.js docs/specs` — confirms the spec
  satisfies the shared contract for `docs/specs/` documents when the
  repo carries the validator. (If the validator is absent, this spec
  is still load-bearing; the contract comes from this document.)
- `cargo test -p elegy-configuration` — runs the new
  `package-verify` unit and integration tests described in the Test
  Plan.
- `elegy configuration package-verify --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json
  --install-receipt <fixture> --json` — emits a `ready` receipt.
- `elegy configuration package-verify --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json --json`
  (no receipt) — emits a `partial` receipt with
  `READINESS-NO-INSTALL-RECEIPT`.
- `elegy generate codex-plugin --package
  contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json
  --output-dir <tmp> --force` — emits only
  `.codex-plugin/plugin.json` and `skills/`.
- `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --workspace` — keeps the wider workspace green.

## Drift Notes

- This spec is `status: draft`. It is the durable home for the
  "plugin tool availability" contract and supersedes ad-hoc discussion
  of the same idea in the plan document. Once the verifier is
  implemented and the pilot fixtures carry the matching receipt, the
  spec should be re-tagged `status: implemented` together with the
  implementation PR.
- `.mcp.json` emission depends on a future `mcp-server-descriptor`
  schema revision that adds truthful launch fields (`command`, `args`,
  `cwd`/`env` policy, transport details). The current schema only
  carries descriptor and tool metadata. Until this schema revision
  lands, R6.2 blocks `.mcp.json` emission and the Non-Goals entry
  reflects the blocked posture. When the schema revision lands, update
  R6.2, the Non-Goals entry, AC10, and the Validation Evidence block
  together. Do not widen the generator alone.
- The `elegy-plugin-package/v2` schema currently does not declare a
  `metadata.subsetOf` block. Adding it is the next schema revision and
  should land before the verifier is implemented; otherwise the
  contract's R2.2 / R2.3 / AC3 / AC4 cannot be enforced. Track the
  schema addition as a sibling change to this spec.
- The pilot package
  `contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json`
  currently projects 13 capabilities while its referenced
  `skill.elegy-planning.json` `hostProjection` declares
  46. Under R2.3 the package MUST declare `metadata.subsetOf` covering
  the 33 omitted capabilities, or AC5 cannot pass. The schema gap
  above blocks this update, so the two changes are co-dependent.
  Treat the schema addition, the package update, and the verifier
  implementation as one pilot slice.
- The CLI command's name is `elegy configuration package-verify`. The
  alternate `elegy agent package-check` is allowed for parity but is
  not the canonical placement. Do not ship both with different
  output envelopes; if both ship, R4.2's byte-equal requirement is the
  contract.
- The readiness receipt schema `elegy-plugin-readiness/v1` is a
  brand-new envelope. It belongs in
  `contracts/schemas/elegy-plugin-readiness.schema.json` once the
  verifier lands; this spec is the prose authority until then.
