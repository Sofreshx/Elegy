# Contributing to Elegy

Thanks for your interest in contributing.

`Elegy` is the main repository for the project's formalization, governance, MCP-facing analysis, and first-party Rust runtime work. The most valuable contributions right now are:

- keeping docs aligned with the active contracts-first and Rust-first repository shape
- improving clarity around package boundaries and dependency direction
- hardening the imported Rust runtime family and its contract-consumer posture
- tightening contributor ergonomics without widening scope casually

## First principles

Please keep these rules in mind:

1. **Be honest about current status.** Do not document commands, examples, or capabilities that do not exist yet.
2. **Respect the accepted direction.** `contracts/`, `governance/`, and `rust/` remain the canonical owned surfaces. The contributor-navigation overlays under `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are pointer shells only.
3. **Keep v1 intentionally narrow.** The current protocol/runtime target is Rust-first, runtime composition, resources-first MCP behavior, and conservative policy defaults.
4. **Prefer safe defaults.** Validation, policy, and security posture are core project behavior, not extras.
5. **Do not widen scope casually.** Changes that affect protocol scope, trust boundaries, packaging topology, or repo-split direction should start with an issue or design discussion.

## Before you start

Review:

- [README.md](README.md)
- [docs/architecture/README.md](docs/architecture/README.md)
- [docs/architecture/elegy-memory-v1.md](docs/architecture/elegy-memory-v1.md) when changing governed memory or repo-local non-authoritative skill-routing surfaces; keep the authority chain explicit and prefer `elegy-memory` command examples over the temporary `elegy` compatibility bridge
- [docs/spec-baseline.md](docs/spec-baseline.md)
- [SECURITY.md](SECURITY.md)

If you touch `src/Elegy-memory`, `src/Elegy-mcp`, or `src/Elegy-skills`, keep those paths documentation-only and route substantive authority or implementation changes back to `contracts/`, `governance/`, `rust/`, and the canonical docs. Treat `.github/skills/` only as repo-local non-authoritative contributor-routing output.
For larger changes, open an issue or draft PR early so maintainers can confirm the work still matches the accepted consolidation direction.

## What to work on now

Good contributions right now include:

- documentation corrections
- wording improvements that remove ambiguity
- package-boundary clarifications
- architecture cross-link fixes
- contributor-experience improvements that do not conflict with the locked direction
- runtime hardening or contract-conformance fixes inside the in-repo Rust workspace

## Local verification

Run the narrowest relevant checks for the surfaces you change.

### Contracts, governance, and workflow changes

Use targeted checks such as:

```powershell
pwsh ./scripts/validate-package-boundaries.ps1
pwsh ./scripts/export-contracts.ps1
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

### Rust runtime changes

Run from `rust/`:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

If your change only affects docs, workflows, or one language slice, explain the narrower validation scope in the PR.

## Pull request expectations

Please keep pull requests:

- small enough to review
- explicit about user-visible impact
- explicit about whether the change is docs, workflow/config, governed contracts, Rust, or transitional deletion work
- updated with docs when behavior or posture changes

Every PR should answer:

- What changed?
- Why is the change needed now?
- Does it change the accepted v1 scope or architecture?
- What follow-up work, if any, remains?

## Scope guardrails

These items are intentionally out of scope for the first release unless the project direction is changed explicitly:

- broad tools/prompts/sampling promises beyond the explicitly implemented MCP surface
- build-time generation as the primary runtime operating model
- write-capable adapters in the runtime family
- broad plugin/extensibility promises
- hosted platform claims
- generalized malware-detection claims

## Security-related contributions

If you believe you found a security issue, please follow [SECURITY.md](SECURITY.md) instead of opening a public bug report first.

## Communication and conduct

Please follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) in all project spaces.