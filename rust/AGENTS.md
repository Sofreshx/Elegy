# Elegy Rust Workspace

## Commands

- Build one crate: `cargo build -p <crate>`
- Test one crate: `cargo test -p <crate>`
- Check the workspace when registry, profile, contract, or shared CLI behavior changed: `cargo test --workspace`
- Format Rust changes before handoff: `cargo fmt --all`

## Rust Boundaries

- Keep CLI errors ergonomic with `anyhow`; keep library errors typed with `thiserror`.
- Avoid `unwrap()` in library code. Use explicit error paths that preserve agent-facing failure context.
- Public APIs need contract-oriented doc comments because these crates are consumed through generated agent surfaces.
- Minimize new dependencies; contract and CLI behavior should not become harder for local agent hosts to install.

## Review Focus

- Check every new command or capability for JSON output, side-effect metadata, dry-run behavior, and profile filtering.
- If a command is exposed through MCP, verify the CLI template and MCP projection stay aligned.
