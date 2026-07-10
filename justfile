set shell := ["bash", "-cu"]

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all-features

check: fmt-check lint test

e2e:
    bash tests/e2e/run.sh

ci: check
    cargo build --release
