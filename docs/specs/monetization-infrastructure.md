---
title: Monetization infrastructure
status: active
owner: Elegy
---

# Monetization infrastructure

## Intent

Define the create → distribute → sell → self-use infrastructure for monetizable
Elegy plugins, starting with `elegy-checks` (extracted from instruction-engine)
and `elegy-client-radar` (continued from existing private repo). Both are
closed-source proprietary plugins sold as fixed-scope services first,
installable products second.

## Context evidence

- `docs/specs/plugin-marketplace-v1.md`: marketplace contract, wrapper pattern,
  archive distribution.
- `docs/adr/2026-07-01-adopt-static-plugin-marketplace.md`: private source →
  public proprietary binary archive pattern.
- `distribution/surfaces.json`: surface registry including `elegy-client-radar`
  and `elegy-ai-radar` external-plugin-wrapper entries with `main-snapshot`
  release archive URLs.
- `marketplace-wrappers/client-radar/`: thin metadata wrapper pattern
  (`.elegy-plugin/plugin.json` + README, Proprietary license).
- `instruction-engine/elegy-checks`: functional
  Rust crate (registry, runner, SQLite evidence, CI-parity map, check packs,
  4 skills) currently vendored inside instruction-engine.
- `Sofreshx/elegy-client-radar`: mature private plugin
  (prospect ranking, ICP, account history, compliance, French public data
  sources, packaging pipeline).
- Monetizing research docs (Obsidian vault): demand discovery system, opportunity
  schema, scoring formula, offer priority ranking.

## Infrastructure diagram

```text
CREATE                          DISTRIBUTE                        SELL                         SELF-USE
──────                          ──────────                        ────                         ────────

┌─────────────────────────────┐         ┌──────────────────────────┐    ┌──────────────────────┐    ┌─────────────────────────┐
│  Private impl repos         │  build  │  Elegy public repo       │    │  Direct-sales layer   │    │  Our dev repos          │
│  (source of truth, closed)  │  per-   │  (marketplace hub)       │    │  (richer than wrapper)│    │  instruction-engine     │
│                             │ target  │                          │    │                       │    │  Elegy itself           │
│  • elegy-checks   (extract) │───────▶ │  marketplace-wrappers/   │───▶│  installer + onboard  │───▶│  install our own plugin │
│  • client-radar   (continue)│  zip +  │    elegy-checks/         │    │  + license/billing    │    │  → dogfood → demo proof │
│  • ai-radar       (later)   │  sha256 │    client-radar/         │    │  (Polar)              │    │                         │
│                             │         │    ai-radar/             │    │  + thin landing page  │    │  Elegy repo = Demo 2    │
│  Rust crate + .codex-plugin │         │                          │    │    (deferred, GitHub  │    │  (Agent-Ready Repo Pack │
│  + skills + schemas         │         │  .elegy-plugin/plugin.json   │    Pages)              │    │   applied here)         │
│                             │         │  + archive URL + checksum    │                       │    │                         │
└─────────────────────────────┘         │  distribution/surfaces.json  │    └──────────────────────┘    └─────────────────────────┘
       ▲                                 │  .elegy/marketplace.json     │            ▲                              ▲
       │                                 └──────────────────────────┘            │                              │
       │ dev path (fast iteration)          │              │                       │                              │
       │ during builds                      │ Holon        │ direct download       │ install from release          │ install from release
       └────────────────────────────────────┘ ecosystem    └───────────────────────┘ archive (parity with users)   │ archive or dev path
                                        discovery                                                                │
```

### Three packaging tiers

| Tier | Shape | Audience | Billing |
|---|---|---|---|
| Marketplace wrapper (existing) | `.elegy-plugin/plugin.json` + archive URL in `.elegy/marketplace.json` | Holon ecosystem discovery | none (free listing) |
| Direct-sales package (new) | installer script + onboarding skill + setup guide + license key hook | teams outside Holon | Polar |
| Delivered service (first) | we run the tool on their repo, hand them a report + repo pack | fastest cash, validates demand | fixed-scope pricing |

## Requirements

### Allowed behavior

- Closed-source proprietary plugin binary archives published to public
  `Sofreshx/Elegy` GitHub Releases via CI.
- Thin marketplace wrappers in public Elegy repo for Holon discovery and
  self-use.
- Direct-sales installer bundle with Polar license/billing hook for
  non-marketplace users.
- Dev-path override (local cargo path/symlink) during active development of
  the checks crate, with release-archive install for parity.
- `elegy-planning` used as durable state for the monetization program
  (scope: `elegy-tools-monetization`).

### Forbidden behavior

- Do not copy implementation source code into the public Elegy repo wrappers.
- Do not put secrets, API keys, or private source URLs in wrappers, skills,
  docs, fixtures, or CI.
- Do not treat the marketplace wrapper as the direct-sales surface — it is
  distribution only, not marketing.
- Do not productize the plugin before validating demand through at least 3
  service engagements.

## Non-goals

- Building a full SaaS billing system — Polar handles this.
- Creating a custom n8n node — deferred until repeated demand.
- Building a static marketing site this sprint — org planned, build deferred.
- Replacing elegy-planning with a new store — client-radar continues to use
  its own scoring, and elegy-planning is used for program-level planning state.

## Acceptance checks

- `elegy-checks` exists as a private repo with `.elegy-plugin/plugin.json`
  (Proprietary license) and passes `cargo build + cargo test`.
  → verify: clone Sofreshx/elegy-checks, run `cargo test`
- Elegy marketplace wrapper `marketplace-wrappers/elegy-checks/` exists with
  working archive URLs.
  → verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace validate --source .`
- instruction-engine installs elegy-checks via marketplace wrapper.
  → verify: instruction-engine `.elegy/marketplace.json` lists elegy-checks
- Agent-Ready Repo Pack demo produced on Elegy repo.
  → verify: `repo-map.json` and `agent-checks.yaml` exist under `docs/ai/`
- Direct-sales installer bundle exists with Polar billing hook.
  → verify: `scripts/install.sh` and `scripts/install.ps1` download + verify
  the correct target archive
- Billing-alternatives doc exists covering Polar vs Gumroad vs Lemon Squeezy.
  → verify: `docs/billing-alternatives.md` exists in the checks impl repo
- `elegy-client-radar` scoring calibrated against 20-50 real targets.
  → verify: `docs/calibration-report.md` exists with real target examples

## Implementation links

- elegy-checks extraction: `Sofreshx/elegy-checks` (private, to be created)
- elegy-client-radar continuation: `Sofreshx/elegy-client-radar/docs/next-steps.md`
- Planning state: scope `elegy-tools-monetization` in `~/.elegy/planning.db`
- Marketplace wrapper pattern: `marketplace-wrappers/client-radar/`
- Release workflow pattern: `elegy-client-radar/.github/workflows/release-plugin.yml`

## Billing alternatives

| Surface | Pros | Cons | Best for |
|---|---|---|---|
| **Polar** (chosen) | Developer-friendly, strong OSS positioning, handles license keys + payments + files, EU-friendly | Newer platform, smaller audience | CLI plugins sold to developers |
| Gumroad | Fast to launch, broad audience, handles files + payments | Less developer-brand aligned, higher fees | Digital products with broad appeal |
| Lemon Squeezy | Merchant-of-record handles EU VAT/tax, good for EU selling | Slightly more setup, less developer-focused | EU-compliant direct sales |

Polar chosen because: developer-brand alignment, license key support (important
for closed-source CLI plugins), commission-free for open-source (useful if we
ever open-core parts), and EU-friendly payment handling.

## Validation evidence

- Planning state captured: scope `elegy-tools-monetization`, goal
  `monetize-elegy-tools`, 2 roadmaps, 8 sections, 17 work-points, 4 insights.
- Validation pass: all entities valid, advisory warnings only (work-points
  without validation expectations — expected at planning stage).

## Drift notes

None. This is the initial spec.
