# Neutral Plugin And Distribution Consolidation — Implementation Plan

> **Status: historical.** This plan is an archive of the implementation steps
> that consolidated Elegy's V1/V2 schema split. The work is done. The current
> schema at `contracts/schemas/elegy-plugin-package.schema.json` is the single
> unified `elegy-plugin-package/v1`. See
> [ADR](../../docs/adr/2026-06-16-elegy-plugin-package-v1-unification.md)
> for the current decision record.

## Overview

This plan implements the signed-off spec at `specs/neutral-package-consolidation/spec.md`. It consolidates Elegy around one neutral portable package contract, removes the V1/V2 schema split, removes Holon-specific language, and applies crates.io publish policy.

**Six phases, 29 numbered steps:**

| Phase | Focus | Steps | Requirements |
|-------|-------|-------|-------------|
| 1 | Schema and Fixture Consolidation | 1–4 | R1, R2, R3 |
| 2 | Crates.io Publish Policy | 5 | R7 |
| 3 | Rust Code Consolidation | 6–9 | R4 |
| 4 | Documentation Cleanup | 10–12 | R5 |
| 5 | Bash Installer | 13 | R6 |
| 6 | Validation | 14–29 | All ACs |

---

## Phase 1: Schema and Fixture Consolidation (R1, R2, R3)

### Step 1 — Create unified schema file

**File:** `contracts/schemas/elegy-plugin-package.schema.json` (CREATE)

- Merge all V2 capabilities into the V1 shape:
  - `schemaVersion` const: `"elegy-plugin-package/v1"`
  - Identity block (name, version, description, authors, homepage, repository, license)
  - `metadata` object including `subsetOf` (from V2) as `type: "array", items: { type: "string" }`, optional
  - `components` with all V1 types: `skillDefinitions`, `instructionSkills`, `mcpProjections`, `capabilityProjections`, `docs`, `assets`
  - Plus all V2 component types (all optional): `configurationTemplates`, `configurationProfiles`, `capabilityContracts`, `evalPacks`, `resourcePacks`, `toolAdapterContracts`, `bridgeAdapterContracts`, `cliHelpers`, `pilotingAdapters`, `fixturePacks`, `toolRequirements`, `hostCompatibility`
  - `publishing` object (from V2) with: `marketplaceTarget` as `type: "string", minLength: 1` (NOT an enum), plus `sourceRepository`, `sourceRef`, `sourceCommit`, `changelogRef`, `provenanceRef`, `signatureRefs`, `compatibility`
  - `hostPolicyHints` (from V1)
- `additionalProperties: false` at schema root
- No `"holon"` enum values anywhere

### Step 2 — Delete old V1 and V2 schema files

**Files:**
- `contracts/schemas/elegy-plugin-package-v1.schema.json` (DELETE)
- `contracts/schemas/elegy-plugin-package-v2.schema.json` (DELETE)

### Step 3 — Rename and update all 10 fixtures

> **Ordering constraint:** Step 3a MUST run before Step 3b because 3b overwrites the V1-renamed files created by 3a.

**3a — Rename V1 fixtures (no content change beyond filename):**

| Old filename | New filename |
|---|---|
| `contracts/fixtures/elegy-plugin-package-v1.minimal.json` | `contracts/fixtures/elegy-plugin-package.minimal.json` |
| `contracts/fixtures/elegy-plugin-package-v1.elegy-planning.json` | `contracts/fixtures/elegy-plugin-package.elegy-planning.json` |
| `contracts/fixtures/elegy-plugin-package-v1.elegy-skills.json` | `contracts/fixtures/elegy-plugin-package.elegy-skills.json` |

**3b — Rename V2 fixtures + update `schemaVersion` to `"elegy-plugin-package/v1"` + remove Holon content:**

| Old filename | New filename | Actions |
|---|---|---|
| `contracts/fixtures/elegy-plugin-package-v2.minimal.json` | **Overwrite** `elegy-plugin-package.minimal.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. V2 minimal is a superset of V1 minimal (adds `configurationTemplates`/`configurationProfiles`). Remove any Holon content. |
| `contracts/fixtures/elegy-plugin-package-v2.demo-config.json` | `contracts/fixtures/elegy-plugin-package.demo-config.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. Remove any Holon content. Must remain valid without `skillDefinitions`. |
| `contracts/fixtures/elegy-plugin-package-v2.elegy-planning.json` | **Overwrite** `elegy-plugin-package.elegy-planning.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. Remove Holon content (`hostCompatibility` entries for `"holon"`, `"holon"` policy tags, `"holon-compatible"` tags). Keep `toolRequirements`, `hostCompatibility` for non-Holon hosts, `subsetOf`. |
| `contracts/fixtures/elegy-plugin-package-v2.elegy-skills.json` | **Overwrite** `elegy-plugin-package.elegy-skills.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. Remove Holon-specific description language, `"holon"` tags, `"holon-compatible"` policy tags. |
| `contracts/fixtures/elegy-plugin-package-v2.negative-missing-subset-marker.json` | `contracts/fixtures/elegy-plugin-package.negative-missing-subset-marker.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. Remove Holon content. Keep negative-test semantics. |
| `contracts/fixtures/elegy-plugin-package-v2.negative-side-effect-loosening.json` | `contracts/fixtures/elegy-plugin-package.negative-side-effect-loosening.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. Remove Holon content. Keep negative-test semantics. |
| `contracts/fixtures/elegy-plugin-package-v2.negative-phantom-capability.json` | `contracts/fixtures/elegy-plugin-package.negative-phantom-capability.json` | Change `schemaVersion` to `"elegy-plugin-package/v1"`. Remove Holon content. Keep negative-test semantics. |

### Step 4 — Update canonical output inventory

**File:** `governance/canonical-output-inventory.json` (UPDATE)

- Remove all entries referencing `elegy-plugin-package-v1.*` and `elegy-plugin-package-v2.*`
- Add entries for:
  - `contracts/schemas/elegy-plugin-package.schema.json`
  - `contracts/fixtures/elegy-plugin-package.minimal.json`
  - `contracts/fixtures/elegy-plugin-package.demo-config.json`
  - `contracts/fixtures/elegy-plugin-package.elegy-planning.json`
  - `contracts/fixtures/elegy-plugin-package.elegy-skills.json`
  - `contracts/fixtures/elegy-plugin-package.negative-missing-subset-marker.json`
  - `contracts/fixtures/elegy-plugin-package.negative-side-effect-loosening.json`
  - `contracts/fixtures/elegy-plugin-package.negative-phantom-capability.json`

---

## Phase 2: Crates.io Publish Policy (R7)

### Step 5 — Add `publish = false` to non-publishable crates

**Files:** All 28 `rust/crates/*/Cargo.toml` files (UPDATE)

- Add `publish = false` to all crate `[package]` sections **except** the four keep-publishable crates:
  - `rust/crates/elegy-contracts/Cargo.toml` — leave `publish` unset (defaults to `true`)
  - `rust/crates/elegy-memory/Cargo.toml` — leave `publish` unset
  - `rust/crates/elegy-planning/Cargo.toml` — leave `publish` unset
  - `rust/crates/elegy-skills/Cargo.toml` — leave `publish` unset
- The 24 crates that get `publish = false`:
  - elegy-adapter-fs, elegy-adapter-http, elegy-agent-events, elegy-cli, elegy-configuration, elegy-core, elegy-data, elegy-descriptor, elegy-desktop, elegy-desktop-win32, elegy-diagram, elegy-documentation, elegy-host-mcp, elegy-mcp, elegy-memory-mcp, elegy-mermaid, elegy-notify, elegy-observe, elegy-observe-win32, elegy-policy, elegy-repo, elegy-runtime, elegy-tooling, elegy-web

---

## Phase 3: Rust Code Consolidation (R4)

### Step 6 — Simplify `elegy-contracts/src/lib.rs`

**File:** `rust/crates/elegy-contracts/src/lib.rs` (UPDATE)

- Remove `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION` constant
- Simplify `validate_elegy_plugin_package()` to accept only `"elegy-plugin-package/v1"` as valid `schemaVersion`
- Remove all V2-only component gates (validation that rejects `configurationTemplates`, `configurationProfiles`, `capabilityContracts`, `evalPacks`, `resourcePacks`, `toolAdapterContracts`, `bridgeAdapterContracts`, `cliHelpers`, `publishing`, `toolRequirements`, `hostCompatibility` when schemaVersion is V1) — these components are now always valid. Note: `pilotingAdapters` and `fixturePacks` exist in the V2 JSON schema but have no corresponding Rust struct fields or validation gates; no code removal is needed for them.
- Remove `load_elegy_plugin_package_v2_fixture_from_dir()` function
- Remove Holon-specific publishing validation (the `marketplaceTarget == "holon"` gate and all its required-field checks for `sourceRepository`, `sourceRef`, `sourceCommit`, `license`, `changelogRef`, `provenanceRef`, `signatureRefs`, `compatibility`)

### Step 7 — Update `elegy-tooling/src/lib.rs`

**File:** `rust/crates/elegy-tooling/src/lib.rs` (UPDATE)

- Remove `HostTarget::Holon` variant from the enum
- Remove the `from_str("holon")` → `HostTarget::Holon` mapping
- Update the valid-options error message to exclude "holon"
- Update inline test JSON strings that currently use `"elegy-plugin-package/v2"` to use `"elegy-plugin-package/v1"` (5 instances at approx. lines 1036, 2683, 2772, 2830, 2905). Strings already using `"elegy-plugin-package/v1"` require no change.

### Step 8 — Update `elegy-configuration/src/lib.rs`

**File:** `rust/crates/elegy-configuration/src/lib.rs` (UPDATE)

- Switch import from `ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION` to `ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION`
- Update inline test JSON strings from `"elegy-plugin-package/v2"` to `"elegy-plugin-package/v1"`

### Step 9 — Update all test files

**9a — `rust/crates/elegy-contracts/tests/conformance.rs`** (UPDATE)
- Remove V2 schema presence assertions
- Update fixture loading to use renamed/unified fixture paths (e.g., `elegy-plugin-package-v2.minimal.json` → `elegy-plugin-package.minimal.json`)
- Remove V1/V2 separation test
- Remove Holon-specific test (`v2_holon_packages_*`)
- Remove any Holon-related assertions
- Update all fixture path references from `v1.*`/`v2.*` to unified names

**9b — `rust/crates/elegy-cli/tests/authoring.rs`** (UPDATE)
- Update fixture path references from `-v1.*`/`-v2.*` to unified names
- Update inline `schemaVersion` strings from `"v2"` to `"v1"`

**9c — `rust/crates/elegy-configuration/tests/cli.rs`** (UPDATE)
- Update fixture path references from `-v2.*` to unified names

---

## Phase 4: Documentation Cleanup (R5)

### Step 10 — Update `docs/distribution.md`

**File:** `docs/distribution.md` (UPDATE)

- Remove "Holon-oriented quick start" section header and all its content
- Replace "Holon" references with neutral language:
  - "Holon package" → "portable package" or "host package"
  - "Holon-oriented" → "downstream host"
- Absorb the package matrix table from README (the "Which Download To Use" table) — place within the existing "Asset model" section, after the "What most consumers should download" subsection.
- Absorb the "Direct release asset families" bullet list from README — place within the existing "Asset model" section, near the existing asset family list.
- Update any `elegy-plugin-package-v2.demo-config.json` references to `elegy-plugin-package.demo-config.json`

### Step 11 — Rewrite `README.md`

**File:** `README.md` (UPDATE)

- Rewrite to be concise (<200 lines total)
- Keep:
  - What Elegy is (brief project description)
  - Install commands (pointers to both installers)
  - Supported targets table
  - Main CLI surfaces table (elegy, elegy-mcp, elegy-memory, elegy-planning, elegy-skills)
  - Links to `docs/distribution.md`, architecture docs
- Remove:
  - "Which Download To Use" package matrix table (moved to `distribution.md`)
  - "Direct release asset families" bullet list (moved to `distribution.md`)
  - Long maintainer workflow details
- Update any `elegy-plugin-package-v2.demo-config.json` references to `elegy-plugin-package.demo-config.json`
- Remove any Holon language

### Step 12 — Update `PACKAGE_README.md`

**File:** `PACKAGE_README.md` (UPDATE)

- Remove any V1/V2 references
- Ensure it reflects the unified package schema
- Remove any Holon language

---

## Phase 5: Bash Installer (R6)

### Step 13 — Create Bash installer

**File:** `scripts/install-distribution.sh` (CREATE)

- Implement using the same manifest/checksum model as `scripts/install-distribution.ps1`
- Required capabilities:
  - Download manifest and checksums first (use `curl` or `wget`)
  - Verify SHA-256 against checksums: use `sha256sum` on Linux, `shasum -a 256` on macOS (or detect platform at runtime and use the appropriate tool). Add a dependency-check preamble that fails early if the required tools (`curl`/`wget`, `unzip`, and the sha tool) are missing.
  - Extract zip archives (use `unzip`)
  - Write `install-receipt.json` with correct structure (schemaVersion, installed assets, verification evidence)
- Supported flags (mirroring the PowerShell installer):
  - `-Tag` — release tag to install
  - `-LocalArtifactsRoot` — path to local artifacts (for offline/dev installs)
  - `-Destination` — installation target directory
  - `-CliSurfaces` — comma-separated CLI surface list
  - `-WrapperSurfaces` — comma-separated wrapper surface list
  - `-Force` — overwrite existing installation
- Supported targets: Linux (`x86_64-unknown-linux-gnu`), macOS (`aarch64-apple-darwin`)
- Must produce valid `install-receipt.json`

---

## Phase 6: Validation (all ACs)

### Step 14 — Rust formatting and lint

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: No formatting or clippy errors introduced by the migration.

### Step 15 — Compilation check

```bash
cargo check --workspace --all-targets --all-features
```

Expected: Entire workspace compiles with no errors.

### Step 16 — Run conformance tests (AC2)

```bash
cargo test -p elegy-contracts --test conformance
```

Expected: All conformance tests pass. All renamed positive fixtures validate against unified schema. V2 assertions removed.

### Step 17 — Run authoring tests (AC3, AC4)

```bash
cargo test -p elegy-cli --test authoring
```

Expected: All authoring tests pass. Negative fixtures produce appropriate validation errors. Updated fixture paths resolve correctly.

### Step 18 — Run configuration CLI tests

```bash
cargo test -p elegy-configuration --test cli
```

Expected: All configuration CLI tests pass. Updated V1 schema version guard works correctly.

### Step 19 — Run full cargo test suite

```bash
cargo test --workspace --all-targets --all-features
```

Expected: Entire workspace compiles and all tests pass with no regressions.

### Step 20 — Verify AC1 (schema content)

```bash
rg '"elegy-plugin-package/v1"' contracts/schemas/elegy-plugin-package.schema.json
rg '"holon"' contracts/schemas/elegy-plugin-package.schema.json
```

Expected: First `rg` finds `"const": "elegy-plugin-package/v1"` for schemaVersion. Second `rg` returns no matches (no `"holon"` enum anywhere in the unified schema). Also confirm: `Test-Path "contracts/schemas/elegy-plugin-package-v1.schema.json"` and `Test-Path "...-v2.schema.json"` both return `$false`.

### Step 21 — Verify AC5 (Holon removal)

```bash
rg -i "holon" contracts/schemas/ contracts/fixtures/elegy-plugin-package.*.json docs/distribution.md README.md PACKAGE_README.md
rg "HostTarget::Holon" rust/ --include '*.rs'
```

Expected: No matches in schema, fixtures, or docs. No `HostTarget::Holon` references in Rust code.

### Step 22 — Verify AC6 (crates.io policy)

```bash
rg '^publish\s*=\s*true' rust/crates/*/Cargo.toml
rg -L '^publish' rust/crates/{elegy-contracts,elegy-memory,elegy-planning,elegy-skills}/Cargo.toml
```

Expected: First `rg` returns no matches (no crate explicitly sets `publish = true`). Second `rg` returns exactly 4 files (the 4 keep-publishable crates which omit `publish` entirely, defaulting to `true`).

### Step 23 — Verify AC7 (governance inventory)

```bash
rg "elegy-plugin-package-v[12]" governance/canonical-output-inventory.json
rg "elegy-plugin-package\.(schema|minimal|demo-config|elegy-planning|elegy-skills|negative)" governance/canonical-output-inventory.json
```

Expected: First `rg` returns no matches (old V1/V2 references removed). Second `rg` returns matches for all 8 renamed artifacts (schema + 7 fixtures).

### Step 24 — Verify AC8 (README and docs cleanup)

```bash
wc -l README.md
rg "Which Download To Use|Direct release asset families" README.md
rg "Holon-oriented" docs/distribution.md
rg "elegy-plugin-package-v[12]" README.md PACKAGE_README.md
```

Expected: README line count < 200. Second `rg` returns no matches (package matrix moved to distribution.md). Third `rg` returns no matches (Holon section removed). Fourth `rg` returns no matches (no V1/V2 references).

### Step 25 — Verify AC9 (canonical output validation)

```bash
pwsh ./scripts/export-contracts.ps1
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

Expected: Passes. Governance inventory entries match renamed artifacts on disk.

### Step 26 — Verify AC10 (Bash installer receipt)

```bash
bash ./scripts/install-distribution.sh -LocalArtifactsRoot ./artifacts/distribution -Destination /tmp/elegy-test -CliSurfaces elegy-cli -Force
cat /tmp/elegy-test/install-receipt.json | python3 -m json.tool
```

Expected: Installer succeeds, produces `/tmp/elegy-test/install-receipt.json` with correct structure (schemaVersion, installed assets, verification evidence fields).

### Step 27 — Verify AC11 (crate versions)

```bash
rg '^version\s*=' rust/crates/{elegy-contracts,elegy-memory,elegy-planning,elegy-skills}/Cargo.toml
```

Expected: All four show `0.1.x` (or `version.workspace = true` with workspace root at `0.1.x`). No Rust crate version was bumped to match bundle version.

### Step 28 — Verify V1/V2 references fully removed (AC7 follow-up)

```bash
rg "elegy-plugin-package-v[12]" contracts/fixtures/ contracts/schemas/ docs/ README.md PACKAGE_README.md governance/canonical-output-inventory.json
```

Expected: No matches. All references use unified `elegy-plugin-package.*` names.

### Step 29 — Cross-spec drift note

**Manual check:** `docs/specs/plugin-tool-availability.md` still references `elegy-plugin-package/v2`. Per the parent spec's Drift Notes, this is a follow-up task. Log an issue or tracking note for the co-update (not blocking for this migration).

---

## Summary of Files Changed

| Action | Count | File Patterns |
|--------|-------|---------------|
| CREATE | 3 | `contracts/schemas/elegy-plugin-package.schema.json`, `specs/neutral-package-consolidation/plan.md`, `scripts/install-distribution.sh` |
| DELETE | 2 | `contracts/schemas/elegy-plugin-package-v1.schema.json`, `contracts/schemas/elegy-plugin-package-v2.schema.json` |
| RENAME | 7 | V1 and V2 fixtures (3 overwritten by V2 counterparts) |
| UPDATE | 10+ | Governance inventory, 4 Rust source files, 3 test files, 3 doc files, 24 Cargo.toml files |
