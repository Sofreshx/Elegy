# Elegy Skills

## Start Here

- Read `../../docs/architecture/skill-core-v1.md` before changing the dedicated skill-registry surface.
- Inspect governed skill artifacts under `../fixtures/` and schemas under `../schemas/` when changing registry semantics or validation behavior.

## Boundaries

- This crate owns the reusable governed skill registry API plus the dedicated `elegy-skills` CLI.
- Governed skill artifacts remain authoritative; this crate loads, validates, filters, and projects them, but does not redefine them.
- Prefer putting registry search, resolve, profile filtering, projection, and validation logic here so `elegy`, MCP host code, and downstream Rust hosts can share one implementation.
- Keep MCP-to-skill generation as lower-level contributor tooling outside this crate's main surface posture.
- Runtime registration, hosted execution, remote package management, autonomous authoring, and host orchestration stay outside this crate.
