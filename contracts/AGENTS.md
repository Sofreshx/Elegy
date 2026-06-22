# Elegy Contracts

## Authority

- `contracts/schemas/**` are the durable contract authority.
- `contracts/fixtures/**`, especially `skill.*.json`, are governed examples, compatibility evidence, and the stable agent-facing discovery authority.
- `contracts/configuration/**` is the governed configuration materialization surface. Keep it aligned with schema and fixture changes.
- Discovery indexes, generated bundles under `artifacts/`, and other materialized outputs are derived surfaces, not authored truth.
- Do not add or revive v1 `skill-definition.*.json` files.

## Plugin Authority

The thin `.elegy-plugin/plugin.json` manifest (`elegy-plugin/v1`) is the single plugin authority. Plugin packages (`elegy-plugin-package/v1`) are retired.

- `contracts/schemas/elegy-plugin-v1.schema.json` — Plugin manifest schema
- `contracts/fixtures/elegy-plugin-v1.fixture.json` — Upstream plugin fixture

Skill definitions and their discovery indexes have been replaced by Agent Skills directories (`skills/<name>/SKILL.md`).

Validate with `cargo test -p elegy-contracts`.

## Change Rules

- Keep schema, fixture, compatibility, and emitted discovery/projection shapes aligned when a public contract changes.
- Keep capability ids, aliases, side-effect flags, argument templates, and output envelopes consistent across every governed and generated surface that exposes them.
- Prefer explicit compatibility entries when changing a contract that external hosts may already consume.
- Treat examples as contract tests: update them with the schema and implementation in the same change.
- When changing a governed skill fixture, update its discovery index and rendered `SKILL.md` mirrors in the same change unless the change is intentionally contract-only and documented as such.
- Obsidian fixtures are external-CLI wrapper contracts. Do not add a Rust binary assumption or planning-authority claim to those artifacts.

## Validation

- Use the narrowest contract or Rust validation that covers the edited artifact.
- For host-facing capability changes, inspect the JSON emitted by `elegy agent manifest/discover --detail --json` and, when relevant, `elegy-skills get/capability/validate --json` or the umbrella `elegy skills ...` compatibility surface.
- If generated outputs or archives are affected, validate the relevant export or canonical-output flow rather than only the source file.
- For configuration templates/profiles, prefer `elegy-configuration apply --dry-run --json` or `verify --json` against the smallest target that proves the changed materialization path.
