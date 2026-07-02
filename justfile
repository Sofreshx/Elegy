toolchain := "1.96.0"

ci: fmt-check clippy test

fmt-check:
    cargo +{{toolchain}} fmt --all -- --check

clippy:
    cargo +{{toolchain}} clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo +{{toolchain}} test --workspace