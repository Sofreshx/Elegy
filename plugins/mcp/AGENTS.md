# Elegy MCP

## Start Here

- Read `../../docs/architecture/mcp-skill-tooling-placement.md` before changing ownership or adding new MCP-facing behavior.
- Inspect governed MCP artifacts under `../../contracts/` when changing descriptor or analysis semantics.

## Boundaries

- Governed descriptor and analysis-result artifacts remain the authority; this crate owns reusable executable author/analyze behavior over them.
- Keep host transport orchestration, auth, product policy, and hosted runtime behavior outside this crate.
- Keep outputs valid against governed contracts and do not invent parallel runtime-only shapes when a shared serialized shape is required.
- Do not document REST/OpenAPI ingestion, hosted execution, or autonomous registration as shipped behavior unless the repo proves them end to end.
