---
name: elegy-documentation
description: Dedicated authority-aware documentation inspection, mapping, objective checking, and export over repo-local docs roots.
version: "2.0"
---

# Elegy Documentation Authority

Dedicated authority-aware documentation inspection, mapping, objective checking, and export over repo-local docs roots.

## Capabilities

- `documentation-init`: Initialize .elegy/docs.yaml with authority roots, entrypoints, derived surfaces, and freshness defaults.
- `documentation-inspect`: Inspect documentation authority roots, entrypoints, and derived surfaces for a repo without changing files.
- `documentation-map`: Map repo-local documentation into current, planning, research, generated, and other classes with derived-surface drift state.
- `documentation-check`: Run objective documentation checks for metadata, authority/status alignment, parseable dates, internal links, freshness warnings, and export drift.
- `documentation-export-llms`: Render a deterministic non-authoritative llms.txt style export over the current documentation map.
- `documentation-export-bundle`: Render a deterministic non-authoritative JSON bundle over the current documentation map.
