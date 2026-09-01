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
    INSTA_UPDATE=no cargo test --test tui_snapshots -- --ignored --skip patched_tv_preserves_semantic_styles_in_the_real_list_and_preview --test-threads=1

# Capability gate for an upstream/custom TV build that supports ANSI + display.
test-tv-ansi-display:
    #!/usr/bin/env bash
    set -euo pipefail
    : "${JTV_TEST_REAL_TV:?point this at the candidate patched tv binary}"
    JTV_TEST_TV_ANSI_DISPLAY=1 cargo test --test tui_snapshots patched_tv_preserves_semantic_styles_in_the_real_list_and_preview -- --ignored --test-threads=1

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

check-ci-prereqs:
    @command -v zsh >/dev/null || { echo "zsh is required for CI tests" >&2; exit 1; }
    sh -n install.sh

ci: check-ci-prereqs fmt-check lint test-fast test-contract
    cargo test --all-features --test pty_harness -- --test-threads=1
    cargo build --release

# Run the Linux CI gate in the pinned Ubuntu userland used for local parity checks.
ci-ubuntu:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v podman >/dev/null || { echo "podman is required for ci-ubuntu" >&2; exit 1; }
    podman build --platform linux/amd64 --tag localhost/jtv-ci-ubuntu:latest --file Containerfile.ci .
    podman run --rm --init --userns=keep-id:uid=1000,gid=1000 --user ubuntu \
      --workdir /workspace \
      --env CI=true \
      --env GITHUB_ACTIONS=true \
      --env CARGO_INCREMENTAL=0 \
      --env CARGO_TERM_COLOR=always \
      --env CARGO_HOME=/tmp/cargo \
      --env CARGO_TARGET_DIR=/tmp/target \
      --env LANG=C.UTF-8 \
      --volume "{{ justfile_directory() }}:/workspace:Z" \
      localhost/jtv-ci-ubuntu:latest just ci

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

build:
  cargo build --release

install:
  install -m 0755 target/release/jtv ~/.local/bin/jtv

# Validate a manual release without creating or pushing a tag.
release-preflight version:
    #!/usr/bin/env bash
    set -euo pipefail
    version={{ quote(version) }}
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "usage: just release 0.5.0" >&2; exit 2; }
    [[ "$(cargo pkgid --locked -p jtv)" == *"#${version}" ]] || { echo "Cargo.toml version does not match ${version}" >&2; exit 1; }
    [[ -z "$(git status --porcelain)" ]] || { echo "working tree must be clean" >&2; exit 1; }
    ! git rev-parse --verify --quiet "refs/tags/v${version}" >/dev/null || { echo "tag v${version} already exists locally" >&2; exit 1; }
    [[ -z "$(git ls-remote --tags origin "refs/tags/v${version}")" ]] || { echo "tag v${version} already exists on origin" >&2; exit 1; }
    just ci

release-dry-run version: (release-preflight version)
    @echo "release v{{version}} preflight passed; no tag or push was created"

# After updating and committing Cargo.toml/Cargo.lock, tag and trigger the release workflow.
release version: (release-preflight version)
    #!/usr/bin/env bash
    set -euo pipefail
    version={{ quote(version) }}
    git tag "v${version}"
    git push origin "v${version}"
