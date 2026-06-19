# Elegy Contracts

## Authority

- `contracts/schemas/**` are the durable contract authority.
- `contracts/fixtures/**`, especially `skill.*.json`, are governed examples, compatibility evidence, and the stable agent-facing discovery authority.
- `contracts/configuration/**` is the governed configuration materialization surface. Keep it aligned with schema and fixture changes.
- Discovery indexes, generated bundles under `artifacts/`, and other materialized outputs are derived surfaces, not authored truth.
- Do not add or revive v1 `skill-definition.*.json` files.

## Authoring a new plugin package

- Copy `contracts/fixtures/elegy-plugin-package.template.json` to `contracts/fixtures/elegy-plugin-package.<your-feature>.json` and replace every `<FILL-IN: ...>` placeholder. The template's `_template_instructions` extension field lists the steps end-to-end.
- Each plugin package is owned by the feature tree that publishes it. The `elegy-plugin-package.<feature>.json` fixture is the **single source of truth** for that feature's publish metadata: archive family, asset prefix, skill bridge path, installer filename, and optional pre/post publish hooks. There is no central catalog and no per-feature workflow file.
- For features with a binary or wrapper, the `publishing` block is required when `toolRequirements` is non-empty. The conformance test `plugin_package_fixtures_with_tool_requirements_have_publishing_metadata` enforces this. Two further conformance tests enforce the central-orchestrator contract: `plugin_package_publishing_blocks_have_orchestrator_required_fields` and `plugin_package_crate_paths_resolve_in_workspace`.
- The central publish orchestrator (`.github/workflows/publish-orchestrator.yml`) discovers surfaces by walking `contracts/fixtures/elegy-plugin-package.*.json`, reading each `publishing` block, and dispatching a build per surface through `._reusable-publish.yml`. Adding a new surface = add a fixture + nothing else. No workflow file to write, no central catalog to update.
- The `publishing` block's `cratePath` and `assetKind` fields are the orchestrator's build inputs. `cratePath` is the workspace crate name (e.g. `elegy-memory`) and is passed to `cargo build -p`. `assetKind` is `cli` (binary-only archive) or `wrapper` (binary + SKILL.md bridge + installer). Skill-only surfaces without a Rust build omit `cratePath`.
- Validate with `cargo run --manifest-path rust/Cargo.toml -p elegy-cli -- contracts validate --project .` then `cargo test -p elegy-contracts --test conformance all_plugin_package_fixtures_match_current_schema -- --nocapture`, `cargo test -p elegy-contracts --test conformance plugin_package_fixtures_with_tool_requirements_have_publishing_metadata -- --nocapture`, `cargo test -p elegy-contracts --test conformance plugin_package_publishing_blocks_have_orchestrator_required_fields -- --nocapture`, and `cargo test -p elegy-contracts --test conformance plugin_package_crate_paths_resolve_in_workspace -- --nocapture` against the smallest target that proves the change.

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
