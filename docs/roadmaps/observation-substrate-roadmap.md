---
created: 2026-05-13
updated: 2026-05-13
category: governance
status: active
doc_kind: roadmap
---

# Observation Substrate Roadmap

## Purpose

Define the reusable observation and recording substrate that Elegy should own for downstream hosts such as Holon without absorbing host-local consent, policy, evidence, or product UX.

## Boundary

Elegy owns:

- portable observation contracts
- reusable low-level collector crates
- deterministic session buffering and export shaping
- thin `elegy observe ...` machine-facing CLI behavior

Elegy does not own:

- observation leases or user-consent state
- workspace or assistant attachment
- evidence graph semantics
- promotion, suggestion, or workflow-learning policy
- product-specific operator UX

## Why now

The current `elegy observe ...` surface is snapshot-oriented. It does not yet provide the typed event, session, and summary contracts needed for a bounded recorder that AI hosts can consume efficiently.

This roadmap narrows the next Elegy observation work to a first Windows-first recorder slice instead of reopening broad desktop automation scope.

## First slice

### OBS-001 Add portable observation recording contracts

Status:
- implemented in the first bounded recorder slice on 2026-05-13

Goal:
- Add governed contracts for `ObservationEvent`, `ObservationSession`, `ObservationSummary`, and optional `ObservationAssetRef`.

Acceptance:
- Contracts model progressive disclosure: summary first, expandable timeline second, bulky assets by ref.
- Contracts stay host-neutral and do not include Holon workspace or approval nouns.

### OBS-002 Add bounded recorder substrate under the existing observe family

Status:
- implemented as a bounded polling-based `elegy observe record` slice on 2026-05-13

Goal:
- Extend the current observation substrate with a Windows-first focus recorder under `elegy observe record`.

Acceptance:
- The first recorder lane captures foreground-window changes plus bounded session metadata.
- The first recorder lane does not require raw keylogging, screenshots, or UIA trees.
- No separate top-level binary is introduced yet.

### OBS-003 Add recorder crates without turning `elegy-observe` into a catch-all

Status:
- deferred

Goal:
- Keep crate boundaries clean while adding recorder behavior.

Acceptance:
- Add `elegy-observe-hooks` for Windows event hooks.
- Add `elegy-observe-session` for session lifecycle, compaction, and export shaping.
- Add a later optional `elegy-observe-uia` crate only when semantic desktop observation is actually selected.

Note:
- The first slice stayed smaller by implementing bounded polling inside `elegy-observe` and deferring dedicated recorder crates until hooks or richer lanes justify them.

## Library posture

- Prefer direct `windows` bindings for small Win32 event-hook surfaces.
- Continue using `sysinfo`, `arboard`, and `notify` where they already fit.
- Treat `uiautomation` as the likely later semantic-observation dependency.
- Treat `windows-capture` or `xcap` as later visual-observation options.
- Do not let weaker or stale input-hook crates drive the contract model.

## Stop conditions

- Stop before adding typed text capture by default.
- Stop before moving consent or evidence semantics into Elegy.
- Stop before creating a dedicated recorder binary unless packaging or daemon-lifecycle evidence requires it.
