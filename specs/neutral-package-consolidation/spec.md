---
spec_id: neutral-package-consolidation
title: Neutral Plugin And Distribution Consolidation
status: draft
type: migration
updated: 2026-06-09
---

# Neutral Plugin And Distribution Consolidation

## Intent

Consolidate Elegy around one neutral portable package contract. Remove the V1/V2 schema split and remove Holon-specific language from schemas, fixtures, Rust code, docs, and README. Use a single schema file `contracts/schemas/elegy-plugin-package.schema.json` with `schemaVersion: "elegy-plugin-package/v1"`. Add a Bash installer for Linux/macOS parity, tighten the README, update PACKAGE_README, and apply crates.io publish policy.

## Context Evidence

### Problem justification

- **V1/V2 split is maintenance deadweight**: Two package schemas with overlapping identity/component shapes force every consumer (Rust validators, CLI tools, fixture authors, docs writers) to branch on version. The V1 schema is a proper subset of V2 — every V1 fixture already validates against a liberalized V2 shape. There is no consumer that actively requires V1-only semantics.
- **Holon was a placeholder host target**: The `marketplaceTarget: "holon"` enum, `HostTarget::Holon` variant, and Holon-specific validation rules exist in code but no Holon marketplace integration ships or is planned. The "Holon" references in fixtures (`hostCompatibility`, `policyTags`, `description`) are aspirational labels, not functional contracts. Removing them eliminates a vestigial host dependency.
- **No downsteam consumer depends on V2 exclusivity**: The `elegy-configuration` crate is the only internal consumer that gates on `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION` — it does so only to ensure configuration components are present, not because it needs V2-specific shape guarantees. All V2 fixtures can carry the same content under `elegy-plugin-package/v1` without semantic loss.
- **Future evolution should happen in one file**: Adding a field to both V1 and V2 schemas today requires two edits and two fixture migrations. A single schema makes every capability available to every fixture by default, with optional fields gating adoption.

### Affected artifacts

- `contracts/schemas/elegy-plugin-package-v1.schema.json`: Current V1 schema. Uses `schemaVersion: "elegy-plugin-package/v1"`. Contains: identity, metadata (no subsetOf), components (skillDefinitions, instructionSkills, mcpProjections, capabilityProjections, docs, assets), hostPolicyHints. No configuration/profiles, no publishing, no hostCompatibility.
- `contracts/schemas/elegy-plugin-package-v2.schema.json`: Current V2 schema. Uses `schemaVersion: "elegy-plugin-package/v2"`. Superset of V1 plus: configurationTemplates/Profiles, capabilityContracts, evalPacks, resourcePacks, toolAdapterContracts, bridgeAdapterContracts, cliHelpers, pilotingAdapters, fixturePacks, toolRequirements, hostCompatibility, publishing (with `marketplaceTarget` enum `["holon"]`), and metadata.subsetOf.
- `contracts/fixtures/elegy-plugin-package-v1.*.json` (3 files): V1 fixtures with `schemaVersion: "elegy-plugin-package/v1"`, no Holon content.
- `contracts/fixtures/elegy-plugin-package-v2.*.json` (7 files): V2 fixtures. `v2.elegy-planning.json` and `v2.elegy-skills.json` carry Holon-specific description language, `"holon"` tags, and `"holon-compatible"` policy tags. `v2.elegy-planning.json` has `hostCompatibility` entries for `"holon"`. Three negative fixtures exist for subset/side-effect/phantom testing.
- `rust/crates/elegy-contracts/src/lib.rs`: Constants `ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION` and `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION`. `validate_elegy_plugin_package()` checks both versions and enforces V2-only component gates. Publishing validation enforces Holon-specific rules when `marketplaceTarget == "holon"`.
- `rust/crates/elegy-tooling/src/lib.rs`: `HostTarget` enum includes `Holon` variant. `from_str("holon")` maps to `HostTarget::Holon`. `project_plugin_for_host()` dispatches Holon to `project_generic_host_plugin()`. Contains inline test JSON strings with hardcoded `"elegy-plugin-package/v1"`/`"v2"` schema versions.
- `rust/crates/elegy-configuration/src/lib.rs`: Imports `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION`, gates package loading on exact V2 match. Contains inline test JSON with V2 schema version.
- `rust/crates/elegy-contracts/tests/conformance.rs`: Asserts V2 schema presence, loads V2 fixture with version assertion, dedicated `v2_holon_packages_*` test.
- `docs/distribution.md`: "Holon-oriented quick start" section. "write Holon-specific configuration" language.
- `README.md`: References `elegy-plugin-package-v2.demo-config.json` in configuration examples. Contains long package matrix tables.
- `PACKAGE_README.md`: Archive-level README. Lists archive families and targets. No Holon language currently.
- `docs/specs/plugin-tool-availability.md`: Existing spec that uses `elegy-plugin-package/v2` as its authority surface. This consolidation spec does not supersede it but will require co-update (see Drift Notes).
- `governance/canonical-output-inventory.json`: Registers v1 and v2 schemas and fixtures as canonical outputs. Must be updated.
- No Bash install script exists yet. Only PowerShell installer at `scripts/install-distribution.ps1`.
- 28 Cargo.toml files under `rust/` lack `publish` field (default to `publish = true`).

## Requirements

### R1: Single Unified Package Schema
- Delete `contracts/schemas/elegy-plugin-package-v1.schema.json`
- Delete `contracts/schemas/elegy-plugin-package-v2.schema.json`
- Create `contracts/schemas/elegy-plugin-package.schema.json` with `schemaVersion` const `"elegy-plugin-package/v1"`, carrying all useful V2 capabilities: configuration templates/profiles, capability contracts, eval packs, resource packs, tool/bridge adapter contracts, CLI helpers, piloting adapters, fixture packs, MCP projections, capability projections, skill definitions, instruction skills, docs, assets, tool requirements, host compatibility, and neutral publishing/provenance metadata.
- **SchemaVersion rationale**: The unified schema reuses the version string `"elegy-plugin-package/v1"` because old V1 fixtures are a strict subset of the new schema — they already carry this version string and will remain valid under the unified schema (all new fields are optional, `additionalProperties: false` only rejects unknown keys). Old V2 fixtures require only a `schemaVersion` value change from `"v2"` to `"v1"` plus Holon-content removal. No consumer has ever depended on V2-exclusive semantics that would be lost by this migration.
- **marketplaceTarget**: Remove `"holon"` from the enum. Change the field type from `enum: ["holon"]` to `type: "string", minLength: 1` so any downstream host can declare a publishing target without schema edits. If no existing fixture uses `marketplaceTarget`, make the field fully optional with no enum restriction.

### R2: Holon Removal
- Remove `"holon"` from all schema enums, fixture values, and Rust code.
- Replace "Holon package" and "Holon-oriented" language with "portable package", "host package", or "downstream host package" in all docs and fixtures.
- Remove `HostTarget::Holon` variant from Rust enum. `from_str("holon")` should NOT parse — return an error.
- Remove Holon-specific publishing validation (lines 4133-4199 in `elegy-contracts/src/lib.rs`).
- Keep neutral source/provenance fields intact: `sourceRepository`, `sourceRef`, `sourceCommit`, `changelogRef`, `provenanceRef`, `signatureRefs`, and generic `compatibility` notes.

### R3: Fixture Migration
- Rename V1 fixtures:
  - `elegy-plugin-package-v1.minimal.json` → `elegy-plugin-package.minimal.json`
  - `elegy-plugin-package-v1.elegy-planning.json` → `elegy-plugin-package.elegy-planning.json`
  - `elegy-plugin-package-v1.elegy-skills.json` → `elegy-plugin-package.elegy-skills.json`
- Rename V2 fixtures (remove V2 suffix, update `schemaVersion` to `"elegy-plugin-package/v1"`, remove Holon content):
  - `elegy-plugin-package-v2.minimal.json` → **overwrite** `elegy-plugin-package.minimal.json` (V2 minimal is a superset of V1 minimal: it adds `configurationTemplates` and `configurationProfiles` alongside the same identity/metadata shape)
  - `elegy-plugin-package-v2.demo-config.json` → `elegy-plugin-package.demo-config.json`
  - `elegy-plugin-package-v2.elegy-planning.json` → **overwrite** `elegy-plugin-package.elegy-planning.json` (V2 planning has all V1 capability projections plus toolRequirements, hostCompatibility, and subsetOf — after Holon removal, it is the authoritative fixture)
  - `elegy-plugin-package-v2.elegy-skills.json` → **overwrite** `elegy-plugin-package.elegy-skills.json` (V2 skills has all V1 capability projections plus toolRequirements and hostCompatibility — after Holon removal, it is the authoritative fixture)
  - `elegy-plugin-package-v2.negative-missing-subset-marker.json` → `elegy-plugin-package.negative-missing-subset-marker.json`
  - `elegy-plugin-package-v2.negative-side-effect-loosening.json` → `elegy-plugin-package.negative-side-effect-loosening.json`
  - `elegy-plugin-package-v2.negative-phantom-capability.json` → `elegy-plugin-package.negative-phantom-capability.json`
- Configuration-only package fixtures (demo-config) must remain valid without `skillDefinitions`.
- Update `governance/canonical-output-inventory.json` to reference renamed fixtures and schema.

### R4: Rust Code Consolidation
- Remove `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION` constant from `elegy-contracts/src/lib.rs`.
- Simplify `validate_elegy_plugin_package()` to accept only `"elegy-plugin-package/v1"` as valid `schemaVersion`. Remove all V2-only component gates: configurationTemplates, configurationProfiles, capabilityContracts, evalPacks, resourcePacks, toolAdapterContracts, bridgeAdapterContracts, cliHelpers, publishing, toolRequirements, and hostCompatibility are now always valid.
- Remove `load_elegy_plugin_package_v2_fixture_from_dir()` function.
- Remove `HostTarget::Holon` variant from `elegy-tooling/src/lib.rs` and its `from_str` mapping. Update the valid-options error message.
- Remove Holon-specific publishing validation (`marketplaceTarget == "holon"` gate and all associated required-field checks for sourceRepository, sourceRef, sourceCommit, license, changelogRef, provenanceRef, signatureRefs, compatibility).
- Update `elegy-configuration/src/lib.rs`: switch from `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION` to `ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION` in the package version guard, update inline test JSON strings.
- Update all test code: `elegy-contracts/tests/conformance.rs` (remove V2 schema assertions, V2 fixture loading, V1/V2 separation test, Holon-specific test), `elegy-cli/tests/authoring.rs` (update fixture paths and schema version strings in inline JSON), `elegy-configuration/tests/cli.rs` (update fixture paths), `elegy-tooling/src/lib.rs` (update inline test JSON schema versions).

### R5: README And Documentation Consolidation
- Make root `README.md` concise: what Elegy is, install commands, supported targets, main CLI surfaces, links to distribution and architecture docs.
- Move long package matrices and maintainer workflow detail from README into `docs/distribution.md`.
- Remove Holon-specific language from `docs/distribution.md` (the "Holon-oriented quick start" section).
- Update `PACKAGE_README.md` to reflect the single package schema and installer surfaces.
- Update any `elegy-plugin-package-v2.demo-config.json` references in docs to `elegy-plugin-package.demo-config.json`.

### R6: Bash Installer Parity
- Create a Bash installer at `scripts/install-distribution.sh` using the same manifest/checksum model as the PowerShell installer.
- Supported on Linux (x86_64-unknown-linux-gnu) and macOS (aarch64-apple-darwin).
- Must download/validate manifest and checksums, verify SHA-256, extract archives, and write `install-receipt.json`.

### R7: Crates.io Publish Policy
- Add `publish = false` to all crate Cargo.toml files except:
  - `elegy-contracts`
  - `elegy-memory`
  - `elegy-planning`
  - `elegy-skills`
- Keep Rust crate version `0.1.x` independent from bundle/release asset versions.

## Non-Goals

- No change to `contracts/schemas/elegy-plugin-readiness-v1.schema.json` (readiness receipt stays independent).
- No change to the `metadata.subsetOf` shape — it remains a flat `Vec<String>` in both schema and Rust code (the structured-object gap noted in `docs/specs/plugin-tool-availability.md` is out of scope).
- No new archive families or distribution lanes beyond the Bash installer.
- No changes to MCP projection, Mermaid tooling, Obsidian CLI, or observe/desktop surfaces.
- No removal of the Codex plugin projection (Codex remains as a host target, only Holon is removed).
- No content changes to `docs/specs/plugin-tool-availability.md` in this spec slice — its V2 references will be addressed in a follow-up (see Drift Notes).
- Scope bundling rationale: The Bash installer (R6) and crates.io policy (R7) are bundled in this spec because they share the same artifact-set context as the schema consolidation and would otherwise require three separate spec-review-plan-implement cycles for what is effectively a single "cleanup and parity" release.

## Acceptance Checks

- AC1: Schema consolidation — V1/V2 files deleted, unified schema created with correct version and no Holon enum
  → verify: `Test-Path "contracts/schemas/elegy-plugin-package-v1.schema.json"` returns `$false`; `Test-Path "contracts/schemas/elegy-plugin-package-v2.schema.json"` returns `$false`; `Test-Path "contracts/schemas/elegy-plugin-package.schema.json"` returns `$true`; the file's `schemaVersion.const` equals `"elegy-plugin-package/v1"` and `publishing.marketplaceTarget` has type `"string"` with no `"enum"` containing `"holon"`.

- AC2: All positive fixtures validate against unified schema
  → verify: `cargo test -p elegy-contracts --test conformance` passes with no V2-specific assertions. A schema conformance test loads each renamed positive fixture and validates against `elegy-plugin-package.schema.json`.

- AC3: Negative fixtures expected-fail
  → verify: `cargo test -p elegy-cli --test authoring` passes. Expected-failure tests confirm that `negative-missing-subset-marker.json`, `negative-side-effect-loosening.json`, and `negative-phantom-capability.json` produce appropriate validation errors.

- AC4: CLI plugin verify passes over all positive fixtures
  → verify: `cargo test -p elegy-cli --test authoring plugin_verify` passes. Manual CLI: `elegy plugin verify --package ./contracts/fixtures/elegy-plugin-package.minimal.json --json` returns `"ready"` status.

- AC5: No Holon language in schema, fixtures, or docs
  → verify: `rg -i "holon" contracts/schemas/elegy-plugin-package.schema.json contracts/fixtures/elegy-plugin-package.*.json docs/distribution.md README.md PACKAGE_README.md` returns no matches. `rg "HostTarget::Holon\|HostTarget \{.*Holon" rust/ --include '*.rs'` returns no matches.

- AC6: Crates.io policy enforced — only allowed crates are publishable
  → verify: `rg '^publish\s*=\s*true' rust/crates/*/Cargo.toml` returns no matches (no crate explicitly sets `publish = true`). `rg '^publish\s*=\s*false' rust/crates/*/Cargo.toml` returns exactly one match per crate except `elegy-contracts`, `elegy-memory`, `elegy-planning`, `elegy-skills` (which omit `publish` entirely). Allowed crates can be confirmed with: `rg -L '^publish' rust/crates/{elegy-contracts,elegy-memory,elegy-planning,elegy-skills}/Cargo.toml`.

- AC7: Governance inventory updated
  → verify: `rg "elegy-plugin-package-v[12]" governance/canonical-output-inventory.json` returns no matches. `rg "elegy-plugin-package\\.(schema|minimal|demo-config|elegy-planning|elegy-skills|negative)" governance/canonical-output-inventory.json` returns matches for all renamed artifacts.

- AC8: README and docs cleanup
  → verify: `(Get-Content README.md | Measure-Object -Line).Lines` is less than 200. README has no "Which Download To Use" table or "Direct release asset families" bullet list (moved to distribution.md). `rg "Holon-oriented" docs/distribution.md` returns no matches. `rg "elegy-plugin-package-v[12]" README.md PACKAGE_README.md` returns no matches.

- AC9: Canonical output validation passes
  → verify: `pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs` passes after `pwsh ./scripts/export-contracts.ps1`.

- AC10: Bash installer creates valid install receipt
  → verify: `bash ./scripts/install-distribution.sh -LocalArtifactsRoot ./artifacts/distribution -Destination /tmp/elegy-test -CliSurfaces elegy-cli -Force` succeeds, produces `/tmp/elegy-test/install-receipt.json` with correct structure (schemaVersion, installed assets, verification evidence).

- AC11: Crate versions remain independent from bundle versions
  → verify: `rg '^version\s*=' rust/crates/{elegy-contracts,elegy-memory,elegy-planning,elegy-skills}/Cargo.toml` shows `0.1.x` for all four. No Rust crate version is bumped to match bundle version during this migration.

## Implementation Links

- `contracts/schemas/elegy-plugin-package-v1.schema.json` (DELETE)
- `contracts/schemas/elegy-plugin-package-v2.schema.json` (DELETE)
- `contracts/schemas/elegy-plugin-package.schema.json` (CREATE — unified schema)
- `contracts/fixtures/elegy-plugin-package*.json` (10 files: 3 rename from V1, 7 rename from V2 with content updates)
- `governance/canonical-output-inventory.json` (UPDATE — replace V1/V2 entries with unified)
- `rust/crates/elegy-contracts/src/lib.rs` (UPDATE — simplify validation, remove V2 constant/loader/Holon publishing validation)
- `rust/crates/elegy-contracts/tests/conformance.rs` (UPDATE — remove V2/Holon assertions, update fixture paths/schema names)
- `rust/crates/elegy-tooling/src/lib.rs` (UPDATE — remove HostTarget::Holon, update inline test JSON)
- `rust/crates/elegy-cli/tests/authoring.rs` (UPDATE — fixture paths, inline schema versions)
- `rust/crates/elegy-configuration/src/lib.rs` (UPDATE — switch V2→V1 schema version guard, update inline test JSON)
- `rust/crates/elegy-configuration/tests/cli.rs` (UPDATE — fixture paths)
- `rust/crates/*/Cargo.toml` (UPDATE — add `publish = false` to all except 4)
- `README.md` (UPDATE — consolidate, move package matrix to distribution.md)
- `PACKAGE_README.md` (UPDATE — remove V1/V2 references)
- `docs/distribution.md` (UPDATE — remove Holon language, absorb package matrix from README)
- `scripts/install-distribution.sh` (CREATE — Bash installer parity)
- Spec plan: `specs/neutral-package-consolidation/plan.md` (to follow in Phase 2)

## Validation Evidence

- Pending implementation.

## Drift Notes

- Cross-spec relationship: `docs/specs/plugin-tool-availability.md` uses `elegy-plugin-package/v2` as its authority surface. This consolidation spec does not supersede it — the tool-availability spec will need a co-update in a follow-up slice to switch its references from `v2` to the unified schema. That spec's Drift Notes already acknowledge its own `subsetOf` schema gap; the co-update is a natural opportunity to address both.
- No ADR is required for the schema version choice: `"elegy-plugin-package/v1"` is reused because (a) old V1 fixtures are a strict subset of the unified schema and remain valid, (b) no consumer depends on V2-exclusive semantics that would be lost, (c) the file rename from `elegy-plugin-package-v1.schema.json` to `elegy-plugin-package.schema.json` makes the break visually obvious. A future major shape change would use `"elegy-plugin-package/v2"` in the same file.
