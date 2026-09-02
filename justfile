toolchain := "1.96.1"
exe := if os() == "windows" { ".exe" } else { "" }

ci: fmt-check clippy test

fmt-check:
    cargo +{{toolchain}} fmt --all -- --check

clippy:
    cargo +{{toolchain}} clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo +{{toolchain}} test --workspace

install-mcp:
    cargo +{{toolchain}} build --release -p elegy-memory-mcp --bin elegy-memory-mcp-stdio -p elegy-host-mcp --bin elegy-run
    mkdir -p ~/.elegy/bin
    cp target/release/elegy-memory-mcp-stdio{{exe}} ~/.elegy/bin/
    cp target/release/elegy-run{{exe}} ~/.elegy/bin/
