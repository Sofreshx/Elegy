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
      "freshness": "unknown",
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
      "freshness": "unknown",
      "path": "docs/adr/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: ADRs",
      "title": "ADRs"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/README.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: Specs",
      "title": "Specs"
    },
    {
      "authorityClass": "current",
      "docKind": "spec",
      "freshness": "unknown",
      "path": "docs/specs/documentation-practices-skill-and-cli.md",
      "sourceOfTruth": "current-canon",
      "status": "active",
      "summary": "title: Documentation practices skill and CLI",
      "title": "Documentation practices skill and CLI"
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
    "docs/specs/documentation-practices-skill-and-cli.md"
  ],
  "schemaVersion": "documentation-bundle/v1"
}