{
  "authorityPosture": "derived output; source files remain authoritative",
  "config": {
    "authorityRoots": {
      "current": [
        "docs/adr",
        "docs/specs"
      ],
      "generated": [],
      "planning": [],
      "research": []
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
      "status": "accepted",
      "summary": "Elegy distributes marketplace metadata as one static `elegy-marketplace/v1`",
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
      "authorityClass": "other",
      "docKind": "generated",
      "freshness": "unknown",
      "path": "docs/ai/checks-demo-report.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "**Generated:** 2026-07-08",
      "title": "Agent-Ready Repo Pack Demo"
    },
    {
      "authorityClass": "other",
      "docKind": "index",
      "freshness": "unknown",
      "path": "docs/architecture/README.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "This directory contains the current architectural guidance for the Elegy repo.",
      "title": "Architecture Docs"
    },
    {
      "authorityClass": "other",
      "docKind": "guide",
      "freshness": "unknown",
      "path": "docs/architecture/documentation-practices.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "This document defines the current documentation doctrine for Elegy.",
      "title": "Documentation Practices"
    },
    {
      "authorityClass": "other",
      "docKind": "system",
      "freshness": "unknown",
      "path": "docs/architecture/ecosystem-topology.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "This document defines the current high-level organization of the Elegy ecosystem so docs, exports, and implementation ownership stay aligned with the repo that actually exists.",
      "title": "Elegy Ecosystem Topology"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/mcp-skill-tooling-placement.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Simplified. MCP authoring and descriptor validation remain in `elegy-mcp`. The skill registry (`elegy-skills`) serves list/search/resolve/get/validate against Agent Skills (SKILL.md). MCP-to-skill generation has been removed. Package-backed configuration has been removed.",
      "title": "MCP, Skill, and Tooling Placement"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/shared-crate-boundaries.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Shared crates stay separate only when they own a real boundary: cross-surface",
      "title": "Shared Crate Boundaries"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/skill-core-v1.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Agent Skills are standard SKILL.md files. Plugin-owned skills live under",
      "title": "Skill Core"
    },
    {
      "authorityClass": "other",
      "docKind": "system",
      "freshness": "unknown",
      "path": "docs/architecture/substrate-governance.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "This document defines the active dependency and ownership rules for the current",
      "title": "Elegy Substrate Governance"
    },
    {
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/architecture/terminology.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "This glossary defines the terms that Phase 1 treats as canonical across the Elegy umbrella repo.",
      "title": "Elegy Terminology"
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
      "authorityClass": "other",
      "docKind": "reference",
      "freshness": "unknown",
      "path": "docs/repo-layout.md",
      "sourceOfTruth": "unclassified",
      "status": "active",
      "summary": "Elegy separates shipped surfaces by role. Directory placement is part of the",
      "title": "Repository Layout"
    },
    {
      "authorityClass": "other",
      "docKind": "research",
      "freshness": "unknown",
      "path": "docs/research/openclaw-orchestration-gap-roadmap.md",
      "sourceOfTruth": "unclassified",
      "status": "exploratory",
      "summary": "Updated: 2026-03-25",
      "title": "Research OpenClaw orchestration gap roadmap"
    },
    {
      "authorityClass": "other",
      "created": "2026-05-13",
      "docKind": "planning",
      "freshness": "n/a",
      "path": "docs/roadmaps/observation-substrate-roadmap.md",
      "sourceOfTruth": "unclassified",
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
      "summary": "`elegy-capability-catalog/v1` is the shared governed contract for plugin",
      "title": "Capability Catalog V1"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/monetization-infrastructure.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Define the create → distribute → sell → self-use infrastructure for monetizable",
      "title": "Monetization infrastructure"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/plugin-marketplace-v1.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "The marketplace root contains `.elegy/marketplace.json`.",
      "title": "Plugin marketplace v1"
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
    "docs/adr/2026-05-25-centralize-documentation-practices-doctrine.md",
    "docs/adr/2026-06-15-adopt-elegy-planning-graph-core.md",
    "docs/adr/2026-06-15-block-crates-io-publishing.md",
    "docs/adr/2026-07-01-adopt-static-plugin-marketplace.md",
    "docs/adr/2026-07-07-adopt-repo-surface-taxonomy.md",
    "docs/adr/2026-07-08-adopt-capability-kind-taxonomy.md",
    "docs/adr/README.md",
    "docs/specs/README.md",
    "docs/specs/capability-catalog-v1.md",
    "docs/specs/monetization-infrastructure.md",
    "docs/specs/plugin-marketplace-v1.md",
    "docs/architecture/ecosystem-topology.md",
    "docs/architecture/substrate-governance.md",
    "docs/agent-integration.md",
    "docs/architecture/documentation-practices.md",
    "docs/distribution.md",
    "docs/architecture/mcp-skill-tooling-placement.md",
    "docs/architecture/shared-crate-boundaries.md",
    "docs/architecture/skill-core-v1.md",
    "docs/architecture/terminology.md",
    "docs/ops/main-ruleset.md",
    "docs/repo-layout.md",
    "docs/spec-baseline.md",
    "docs/roadmaps/observation-substrate-roadmap.md",
    "docs/research/openclaw-orchestration-gap-roadmap.md",
    "docs/ai/checks-demo-report.md",
    "docs/architecture/README.md"
  ],
  "schemaVersion": "documentation-bundle/v1"
}