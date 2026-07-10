set shell := ["bash", "-cu"]
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all-features

# Fast unit, integration, sandbox, PTY-harness, and fake-tool tests (no real TV).
test-fast:
    cargo test --all-features -- --test-threads=1

# OS-boundary contracts for fake just/Television processes and cable callbacks.
test-contract:
    cargo test --all-features --test tv_contract -- --test-threads=1
    cargo test --all-features --test tv_action_pty -- --test-threads=1
    cargo test --all-features --test tui_diagnostics -- --test-threads=1

# Run serialized real-Television behavioral workflows.
test-tui:
    cargo test --test tui_workflows -- --ignored --test-threads=1

# Reviewed 120x40 virtual-screen evidence; CI must never accept snapshots.
test-snapshots:
    INSTA_UPDATE=no cargo test --test tui_snapshots -- --ignored --test-threads=1

# Repeat the real interactive suite without retries hiding an individual run.
test-tui-soak runs="10":
    #!/usr/bin/env bash
    set -euo pipefail
    for run in $(seq 1 {{runs}}); do
        echo "jtv TUI soak run ${run}/{{runs}}"
        cargo test --test tui_workflows -- --ignored --test-threads=1
    done

# Prove a clean default release contains jtv but not the feature-gated fake tool.
verify-release-artifacts:
    cargo test --test release_artifacts default_release_excludes_test_helper -- --ignored --test-threads=1

check: fmt-check lint test

e2e:
    bash tests/e2e/run.sh

ci: check
    cargo build --release

# Complete local gate; requires pinned just/TV, Rust 1.85, and cargo-audit.
test-all: fmt-check lint test-fast test-contract
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
        Linux*) just test-snapshots; just test-tui ;;
        Darwin*) just test-tui ;;
        *) echo "real Television and canonical snapshots are not release gates on this platform" ;;
    esac
    just verify-release-artifacts
    cargo +1.85.0 check --locked --all-targets --all-features
    cargo audit
