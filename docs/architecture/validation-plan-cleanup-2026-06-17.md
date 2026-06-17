# Validation Plan

This is the validation work to run after the current cleanup pass when a
machine with Rust toolchain, PowerShell, and the existing CI is available.

## 1. Rust unit and integration tests

```bash
cd rust
cargo test -p elegy-cli --test docs
cargo test -p elegy-documentation
cargo test -p elegy-tooling
cargo test -p elegy-cli docs
```

The new `elegy-plugin-package.elegy-doc-practices.json` fixture must validate
against `contracts/schemas/elegy-plugin-package.schema.json`. The new
`skill.elegy-doc-practices.json` must validate against
`contracts/schemas/skill.schema.json`.

A parity check using `elegy plugin verify` should be added as a follow-up to
the new fixture.

## 2. Schema validation for new fixtures

```bash
cd rust
cargo run -p elegy-cli -- plugin verify --package ../contracts/fixtures/elegy-plugin-package.elegy-doc-practices.json --json
```

Should return `ready` or `partial` (no blocker; partial may reflect missing
companion skill binary on host).

## 3. Documentation objective check

```bash
cd rust
cargo run -p elegy-cli -- docs check --project . --json
```

After migrating `.elegy/docs.yaml` to V2 the `docs check` should:

- Load the V2 config without warnings.
- Detect that the new `skill.elegy-doc-practices.json` is governed
  (no issue expected).
- Treat `docs/specs/obsidian-skill-and-cli.md` as a local exception (no
  issue expected for that file).
- Re-emit `docs/docs-index.md` against the V2 config.

## 4. Documentation index regeneration

```bash
cd rust
cargo run -p elegy-cli -- docs index --project . --json
```

Should regenerate `docs/docs-index.md` reflecting:

- the new plugin package and skill fixtures
- the deleted `architecture-tradeoffs.md` (no longer listed)
- the rewritten `elegy-plugin-readiness.md` (now references the V1
  unification ADR as its primary decision)
- all architecture, ADR, and spec docs under configured authority roots
- V2 config metadata (schemaVersion, authorityRoots, etc.)

## 5. Canonical output validation

```powershell
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

Confirms exported contract bundle is current and `.elegy/docs.yaml` is part
of the generated outputs.

## 6. CI workflow alignment

The `.github/workflows/rust-ci.yml` lists `.elegy/docs.yaml` as a tracked
path. After the V2 migration the file format is still YAML, so the workflow
trigger remains valid.

The new `contracts/fixtures/elegy-plugin-package.elegy-doc-practices.json`
and `contracts/fixtures/skill.elegy-doc-practices.json` are added under
`contracts/fixtures/` and should appear in any `contracts/**` glob used by
CI for validation.

## 7. Plugin package discoverability

The new plugin package must be discoverable through `elegy-skills` or
`elegy plugin export`. The skill fixture should appear in the built-in
registry after the V1 unification flow is run end to end.

## 8. Manual smoke test for the central doctrine

Open `skills/elegy-doc-practices/SKILL.md` and verify that the doctrine
file matches what `docs/adr/2026-05-25-centralize-documentation-practices-doctrine.md`
describes. The acceptance criteria in
`docs/specs/documentation-practices-skill-and-cli.md` should all be
checked:

- [x] A central `skills/elegy-doc-practices/` package exists with `SKILL.md`,
  doctrine references, assets, eval fixtures, and adoption examples.
- [x] `elegy docs init/new/check/index` are implemented on the umbrella CLI.
- [x] Objective validation catches invalid metadata, invalid statuses, filename
  mismatches, missing required headings, and broken internal links.
- [x] Repo-local `.elegy/docs.yaml` overrides work for non-default ADR/spec/index paths.
- [x] Repo docs include phase-based enforcement guidance: PR checklist first,
  advisory CI second, blocking only for objective failures later.
