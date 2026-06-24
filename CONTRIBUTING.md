# Contributing to Elegy

Thanks for your interest in contributing.

`Elegy` is the main repository for the project's formalization, MCP-facing analysis, and first-party Rust runtime work. The most valuable contributions right now are:

- keeping docs aligned with the active contracts-first and Rust-first repository shape
- improving clarity around package boundaries and dependency direction
- hardening the imported Rust runtime family and its contract-consumer posture
- tightening contributor ergonomics without widening scope casually

## First principles

Please keep these rules in mind:

1. **Be honest about current status.** Do not document commands, examples, or capabilities that do not exist yet.
2. **Respect the accepted direction.** `plugins/`, `shared/`, and `hosts/` remain the canonical owned surfaces.
3. **Keep v1 intentionally narrow.** The current protocol/runtime target is Rust-first, runtime composition, resources-first MCP behavior, and conservative policy defaults.
4. **Prefer safe defaults.** Validation, policy, and security posture are core project behavior, not extras.
5. **Do not widen scope casually.** Changes that affect protocol scope, trust boundaries, packaging topology, or repo-split direction should start with an issue or design discussion.

## Before you start

Review:

- [README.md](README.md)
- [docs/architecture/README.md](docs/architecture/README.md)
- [plugins/memory/docs/architecture/v1.md](plugins/memory/docs/architecture/v1.md) when changing governed memory or repo-local non-authoritative skill-routing surfaces; keep the authority chain explicit and prefer `elegy-memory` command examples over the temporary `elegy` compatibility bridge
- [docs/spec-baseline.md](docs/spec-baseline.md)
- [SECURITY.md](SECURITY.md)

Treat `.github/skills/` only as repo-local non-authoritative contributor-routing output.
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

### Git hooks

This repo ships pre-push and pre-commit hooks under `.githooks/`. To enable them locally, run from the repo root:

```bash
git config core.hooksPath .githooks
```

The pre-push hook runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, the `elegy-documentation` check, and `cargo test --workspace --all-targets --all-features`. The pre-commit hook runs the fast `cargo fmt --all --check`.

### Contracts and workflow changes

Use targeted checks such as:

```bash
cargo run -p elegy-core --bin elegy-contracts -- contracts validate --project .
cargo test -p elegy-contracts --test conformance
```

### Rust runtime changes

Run from the repo root:

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