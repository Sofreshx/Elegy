# elegy.resources.d/

This directory is **hand-authored and load-bearing** for the `elegy-run` MCP
host (`hosts/host-mcp`). It is not generated output and must not be swept up
by an artifact-cleanup pass.

`elegy.toml` at the repo root declares `descriptors.include =
["elegy.resources.d/*.toml"]`. If this directory becomes empty or every file
in it is removed, `elegy-run` starts in **degraded mode**: it still answers
the MCP `initialize` handshake, but its resource catalog is limited to a
single diagnostic resource explaining the failing descriptor pattern
(`DESC-CONFIG-002` in `shared/descriptor/src/lib.rs`) instead of exposing
real content.

History: a previous minimal placeholder (`elegy.resources.d/planning.toml`,
`content = "Minimal descriptor for MCP host."`) was removed in commit
`e6c89c9` ("Remove obsolete generated artifacts") because nothing declared it
as load-bearing. That silently broke the `elegy-run` MCP server for the
Claude Desktop `elegy-planning` registration. Do not repeat that: before
deleting or emptying any file here, confirm nothing in
`.elegy/checks.json` (`elegy-run-preflight`) or the CI workflow depends on it.

Current descriptor: `elegy-authority.toml`, exposing repo authority docs
(`docs/readiness.md`, `docs/architecture/README.md`, `docs/docs-index.md`,
`distribution/surfaces.json`) as MCP filesystem resources.
