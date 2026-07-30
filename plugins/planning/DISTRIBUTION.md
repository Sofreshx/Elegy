# `elegy-planning` — distribution

`elegy-planning` is the durable planning product/tool over SQLite. Goals,
roadmaps, workflow state, leases, and evidence are domain behavior, not an
external-system adapter. Its former plugin manifest, marketplace entry, and
plugin archive lane are deprecated. Release the CLI only as a flat binary.

```bash
cargo build -p elegy-planning
cargo run -p elegy-planning -- --json version
cargo test -p elegy-planning
```

The capability catalog describes machine-invokable tool operations; it does not
change Planning's `surfaceClass`.
