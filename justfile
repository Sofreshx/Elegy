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
    bash scripts/mcp-install-receipt.sh write ~/.elegy/bin ~/.elegy/bin/elegy-memory-mcp-stdio{{exe}} ~/.elegy/bin/elegy-run{{exe}}

# Compares the binaries in ~/.elegy/bin against a fresh workspace build and
# the install-receipt.json written by install-mcp, so a stale local MCP
# install (e.g. the one that shipped a dead elegy-run for ~10 days without
# anyone noticing) fails loudly instead of silently.
check-mcp-install:
    cargo +{{toolchain}} build --release -p elegy-memory-mcp --bin elegy-memory-mcp-stdio -p elegy-host-mcp --bin elegy-run
    bash scripts/mcp-install-receipt.sh check ~/.elegy/bin target/release/elegy-memory-mcp-stdio{{exe}} target/release/elegy-run{{exe}}
