# `elegy-documentation` — distribution

`elegy-documentation` is a repository documentation-governance tool. Its former
plugin manifest and archive lane are deprecated; release it only as a flat
binary.

```bash
cargo build -p elegy-documentation
cargo run -p elegy-documentation -- check --project . --json
cargo test -p elegy-documentation
```
