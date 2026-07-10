# Testing jtv

`jtv` is tested as both a command-line program and an interactive terminal
application. Correctness does not depend on a human reading raw ANSI output: a
native PTY drives real keystrokes, a VT parser reconstructs the visible 120x40
character-cell screen, and fixture event logs prove what was actually executed.

## Test architecture

The suite is deliberately layered:

1. Unit and integration tests cover Justfile JSON parsing, parameter semantics,
   safe argv construction, cleanup, queue policy, and channel installation.
2. Fake-tool contracts run `jtv` across a real OS process boundary. The
   feature-gated `jtv-test-tool` impersonates `just` and `tv` and records argv,
   cwd, selected environment, callbacks, statuses, and malformed responses.
3. PTY harness tests verify key delivery, resizing, alternate-screen parsing,
   fragmented Unicode, timeouts, bounded diagnostics, interruption, and child
   reaping without depending on Television.
4. Real-TV workflows drive Television 0.15.9 with named keys and semantic,
   deadline-bounded screen waits. Assertions pair visible state with exact
   append-only fixture events and cleanup checks.
5. A small Linux snapshot set records reviewed, normalized screens and stable
   non-default VT style runs. Semantic assertions remain authoritative;
   snapshots make presentation drift visible.

Escape cancels Television-owned recipe and nested-picker screens. Dialoguer-owned
scalar and secret prompts use Ctrl-C cancellation; both paths are exercised and
must leave no execution or temporary state behind.

The reusable harness lives under `tests/support/`. On failure it can write only
sanitized diagnostics beneath `target/jtv-test-artifacts/<scenario>/`: metadata,
event history, normalized screen, and a bounded transcript. Temporary homes,
runtime state, cable configuration, and projects are isolated per scenario.

## Prerequisites

- Rust 1.85 or newer; CI also compiles all targets with exactly Rust 1.85.0.
- `just` 1.53.0 for real workflows.
- Television (`tv`) 0.15.9 for real workflows and snapshots.
- `cargo-audit` for the complete local gate.

Install the pinned tools with:

```sh
cargo install just --version 1.53.0 --locked
cargo install television --version 0.15.9 --locked
cargo install cargo-audit --locked
rustup toolchain install 1.85.0
```

The test-only dependencies are pinned to `portable-pty` 0.9.0, `vt100` 0.16.2,
and `insta` 1.48.0. These versions retain the project's Rust 1.85 MSRV while
providing native PTYs, character-cell reconstruction, and reviewed snapshots.

## Commands

```sh
just test-fast       # unit/integration, sandbox, PTY harness, and fake contracts
just test-contract   # fake just/TV process-boundary contracts only
just test-tui        # all ignored real-TV workflows, serialized
just test-snapshots  # canonical snapshots with updates forbidden
just test-tv-ansi-display # candidate TV ANSI+display cell-style gate
just test-tui-soak   # ten complete real-TV runs; accepts another run count
just verify-release-artifacts # clean release excludes the fake test helper
just test-all        # local release gate, including MSRV and RustSec audit
```

`cargo test` does not run ignored real-tool tests accidentally. The explicit TUI
commands fail with actionable tool/version diagnostics when their prerequisites
are absent. `tests/e2e/run.sh` is retained as a thin compatibility entry point
for the Rust PTY suite; it no longer depends on Expect or timing sleeps.

### Television ANSI + display compatibility

Television 0.15.9 treats `[source] ansi = true` and a separate `display` template
as incompatible: emitted SGR bytes can become visible text. jtv must retain the
display template because action callbacks consume only opaque IDs, never recipe
labels. At runtime jtv therefore falls back to plain source rows for every
unverified TV build, while retaining icons, text markers, structured previews, and safe
selection behavior.

The repository has a local, opt-in capability gate for an upstream candidate:

```sh
JTV_TEST_REAL_TV=/absolute/path/to/patched/tv just test-tv-ansi-display
```

The test launches the real TUI and requires an ANSI-16 semantic color to survive
into the final VT cells. Merely stripping escape bytes while preserving text is
a failure. It also compares the reviewed
`patched_tv_colored_source_120x40` text-and-style snapshot, filters the list,
and reaches the parameter prompt through the opaque action ID.
`JTV_UNSAFE_TV_ANSI_DISPLAY=1` is for a deliberately selected custom build;
normal CI never sets it. Do not raise the minimum TV version or remove the
fallback until an upstream release passes this gate.

Optional `bat` preview highlighting is Unix-only. Its tests reject hostile,
oversized, and invalid output and kill the helper process group on timeout.
Windows intentionally uses the internal renderer rather than risking an
unbounded descendant that inherited the preview pipe.

### Expect parity map

| Former Expect behavior | Rust PTY replacement |
|---|---|
| Simple selection, preview, confirmation, and execution | `browse_filter_visible_preview_and_run` plus reviewed browser/preview snapshots |
| Definition cycling and cache behavior | `cycles_to_faithful_definition_and_runs_the_explicit_dry_run_action`, the Definition snapshot, and `television_caches_repeated_definition_previews` |
| Escape cancellation | `escape_cancels_root_nested_and_confirmation` |
| Ctrl-C and subsequent shell command | `ctrl_c_root_and_nested_leave_no_state` and `terminal_is_usable_after_interrupt` |
| Literal shell metacharacters | `scalar_defaults_alias_module_and_variadics` |
| Choice and file pickers | `nested_choice_boolean_file_and_directory_pickers` |
| Secret redaction | `secret_is_hidden_and_confirmation_is_redacted` plus the redacted confirmation snapshot |
| Three selections, stop on failure, exit 7 | `multi_select_is_deterministic_and_stops_on_first_failure` |
| Temporary/history-file absence | Every workflow's `assert_clean` lifecycle check |

The cache workflow traces every real `just` invocation and asserts the complete
four-process set: one version probe, one compatibility dump, one project dump,
and one cached Definition. Thus launch never starts a subprocess per recipe,
and cycling away from and back to the same Definition invokes `just --show`
only once through Television's preview cache.

## Failure artifacts and secret safety

When reproducing a failure, use the scenario/harness preservation option so its
`FailureArtifacts` bundle is retained. Read `metadata.txt` first, then
`screen.txt`, `events.tsv`, and the bounded `transcript.txt`. Screen text is
normalized to replace sandbox paths and volatile identifiers.

Never add a secret through `send_text`; use the harness's secret-input method so
the event is represented only as `SecretInput`. Every scenario that handles a
secret registers a unique sentinel. Artifact creation scans every proposed file
and fails rather than writing or uploading that sentinel. Command confirmations,
screen frames, transcripts, snapshots, and fake-tool diagnostics must contain
`[REDACTED]`, never the value. CI uploads only `target/jtv-test-artifacts`, never
the scenario's temporary home or raw process state.

## Snapshot review

Normal tests and CI set `INSTA_UPDATE=no`; they cannot bless changed UI. To review
an intentional change locally:

```sh
INSTA_UPDATE=new cargo test --test tui_snapshots -- --ignored --skip patched_tv_preserves_semantic_styles_in_the_real_list_and_preview --test-threads=1
cargo insta review
just test-snapshots
```

Inspect every `.snap.new` for meaningful content, stable normalization, accidental
absolute paths, identifiers, and secrets before accepting it. Canonical snapshots
use Linux at 120x40 plus a reviewed 80x24 plain/ASCII state, recording both the
visible grid and non-default cell-style runs. The rapid multi-mark state records
the grid only because TV 0.15.9 emits nondeterministic partial modifier deltas;
its three marks and identities are asserted semantically. macOS and Windows still run semantic screen
tests; platform-specific glyph/layout differences are not hidden to force one
dishonest golden image.

## CI platform matrix

| Gate | Linux | macOS | Windows |
|---|---:|---:|---:|
| Format, Clippy, build, fast tests | required | required | required |
| Native PTY harness | required | required | required |
| Fake-tool contracts | required | required | required |
| Real-TV browse/preview/run | required | required smoke | capability probe |
| Nested picker/cancel/terminal recovery | required | required smoke | capability probe |
| Canonical snapshots and short soak | required | — | — |
| Rust 1.85 and RustSec audit | required | — | — |

The Windows Television probe is explicitly nonblocking. It establishes whether
the pinned binary builds and starts; it is not a claim of supported Windows
interactive behavior. Windows PTY primitives and process contracts remain hard
gates. Required Windows real-TV behavior should be introduced only with an
accepted product-support contract and real workflow tests.

## Flake policy

- Synchronize on semantic screen/process conditions with bounded deadlines; do
  not add arbitrary sleeps to make a scenario pass.
- Tests are serialized when they own a terminal or process-global signal state.
- A retry must never turn red into green. `test-tui-soak` reports the exact failing
  iteration and stops immediately.
- Reproduce a failure with the same pinned binaries, viewport, locale, and test
  filter. Use the sanitized artifact bundle to distinguish a product regression,
  harness defect, platform difference, or genuinely insufficient deadline.
- Fix or quarantine a confirmed platform issue with an owner and explanation;
  never loosen assertions or refresh snapshots solely because output is unstable.

## Optional human exploratory session

Manual exploration assesses usability, color, ergonomics, and discoverability;
it is not a correctness gate.

1. Create an isolated temporary project and set temporary `HOME`,
   `XDG_CONFIG_HOME`, and `TMPDIR` values. Add a Justfile and `.jtv.toml` without
   real credentials.
2. Build `jtv`, then run `jtv init` and `jtv doctor` with pinned `just` and `tv`.
3. Browse and filter recipes. Inspect short, long, and parameterized previews;
   resize the terminal and assess focus, selection, colors, Unicode, and overflow.
4. Exercise choice/path pickers, ordinary and secret prompts, multi-selection,
   confirmation decline, Escape, and Ctrl-C. Confirm that no secret is visible.
5. Run a normal shell command afterward and inspect the temporary runtime/config
   directories for leaked session, picker, or legacy history files.
6. Record observations. An asciinema capture or screenshot is optional, must use
   synthetic values, and must be reviewed for secrets and host paths.
7. Remove the isolated project, home, configuration, runtime, and recording.

Every objective correctness observation above has an automated equivalent. File
an issue for subjective findings rather than encoding fragile pixel assertions.
