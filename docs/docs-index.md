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
      "freshness": "fresh",
      "path": "docs/adr/2026-05-25-centralize-documentation-practices-doctrine.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "title: Centralize documentation practices doctrine",
      "title": "Centralize documentation practices doctrine"
    },
    {
      "authorityClass": "current",
      "created": "2026-05-29",
      "docKind": "adr",
      "freshness": "fresh",
      "path": "docs/adr/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: ADRs",
      "title": "ADRs"
    },
    {
      "authorityClass": "current",
      "created": "2026-06-15",
      "docKind": "adr",
      "freshness": "fresh",
      "path": "docs/adr/2026-06-15-adopt-elegy-planning-graph-core.md",
      "sourceOfTruth": "current-canon",
      "status": "accepted",
      "summary": "title: Adopt elegy-planning graph core",
      "title": "Adopt elegy-planning graph core"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "fresh",
      "path": "docs/specs/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: Specs",
      "title": "Specs"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "fresh",
      "path": "docs/specs/documentation-practices-skill-and-cli.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: Documentation practices skill and CLI",
      "title": "Documentation practices skill and CLI"
    },
    {
      "authorityClass": "current",
      "created": "2026-05-25",
      "updated": "2026-06-02",
      "docKind": "spec",
      "freshness": "fresh",
      "path": "rust/features/elegy-planning/docs/specs/index.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Durable planning authority for goals, roadmaps, plans, todos, issues, review points, work-point graphs, project-run leases, validation, and projection rendering.",
      "title": "elegy-planning Spec"
    },
    {
      "authorityClass": "current",
      "created": "2026-06-02",
      "docKind": "spec",
      "freshness": "fresh",
      "path": "rust/features/elegy-planning/docs/specs/state-machine.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: elegy-planning state machine",
      "title": "elegy-planning state machine"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "fresh",
      "path": "docs/specs/obsidian-skill-and-cli.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: Obsidian skill and CLI",
      "title": "Obsidian skill and CLI"
    }
  ],
  "entrypoints": [
    {
      "authorityClass": "other",
      "exists": true,
      "path": "README.md",
      "summary": "Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.",
      "title": "Elegy"
    }
  ],
  "projectRoot": ".",
  "recommendedReadingOrder": [
    "README.md",
    "docs/adr/2026-05-25-centralize-documentation-practices-doctrine.md",
    "docs/adr/README.md",
    "docs/specs/README.md",
    "docs/specs/documentation-practices-skill-and-cli.md",
    "rust/features/elegy-planning/docs/specs/index.md",
    "rust/features/elegy-planning/docs/specs/state-machine.md"
  ],
  "schemaVersion": "documentation-bundle/v1"
}
