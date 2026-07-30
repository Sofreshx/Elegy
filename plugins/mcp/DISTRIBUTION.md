# `elegy-mcp` — distribution

`elegy-mcp` is a development CLI for descriptor authoring and static analysis.
It does not connect to or host a live MCP server. Its former plugin manifest
and archive lane are deprecated; release it only as a flat tool binary.

```bash
cargo build -p elegy-mcp
cargo run -p elegy-mcp -- --help
cargo test -p elegy-mcp
```
