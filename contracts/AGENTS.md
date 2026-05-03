# Elegy Contracts

## Authority

- `contracts/schemas/**` and `contracts/fixtures/skill-definition-v2.*.json` are the durable agent-facing contract surface.
- Discovery indexes are projections of v2 definitions, not independent authority.
- Do not add or revive v1 `skill-definition.*.json` files.

## Change Rules

- Keep capability ids, aliases, side-effect flags, argument templates, and output envelopes consistent across schema, fixture, and generated/discovery projections.
- Prefer explicit compatibility entries when changing a contract that external hosts may already consume.
- Treat examples as contract tests: update them with the schema and implementation in the same change.

## Validation

- Validate schema/fixture edits with the narrowest available contract or Rust tests.
- For agent-facing changes, manually inspect the JSON that `elegy agent manifest/discover` or `elegy skills describe` emits.
