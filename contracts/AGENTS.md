# Elegy Contracts

## Authority

- `contracts/schemas/**` are the durable contract authority.
- Governed fixtures under `contracts/fixtures/**`, especially `skill-definition-v2.*.json`, are the stable agent-facing example and discovery surface.
- Discovery indexes, generated bundles under `artifacts/`, and other materialized outputs are derived surfaces, not authored truth.
- Do not add or revive v1 `skill-definition.*.json` files.

## Change Rules

- Keep schema, fixture, compatibility, and emitted discovery/projection shapes aligned when a public contract changes.
- Keep capability ids, aliases, side-effect flags, argument templates, and output envelopes consistent across every governed and generated surface that exposes them.
- Prefer explicit compatibility entries when changing a contract that external hosts may already consume.
- Treat examples as contract tests: update them with the schema and implementation in the same change.

## Validation

- Use the narrowest contract or Rust validation that covers the edited artifact.
- For agent-facing changes, inspect the JSON that `elegy agent manifest/discover` or `elegy skills describe` emits.
- If generated outputs or archives are affected, validate the relevant export or canonical-output flow rather than only the source file.
