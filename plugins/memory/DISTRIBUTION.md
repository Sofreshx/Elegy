# `elegy-memory` — distribution

`elegy-memory` is a stateful retrieval product/tool over a scoped SQLite store.
Its salience, correction, contradiction, and consolidation behavior is domain
logic, so its former plugin manifest and archive lane are deprecated. The CLI
is released as a flat binary; stdio and HTTP MCP binaries are separate optional
host adapters.

```bash
cargo build -p elegy-memory
cargo run -p elegy-memory -- --help
cargo test -p elegy-memory
```
