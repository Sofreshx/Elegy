# `elegy-observe` — distribution

`elegy-observe` is an OS-observation CLI and an adapter candidate in `rework`.
Its former plugin manifest and archive lane are deprecated. It is released only
as a flat binary while a typed capability catalog and portable package evidence
are absent.

```bash
cargo build -p elegy-observe
cargo run -p elegy-observe -- --help
cargo test -p elegy-observe
```

Do not add it to the marketplace or export it as a plugin until
`distribution/surfaces.json` promotes it to an active adapter plugin.
