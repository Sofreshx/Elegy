# Elegy Rust Workspace

## Ownership

- `rust/` owns reusable executable behavior over governed artifacts. It does not become the canonical authority for schemas, fixtures, compatibility policy, or discovery truth.
- Keep operator surfaces thin: binaries and host shells should stay wrappers over reusable library or tooling crates.
- When a Rust change affects an agent-visible capability, keep the Rust behavior aligned with the governed artifacts under `contracts/`.
- Prefer dedicated bounded CLIs for their owned lanes: `elegy-memory`, `elegy-mcp`, `elegy-planning`, `elegy-skills`, `elegy-configuration`, and `elegy-documentation`. Keep `elegy` as the umbrella compatibility/general surface.

## Design Rules

- Keep CLI errors ergonomic with `anyhow`; keep library errors typed with `thiserror`.
- Avoid `unwrap()` in library code. Use explicit error paths that preserve agent-facing failure context.
- Minimize new dependencies, especially in crates that feed CLI, MCP, or host surfaces.
- If a capability is exposed through both CLI and MCP, the behavior, metadata, dry-run semantics, and output envelopes should stay aligned.

## Review Focus

- Check agent-visible behavior, not only compile success: JSON envelopes, side-effect metadata, explicit `Unsupported` behavior, profile filtering, and dry-run paths.
- If a command is exposed through MCP, verify the CLI template and MCP projection stay aligned.
- Prefer local crate validation first; widen to workspace validation only when shared crates, contracts, profiles, or operator surfaces changed.
- For documentation behavior, prefer `elegy-documentation` for authority-aware inspect/map/check and treat umbrella `elegy docs ...` as the compatibility scaffold/index path.
