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
    "compatibilityMode": "v1-compat",
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
    "localExceptions": [],
    "requiredFrontmatter": [
      "title",
      "status",
      "owner"
    ],
    "schemaVersion": "elegy-docs/v1"
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
      "path": "docs/specs/elegy-planning.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Durable planning authority for goals, roadmaps, plans, todos, issues, review points, work-point graphs, project-run leases, validation, and projection rendering.",
      "title": "elegy-planning Spec"
    },
    {
      "authorityClass": "current",
      "created": "2026-06-02",
      "docKind": "architecture",
      "freshness": "fresh",
      "path": "docs/architecture/elegy-planning-v1.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "Architecture mirror for the elegy-planning durable planning surface, mirroring the elegy-memory-v1 and elegy-configuration-v1 layout.",
      "title": "Elegy-planning V1"
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
    "docs/adr/README.md",
    "docs/specs/README.md",
    "docs/specs/documentation-practices-skill-and-cli.md",
    "docs/specs/elegy-planning.md",
    "docs/architecture/elegy-planning-v1.md"
  ],
  "schemaVersion": "documentation-bundle/v1"
}
