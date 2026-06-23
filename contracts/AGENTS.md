# Elegy Contracts

## Authority

- `contracts/schemas/**` are the durable contract authority, but only for files
  with a real production or test consumer. Schemas whose only purpose was
  documentation of a Rust struct are removed; the struct is the source of truth.
- `contracts/fixtures/**` are governed examples and test data. Fixtures are
  kept only when a Rust conformance test loads them, or when they match a
  schema that the production code loads.
- `contracts/configuration/**` is the governed configuration materialization
  surface (templates, profiles, built-in host blocks). Keep it aligned with
  schema and fixture changes.
- Discovery indexes, generated bundles under `artifacts/`, and other
  materialized outputs are derived surfaces, not authored truth.

## Current State

After the strict purge, `contracts/schemas/` holds the 7 files with real
consumers and `contracts/fixtures/` holds the 14 fixtures loaded by the
conformance suite. Plugin packages (`elegy-plugin-package/v1`) and
skill-discoverable JSON fixtures (`skill.elegy-*.json`) are retired; the
Agent Skills directories under `skills/<name>/SKILL.md` are the agent-facing
skill surface.

The `elegy-plugin/v1` thin manifest schema and its fixture were also removed
during the purge: no `.elegy-plugin/plugin.json` file exists in this repo,
and the Rust struct (`ElegyPluginV1` in `rust/core/elegy-contracts`) is kept
only because `elegy-tooling` uses it to scaffold plugin directories. The
scaffolder writes a `plugin.json` at scaffold time but no real plugin in
this repo uses that file as its agent-facing manifest.

## Change Rules

- A schema or fixture stays in `contracts/` only while something in the Rust
  workspace or the conformance suite loads it. When the only consumer is a
  deleted test or a removed production code path, delete the file in the
  same change.
- When a governed schema is removed because its consumer is removed, also
  remove the matching Rust struct and any conformance test that exercises
  the struct (unless another consumer remains).
- When a fixture is removed, remove the corresponding `load_X_fixture_from_dir`
  helper from `elegy-contracts` and any `upstream_X_fixture_is_semantically_valid`
  conformance test.
- Keep capability ids, aliases, side-effect flags, argument templates, and
  output envelopes consistent across every governed and generated surface
  that exposes them.
- Treat examples as contract tests: update them with the schema and
  implementation in the same change.
- `contracts/configuration/**` changes must be validated with
  `elegy-configuration apply --dry-run --json` against the smallest target.

## Validation

- Use the narrowest contract or Rust validation that covers the edited
  artifact.
- For schema and fixture changes: `cd rust && cargo run -p elegy-cli --
  contracts validate --project ..` and `cargo test -p elegy-contracts`.
- For host-facing capability changes, inspect the JSON emitted by
  `elegy agent manifest/discover --detail --json` and, when relevant,
  `elegy-skills get/capability/validate --json` or the umbrella
  `elegy skills ...` compatibility surface.
- If generated outputs or archives are affected, validate the relevant
  export or canonical-output flow rather than only the source file.
- For configuration templates/profiles, prefer `elegy-configuration apply
  --dry-run --json` or `verify --json` against the smallest target that
  proves the changed materialization path.
