{
  "authorityPosture": "derived output; source files remain authoritative",
  "config": {
    "authorityRoots": {
      "current": [
        "docs/architecture",
        "docs/adr",
        "docs/specs"
      ],
      "generated": [
        "docs/ai"
      ],
      "planning": [
        "docs/plans",
        "docs/roadmaps"
      ],
      "research": [
        "docs/research"
      ]
    },
    "compatibilityMode": "v2",
    "derivedSurfaces": {
      "bundles": [
        "docs/docs-index.md"
      ],
      "llms": [],
      "manifests": [],
      "sidebars": []
    },
    "entrypoints": [
      "README.md"
    ],
    "freshnessWarnings": {
      "currentDays": 120,
      "planningDays": 45,
      "researchDays": 90
    },
    "localExceptions": [
      "docs/architecture/codex-plugin-projection.md",
      "docs/specs/codex-plugin-compatibility.md",
      "docs/specs/documentation-practices-skill-and-cli.md",
      "docs/specs/obsidian-skill-and-cli.md"
    ],
    "requiredFrontmatter": [
      "title",
      "status",
      "owner"
    ],
    "schemaVersion": "elegy-documentation/v2"
  },
  "configPath": ".elegy/docs.yaml",
  "configuredDerivedSurfaces": {
    "bundles": [
      "docs/docs-index.md"
    ],
    "llms": [],
    "manifests": [],
    "sidebars": []
  },
  "documents": [
    {
      "authorityClass": "current",
      "created": "2026-05-25",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-05-25-centralize-documentation-practices-doctrine.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "- Elegy needs one shared doctrine for deciding when changes require ADRs, specs,",
      "title": "Centralize documentation practices doctrine"
    },
    {
      "authorityClass": "current",
      "created": "2026-06-15",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-06-15-adopt-elegy-planning-graph-core.md",
      "sourceOfTruth": "current-canon",
      "status": "proposed",
      "summary": "`elegy-planning` currently models durable planning through a mostly linear",
      "title": "Adopt elegy-planning graph core"
    },
    {
      "authorityClass": "current",
      "created": "2026-06-15",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-06-15-block-crates-io-publishing.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "Elegy distributes through GitHub Releases, binary artifacts, wrapper",
      "title": "Block all crates.io publishing; keep advisory crate smoke test"
    },
    {
      "authorityClass": "current",
      "created": "2026-07-01",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-07-01-adopt-static-plugin-marketplace.md",
      "sourceOfTruth": "current-canon",
      "status": "superseded",
      "summary": "> Superseded on 2026-07-30 by marketplace v2 and the Codex-parity ADR. This",
      "title": "Adopt a static plugin marketplace"
    },
    {
      "authorityClass": "current",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-07-07-adopt-repo-surface-taxonomy.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "Accepted.",
      "title": "Adopt Repo Surface Taxonomy"
    },
    {
      "authorityClass": "current",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-07-08-adopt-capability-kind-taxonomy.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "Accepted.",
      "title": "Adopt Capability-Kind Taxonomy"
    },
    {
      "authorityClass": "current",
      "created": "2026-07-20",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-07-20-retire-central-skill-registry.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "Adapters may own optional Agent Skills, while standalone skills use host-native",
      "title": "Retire the central skill registry and resolver"
    },
    {
      "authorityClass": "current",
      "created": "2026-07-30",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-07-30-adopt-codex-parity-and-delegated-authentication.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "Elegy's v1/v2 abstraction renamed or omitted native Codex package behavior and",
      "title": "Adopt Codex-parity packages and delegated authentication"
    },
    {
      "authorityClass": "current",
      "created": "2026-07-30",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/2026-07-30-adopt-evidence-backed-readiness-and-plugin-boundary.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "Elegy documentation and discovery previously treated several different facts as if they proved the same thing:",
      "title": "Adopt evidence-backed readiness and a system-adapter plugin boundary"
    },
    {
      "authorityClass": "current",
      "created": "2026-05-29",
      "docKind": "adr",
      "freshness": "unknown",
      "path": "docs/adr/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Store durable architecture and governance decisions here. Create new records with `elegy-documentation new adr --title \"...\"`.",
      "title": "ADRs"
    },
    {
      "authorityClass": "other",
      "docKind": "guide",
      "freshness": "unknown",
      "path": "docs/agent-integration.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Elegy is designed for AI-agent hosts that can run local subprocesses. The",
      "title": "Agent Integration"
    },
    {
      "authorityClass": "generated",
      "docKind": "generated",
      "freshness": "unknown",
      "path": "docs/ai/checks-demo-report.md",
      "sourceOfTruth": "generated-derived",
      "status": "active",
      "summary": "**Generated:** 2026-07-08",
      "title": "Agent-Ready Repo Pack Demo"
    },
    {
      "authorityClass": "current",
      "docKind": "index",
      "freshness": "unknown",
      "path": "docs/architecture/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "This directory contains the current architectural guidance for the Elegy repo.",
      "title": "Architecture Docs"
    },
    {
      "authorityClass": "current",
      "docKind": "guide",
      "freshness": "unknown",
      "path": "docs/architecture/documentation-practices.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "This document defines the current documentation doctrine for Elegy.",
      "title": "Documentation Practices"
    },
    {
      "authorityClass": "current",
      "docKind": "system",
      "freshness": "unknown",
      "path": "docs/architecture/ecosystem-topology.md",
      "sourceOfTruth": "current-canon",
      "status": "current",
      "summary": "Elegy is a Rust toolkit and evidence-gated distribution layer for reusable",
      "title": "Elegy ecosystem topology"
    },
    {
      "authorityClass": "current",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/mcp-skill-tooling-placement.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "MCP authoring and descriptor validation remain in `elegy-mcp`. Agent Skills",
      "title": "MCP, Skill, and Tooling Placement"
    },
    {
      "authorityClass": "current",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/shared-crate-boundaries.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Shared crates stay separate only when they own a real boundary: cross-surface",
      "title": "Shared Crate Boundaries"
    },
    {
      "authorityClass": "current",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/skill-core-v1.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Agent skills are standard `SKILL.md` instruction packages.",
      "title": "Skill placement"
    },
    {
      "authorityClass": "current",
      "docKind": "system",
      "freshness": "unknown",
      "path": "docs/architecture/substrate-governance.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "This document defines the active dependency and ownership rules for the current",
      "title": "Elegy Substrate Governance"
    },
    {
      "authorityClass": "current",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/terminology.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "This glossary defines the canonical terms used across the Elegy umbrella repo.",
      "title": "Elegy Terminology"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/deprecations.md",
      "sourceOfTruth": "unclassified",
      "status": "current",
      "summary": "This page records labels and distribution forms that agents must no longer",
      "title": "Elegy deprecations and reclassification"
    },
    {
      "authorityClass": "other",
      "docKind": "guide",
      "freshness": "unknown",
      "path": "docs/distribution.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Elegy ships release assets through GitHub Releases, not package feeds or",
      "title": "Distribution and downstream consumption"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/ops/main-ruleset.md",
      "sourceOfTruth": "unclassified",
      "status": "draft",
      "summary": "Status: **artifact**. Not yet applied. Apply after `reconcile/main-roro` merges CI-green.",
      "title": "main-protection Branch Ruleset"
    },
    {
      "authorityClass": "planning",
      "docKind": "planning",
      "freshness": "unknown",
      "path": "docs/plans/automation-portability-handoff.md",
      "sourceOfTruth": "planning-non-canon",
      "status": "active",
      "summary": "The canonical terminology boundary landed on 2026-07-15. Automation Pack",
      "title": "Automation Portability Handoff"
    },
    {
      "authorityClass": "other",
      "docKind": "generated",
      "freshness": "unknown",
      "path": "docs/readiness.md",
      "sourceOfTruth": "unclassified",
      "status": "current",
      "summary": "This file is generated from the release catalog, adapter manifests, and canonical readiness artifacts. `surface class` states what a component is; `kind` only controls build mechanics. `implemented` means source behavior and tests exist, not that a clean installation is usable. Default agent discovery includes only `usable` and `production` active adapter plugins.",
      "title": "Elegy ecosystem readiness"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/repo-layout.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Elegy records every distributed surface's role in",
      "title": "Repository Layout"
    },
    {
      "authorityClass": "research",
      "docKind": "research",
      "freshness": "unknown",
      "path": "docs/research/historical-monetization-infrastructure.md",
      "sourceOfTruth": "research-non-canon",
      "status": "exploratory",
      "summary": "> This is retained research, not current architecture or distribution policy.",
      "title": "Historical monetization infrastructure proposal"
    },
    {
      "authorityClass": "research",
      "docKind": "research",
      "freshness": "unknown",
      "path": "docs/research/openclaw-orchestration-gap-roadmap.md",
      "sourceOfTruth": "research-non-canon",
      "status": "exploratory",
      "summary": "Updated: 2026-03-25",
      "title": "Research OpenClaw orchestration gap roadmap"
    },
    {
      "authorityClass": "planning",
      "created": "2026-05-13",
      "docKind": "planning",
      "freshness": "fresh",
      "path": "docs/roadmaps/observation-substrate-roadmap.md",
      "sourceOfTruth": "planning-non-canon",
      "status": "active",
      "summary": "Define the reusable observation and recording substrate that Elegy should own for downstream hosts such as Holon without absorbing host-local consent, policy, evidence, or product UX.",
      "title": "Observation Substrate Roadmap",
      "updated": "2026-06-30"
    },
    {
      "authorityClass": "other",
      "created": "2026-03-19",
      "docKind": "reference",
      "freshness": "n/a",
      "path": "docs/spec-baseline.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Record the protocol baseline Elegy is targeting so governed contracts, exported bundles, and Rust tooling do not drift implicitly.",
      "title": "MCP Spec Baseline",
      "updated": "2026-06-30"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Store implementation-facing behavior specs here. Create new records with `elegy-documentation new spec --title \"...\"`.",
      "title": "Specs"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/capability-catalog-v1.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "A capability catalog describes implemented invocation shape. It is not evidence",
      "title": "Capability Catalog V1"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/plugin-connections-v1.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Authentication is not one boundary.",
      "title": "Plugin connections and authentication"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/plugin-marketplace-v2.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "The canonical index is `.elegy/marketplace.json` with",
      "title": "Plugin marketplace v2"
    },
    {
      "authorityClass": "current",
      "created": "2026-07-30",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/readiness-v1.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Each distributed surface owns one JSON artifact with `schemaVersion: elegy-readiness/v1`. Plugin manifests reference it through `readiness`; standalone entries reference it from `distribution/surfaces.json`.",
      "title": "Evidence-backed readiness v1"
    }
  ],
  "entrypoints": [
    {
      "authorityClass": "other",
      "exists": true,
      "path": "README.md",
      "summary": "[![Latest release](https://img.shields.io/github/v/release/Sofreshx/Elegy?display_name=tag&sort=semver)](https://github.com/Sofreshx/Elegy/releases/latest)",
      "title": "Elegy"
    }
  ],
  "projectRoot": ".",
  "recommendedReadingOrder": [
    "README.md",
    "docs/architecture/ecosystem-topology.md",
    "docs/architecture/substrate-governance.md",
    "docs/architecture/documentation-practices.md",
    "docs/architecture/mcp-skill-tooling-placement.md",
    "docs/architecture/shared-crate-boundaries.md",
    "docs/architecture/skill-core-v1.md",
    "docs/architecture/terminology.md",
    "docs/adr/2026-05-25-centralize-documentation-practices-doctrine.md",
    "docs/adr/2026-06-15-adopt-elegy-planning-graph-core.md",
    "docs/adr/2026-06-15-block-crates-io-publishing.md",
    "docs/adr/2026-07-01-adopt-static-plugin-marketplace.md",
    "docs/adr/2026-07-07-adopt-repo-surface-taxonomy.md",
    "docs/adr/2026-07-08-adopt-capability-kind-taxonomy.md",
    "docs/adr/2026-07-20-retire-central-skill-registry.md",
    "docs/adr/2026-07-30-adopt-codex-parity-and-delegated-authentication.md",
    "docs/adr/2026-07-30-adopt-evidence-backed-readiness-and-plugin-boundary.md",
    "docs/adr/README.md",
    "docs/specs/README.md",
    "docs/specs/capability-catalog-v1.md",
    "docs/specs/plugin-connections-v1.md",
    "docs/specs/plugin-marketplace-v2.md",
    "docs/specs/readiness-v1.md",
    "docs/architecture/README.md",
    "docs/plans/automation-portability-handoff.md",
    "docs/roadmaps/observation-substrate-roadmap.md",
    "docs/research/historical-monetization-infrastructure.md",
    "docs/research/openclaw-orchestration-gap-roadmap.md",
    "docs/agent-integration.md",
    "docs/distribution.md",
    "docs/deprecations.md",
    "docs/ops/main-ruleset.md",
    "docs/repo-layout.md",
    "docs/spec-baseline.md",
    "docs/readiness.md"
  ],
  "schemaVersion": "documentation-bundle/v1"
}