---
title: Host-Neutral Plugin Install Ergonomics
status: draft
owner: Elegy
created: 2026-06-10
updated: 2026-06-10
doc_kind: spec
summary: Predictable local install layout with command shims, idempotent PATH updates, and machine-readable receipt extensions for host-neutral plugin consumption.
---

## Problem

Before this spec, Elegy's distribution installer (`scripts/install-distribution.ps1`) extracts CLI binaries into `bin/<surface>/` and wrapper surfaces into `wrappers/<surface>/` but does nothing to surface these executables for invocation:
- No PATH mutation or shim directory is created.
- No machine-readable map from tool name to executable path is recorded beyond the flat `installedAssets` array in `install-receipt.json`.
- Each consumer — agent hosts, MCP projections, human operators — must independently locate binaries by convention or by full path.
- The Codex plugin ecosystem demonstrates mature practice here (explicit `bin/` layout with `shims/`, idempotent PATH registration, and install receipts as the authoritative tool-resolution contract), but Elegy lacks equivalent ergonomics.

Additionally, two smaller but real issues exist in the current receipt:
1. **Schema version drift**: The PowerShell installer writes `"schemaVersion": "1.0.0"` while the bash installer writes `"schemaVersion": "elegy-install-receipt/v1"`. These are semantically equivalent but syntactically inconsistent.
2. **No receipt JSON Schema**: Neither the distribution receipt nor the plugin-install receipt has a governed JSON Schema in `contracts/schemas/`.

## Goals

1. **Command shims (default opt-out).** After install, create a `bin/shims/` directory containing one thin wrapper (shim) per installed CLI surface. Each shim invokes the corresponding binary in `bin/<surface>/`. Shims are local to the install root — they do not modify the system.
2. **Install receipt extension.** Extend `install-receipt.json` with:
   - `commandShimRoot`: absolute path to `bin/shims/`.
   - `commandShims`: array of `{ toolName, shimPath, targetExecutablePath }` objects, one per installed CLI surface.
   - `pathUpdate`: records whether `-AddToPath` was used and what was modified (or `null` if not used).
3. **Idempotent PATH opt-in.** Add `-AddToPath` flag. When specified, append `bin/shims/` to the user-scoped `PATH` environment variable exactly once per install root. Subsequent runs with the same destination must not duplicate entries.
4. **`-NoCommandShims` flag.** Disables shim creation entirely. The receipt still records `commandShimRoot: null` and an empty `commandShims` array.
5. **Wrapper installer passthrough.** All 7 `src/Elegy-*/install.ps1` thin wrappers pass through `-AddToPath` and `-NoCommandShims` to the distribution installer.
6. **Tool resolution contract.** Document the following resolution order for agents and verifiers:
   1. Install receipt `commandShims[].shimPath`
   2. Install receipt `commandShims[].targetExecutablePath`
   3. Explicit user-provided `--bin-dir`
   4. PATH fallback
7. **Schema consistency (PowerShell).** Align `schemaVersion` in the PowerShell installer to `"1.1.0"` for the extended receipt format. Bash installer alignment is deferred until the working branch reconciles with `main` (where `scripts/install-distribution.sh` exists).
8. **Host neutrality.** Do not auto-register with any host marketplace, plugin library, or agent runtime during the generic Elegy install. Host-specific projections (Codex, OpenCode, Holon) consume the same receipt/shim contract.

## Non-Goals

- Do not create or modify system-wide PATH entries (`Machine` scope). Only `User` scope is touched by `-AddToPath`.
- Do not generate `*.cmd`, `*.bat`, or symlink-based shims on Windows. PowerShell shims (`.ps1`) are the only shim type on Windows.
- Do not generate shell-specific shims (e.g., bash aliases, fish functions) beyond the generic `bin/shims/` contract.
- Do not add a Plugin Install Receipt JSON Schema to `contracts/schemas/` in this spec (separate follow-up).
- Do not change the plugin-install receipt format (`ElegyPluginInstallReceiptV1` struct, defined on `main` in `rust/crates/elegy-contracts/src/lib.rs`).
- Do not change how the tool resolution function in `elegy-tooling` resolves binaries — it should consume the new receipt fields without needing to change its fallback logic. (On `main`, this function is `resolve_binary_path()` at `rust/crates/elegy-tooling/src/lib.rs`.)
- Do not add PATH deduplication logic to the bash installer in this spec (bash installer parity is a follow-up).
- Do not change the `elegy-cli` legacy compatibility path (`bin/cli/`).

## Behavior

### Shim Creation (Default)

On every install, after all assets are extracted and verified, the installer:
1. Creates `bin/shims/` under `$Destination`.
2. For each installed CLI surface, creates a shim wrapper at `bin/shims/<toolName>.ps1` (Windows) or `bin/shims/<toolName>` (Unix).
   - **Windows shim**: A minimal `.ps1` script that resolves the target executable relative to the shim's own install root and forwards all arguments: `& "<installRoot>\bin\<surface>\<binary>.exe" @args`.
   - **Unix shim**: A minimal shell script that resolves the target executable relative to the shim's own install root and forwards all arguments: `#!/bin/sh; exec "$INSTALL_ROOT/bin/<surface>/<binary>" "$@"`.
3. Records each shim in the receipt's `commandShims` array.

**Wrapper-only surfaces.** Wrapper-only surfaces (e.g., `elegy-obsidian`) with no corresponding CLI binary do not receive a shim. The shim count equals the count of installed CLI surfaces with binaries.

**Reinstall behavior.** If `bin/shims/` already exists from a prior install, existing shims are overwritten. Shims for CLI surfaces that are no longer in the current install request are NOT removed (no cleanup of stale shims). This keeps the default behavior simple and reversible — users can delete `bin/shims/` and re-run to get a clean set.

**Partial failure.** Shim creation happens AFTER all assets are verified and extracted, immediately before the receipt is written. If the installer fails before writing the receipt, orphaned shims may remain in `bin/shims/`. This is acceptable because: (a) shims are thin wrappers that reference concrete binary paths, (b) the absence of a receipt with matching `commandShimRoot` means consumers will not trust orphaned shims, and (c) a reinstall will overwrite them.

### Shim Opt-Out (`-NoCommandShims`)

When `-NoCommandShims` is specified:
- `bin/shims/` is not created.
- Existing `bin/shims/` from a prior install is left untouched (no cleanup).
- Receipt records `commandShimRoot: null` and `commandShims: []`.

### PATH Mutation (`-AddToPath`)

When `-AddToPath` is specified:
- **Windows**: Uses `[Environment]::SetEnvironmentVariable('PATH', ..., 'User')`. Before appending, parses the current user PATH and checks if the shim path is already present. If present, skips the write (idempotent).
- **Unix**: Appends `export PATH="$INSTALL_ROOT/bin/shims:$PATH"` to `~/.profile` (or `~/.bash_profile` if it exists). Checks for existing entry before appending. The receipt records `"scope": "profile"` with the file path that was modified.
- When `-AddToPath` is used together with `-NoCommandShims`, the installer emits a warning: "AddToPath requires command shims. Ignoring -AddToPath." and does not modify PATH.
- Receipt records `pathUpdate` with the target variable (`PATH` or `$PATH`), scope (`User` on Windows / `profile` on Unix with the file path), the appended path, and whether it was a no-op (already present).

### Default Install Output

Without `-AddToPath`, the installer prints the shim directory path and a message:
```
Command shims created at: C:\path\to\.elegy\bin\shims
Add this directory to your PATH to invoke Elegy tools directly:
  [Environment]::SetEnvironmentVariable('PATH', "$env:PATH;C:\path\to\.elegy\bin\shims", 'User')
Or re-run with -AddToPath.
```

### Receipt Extension (schemaVersion 1.1.0)

The receipt gains three new top-level keys after `installedAssets`:

```json
{
  "commandShimRoot": "C:\\Users\\...\\.elegy\\bin\\shims",
  "commandShims": [
    {
      "toolName": "elegy-planning",
      "shimPath": "C:\\Users\\...\\.elegy\\bin\\shims\\elegy-planning.ps1",
      "targetExecutablePath": "C:\\Users\\...\\.elegy\\bin\\elegy-planning\\elegy-planning.exe"
    }
  ],
  "pathUpdate": {
    "variable": "PATH",
    "scope": "User",
    "appendedPath": "C:\\Users\\...\\.elegy\\bin\\shims",
    "alreadyPresent": false
  }
}
```

When `-AddToPath` is not used (default install — shims created, no PATH mutation):
```json
{
  "commandShimRoot": "C:\\Users\\...\\.elegy\\bin\\shims",
  "commandShims": [
    {
      "toolName": "elegy-planning",
      "shimPath": "C:\\Users\\...\\.elegy\\bin\\shims\\elegy-planning.ps1",
      "targetExecutablePath": "C:\\Users\\...\\.elegy\\bin\\elegy-planning\\elegy-planning.exe"
    }
  ],
  "pathUpdate": null
}
```

When `-AddToPath` is not used, `pathUpdate` is `null`. When `-AddToPath` is used and PATH was already present, `alreadyPresent` is `true` and the write is skipped.

When `-NoCommandShims` is used:
```json
{
  "commandShimRoot": null,
  "commandShims": [],
  "pathUpdate": null
}
```

### Wrapper Installer Passthrough

Each `src/Elegy-*/install.ps1` adds two new parameters (`-AddToPath`, `-NoCommandShims`) and forwards them to the distribution installer's `-AddToPath` and `-NoCommandShims` switches.

### Host-Neutral Resolution Contract

The tool resolution order (for agents, MCP hosts, verification, and human operators):
1. Install receipt `commandShims[].shimPath` — if the receipt is available and shims exist.
2. Install receipt `commandShims[].targetExecutablePath` — direct binary path from receipt.
3. Explicit `--bin-dir` argument — user-provided convenience override.
4. PATH fallback — standard shell lookup.

This contract is documented in the install receipt and intended to be consumed by the tool resolution function in `elegy-tooling` (currently at `rust/crates/elegy-tooling/src/lib.rs` on `main`).

### Schema Version Alignment

The PowerShell installer (`install-distribution.ps1`) writes `"schemaVersion": "1.1.0"`. Bash installer alignment is deferred until the working branch reconciles with `main` (where `scripts/install-distribution.sh` exists).

## Acceptance Criteria

- [ ] AC 1: Default install (`.\install-distribution.ps1 -Destination C:\test-install`) creates `bin/shims/` with one `.ps1` shim per CLI surface, records them in `install-receipt.json`, and writes `schemaVersion: "1.1.0"`.
  → verify: Run default install, inspect `bin/shims/` for `.ps1` files, parse `install-receipt.json` and confirm `commandShimRoot`, `commandShims[]`, and `pathUpdate` fields exist. Confirm `jq .schemaVersion install-receipt.json` returns `"1.1.0"`.
- [ ] AC 2: `-NoCommandShims` skips shim creation; receipt records `commandShimRoot: null` and empty `commandShims`.
  → verify: Run `-NoCommandShims` install, confirm `bin/shims/` is absent, parse receipt and confirm null/empty values.
- [ ] AC 3: `-AddToPath` appends `bin/shims/` to user PATH exactly once; second run is a no-op.
  → verify: Run `-AddToPath` twice on the same destination; inspect `[Environment]::GetEnvironmentVariable('PATH', 'User')` and confirm exactly one occurrence of the shim path.
- [ ] AC 4: `-AddToPath` + `-NoCommandShims` together emit a warning and skip PATH mutation.
  → verify: Run with both flags, confirm warning text in output, confirm PATH is unchanged.
- [ ] AC 5: Wrapper installers (`src/Elegy-planning/install.ps1` etc.) pass through `-AddToPath` and `-NoCommandShims`.
  → verify: Run `.\src\Elegy-planning\install.ps1 -Destination C:\test-planning -AddToPath`, confirm shims are created and PATH is updated. Run with `-NoCommandShims`, confirm shims are skipped.
- [ ] AC 6: Existing distribution validation tests continue to pass.
  → verify: Run existing validation: `cargo test -p elegy-cli --test docs`, `cargo test -p elegy-cli docs`, `cargo fmt --all -- --check`, etc.
- [ ] AC 7: Tool resolution can find a tool from the install receipt's `commandShims` array.
  → verify: Unit test that loads a sample receipt with shims, calls the resolution function with `"elegy-planning"` and a receipt, and confirms it returns the shim path.

## Validation

- `cargo test -p elegy-cli --test docs`
- `cargo test -p elegy-cli docs`
- `cargo fmt --all -- --check`
- `cargo test -p elegy-tooling` (for new receipt-resolution tests)
- Manual: Run the full install flow with `-AddToPath`, `-NoCommandShims`, and default on Windows. Verify receipt JSON and PATH state.
- Manual: Run wrapper installers with new flags and verify passthrough.

### Validation Evidence (2026-06-10)

- **Syntax:** `scripts/install-distribution.ps1` and all 7 `src/Elegy-*/install.ps1` pass PowerShell AST parsing (no syntax errors).
- **Spec-fit review:** All 8 Goals, 5 edge cases, and 6 Non-Goals verified by code inspection — zero spec drift.
- **`cargo fmt --all -- --check`:** Pre-existing formatting drift in `rust/crates/elegy-planning/` unrelated to this change (no Rust files modified).
- **AC 1-5 (manual):** Pending — requires GitHub release artifacts or local build to exercise full install flow.
- **AC 7 (tool resolution):** Pending — deferred per spec Non-Goal (no `resolve_binary_path` changes on working branch; code exists on `main`).

## Links

- Distribution installer (PowerShell): `scripts/install-distribution.ps1`
- Distribution installer (bash): `scripts/install-distribution.sh` (exists on `main`; will resolve after branch reconciliation)
- Wrapper installers: `src/Elegy-memory/install.ps1`, `src/Elegy-planning/install.ps1`, `src/Elegy-skills/install.ps1`, `src/Elegy-configuration/install.ps1`, `src/Elegy-documentation/install.ps1`, `src/Elegy-mcp/install.ps1`, `src/Elegy-obsidian/install.ps1`
- Install receipt structure: `scripts/install-distribution.ps1:895-921`
- Binary resolution: `rust/crates/elegy-tooling/src/lib.rs:770-810` (exists on `main`; `resolve_binary_path()` will be verified after reconciliation)
- Plugin install receipt struct (`ElegyPluginInstallReceiptV1`): `rust/crates/elegy-contracts/src/lib.rs:944-959` (exists on `main`; will be verified after reconciliation)
- Plugin tool availability spec: `docs/specs/plugin-tool-availability.md` (exists on `main`; will resolve after branch reconciliation)
- Package installer: `scripts/package-installer.ps1`
- Contracts schema directory: `contracts/schemas/`
