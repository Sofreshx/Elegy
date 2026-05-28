# Elegy-configuration V1

## Purpose

`elegy-configuration` is the deterministic materialization lane for agent-facing
repo, workspace, and home assets that sit above installed Elegy distributions
and below product-specific bootstrap or runtime wiring.

## Current split

- governed template, profile, and receipt truth lives under `contracts/`
- `rust/crates/elegy-configuration` owns reusable `list`, `show`, `apply`, and
  `verify` behavior over those governed artifacts
- `elegy configuration ...` on the umbrella CLI remains the compatibility surface
- `elegy-configuration` is the dedicated operator surface and dedicated release target for this lane
- `src/Elegy-configuration/` is the thin wrapper surface for bounded downstream installs

## Boundary

What this lane owns:

- deterministic application and verification of governed templates and profiles
- deterministic application and verification of governed configuration carried by local `elegy-plugin-package/v2` package files
- small built-in materialization slices for repo skill mirrors, repo-local
  OpenCode configuration, repo-local Codex skill mirrors, and bounded Codex
  home setup
- deterministic operations such as `copyFile`, `copyDirectory`,
  `mirrorDirectory`, `patchTextBlock`, `mergeJson`, and `patchTomlBlock`
- first-class handling of assets such as skill mirrors, instructions, MCP
  config, hooks, agents, and support files when a template declares them

What this lane does not own:

- release-tag selection, published target selection, archive download, checksum
  verification, or extraction
- the generic distribution installer in `scripts/install-distribution.ps1`
- host-specific bootstrap, auth, policy, approvals, state, orchestration, or
  startup wiring in consuming repos
- promotion of `.github/skills` or any other path into a universal authority;
  template bindings choose defaults and callers may override them

## Current built-ins

Built-in templates:

- `repo-skill-mirror-minimal`
- `repo-opencode-agentic-minimal`
- `codex-home-minimal`

Built-in profiles:

- `repo-opencode-minimal`
- `repo-codex-minimal`

These built-ins are intentionally small and deterministic. They demonstrate the
materialization boundary without absorbing downstream product bootstrap.

## Operator examples

```bash
elegy configuration list --json
elegy configuration show --template-id repo-opencode-agentic-minimal --json
elegy configuration apply --profile-id repo-opencode-minimal --target . --dry-run --json
elegy configuration verify --profile-id repo-opencode-minimal --target . --json
elegy-configuration list --json
elegy-configuration apply --package ./contracts/fixtures/elegy-plugin-package-v2.demo-config.json --profile-id demo-profile --target . --dry-run --json
```

Bindings remain explicit and overrideable:

```bash
elegy configuration apply --template-id repo-skill-mirror-minimal --target . --binding authority.skills=.github/skills --binding target.skills=.agents/skills --json
```

## Verification posture

- `apply` emits a deterministic receipt describing the materialized subject and
  verification outcome
- `apply --dry-run` emits the same receipt family with preview actions and no writes
- `verify` checks the target against the same template or profile plus bindings
- package-backed applies and verifies use `sourceKind: "package"` and resolve configuration components relative to the local package file
- schemas and fixtures under `contracts/schemas/` and `contracts/fixtures/`
  remain the durable contract truth for this lane

Use this lane to materialize governed agentic assets. Use the release installer
to place Elegy itself on disk. Keep host-local runtime behavior in the consumer.
