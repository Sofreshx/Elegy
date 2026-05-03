# Elegy Codex Guidance

## Start Here

- Read `README.md`, then the smallest relevant doc under `docs/`.
- Use `docs/agent-integration.md` before changing host onboarding, discovery, invocation envelopes, MCP projection, or agent-facing JSON.
- Treat `contracts/fixtures/skill-definition-v2.*.json` as the governed capability authority; do not reintroduce v1 `skill-definition.*.json`.
- Prefer `elegy agent check/manifest/discover --json` for host onboarding work and `elegy skills list/search/describe --json` for raw registry work.

## Review Discipline

- For non-trivial changes, state the smallest safe plan before editing.
- Before handoff, challenge the change for contract drift, unsafe side effects, stale docs, and missing validation.
- When capability behavior changes, verify both the Rust implementation and the governed fixture/projection that exposes it to agents.

## Safety Boundaries

- MCP is an adapter over governed skills and CLI behavior, not the primary authority.
- Side-effecting MCP tools remain blocked unless a call is dry-run or the host is explicitly started with `--allow-side-effects`.
- Profiles are allowlists, not approvals. A visible capability still needs host policy before side effects.
- Mermaid reverse projection is bounded analysis; do not describe it as canonical workflow reconstruction.

## Validation

- Run validation from `rust/` unless the task is docs-only.
- Prefer the narrowest command that covers the changed crate, then broaden only when contracts or shared registry behavior changed.
- If docs or fixtures changed without code, validate the relevant generated/discovery surface rather than only proofreading Markdown.
