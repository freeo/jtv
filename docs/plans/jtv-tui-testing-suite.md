# Goal Plan: Complete Interactive TUI Testing Suite for `jtv`

Proposed outcome: Replace manual confidence checks with a deterministic, layered test system that drives real Television through a PTY, reconstructs what a user sees, verifies process contracts and side effects, preserves a small visual baseline, and leaves manual testing only for subjective usability.

Execution readiness: **Ready**

Discovery confidence: **High**

BDD: **Used** — the change verifies user-visible TUI workflows, cancellation and interruption state transitions, nested pickers, secret handling, and multi-recipe execution.

## User Problem Trace

### User's stated goal

- Implement the complete CLI/TUI testing pyramid described in the current conversation for `jtv`.
- Make interactive testing automated enough that correctness no longer depends on the old manual test procedure.
- Exercise the real Television interface rather than treating the TUI as a black box.
- Use only `gpt-5.6-terra` for every implementation and review subagent; the orchestrator chooses thinking level and may spend as much effort as needed.

### Reported friction, constraints, and gaps

| Symptom or constraint | Plan response |
|---|---|
| The old Bash version depended on manual testing. | AC3, AC5, and Phase 3 automate real keyboard workflows, side effects, cancellation, signal handling, and terminal recovery. |
| Raw TUI output looks like ANSI control sequences rather than a stable screen. | AC1 and AC4 introduce a fixed-size PTY plus VT parser, exposing both raw transcripts and reconstructed screen frames. |
| A TUI may appear visually correct while invoking the wrong command. | AC2 and AC3 pair screen assertions with fake-process contracts and exact fixture side effects. |
| Existing preview verification instruments the callback instead of asserting the final screen. | AC4 and Phase 4 verify the preview in the reconstructed Television screen; protocol instrumentation remains a lower-level diagnostic, not the visual oracle. |
| Fixed sleeps and raw byte matching are timing-sensitive. | AC1 requires condition-based waits with deadlines, screen-settle detection, and diagnostic capture on timeout. |
| Secrets and temporary paths can leak into transcripts or snapshots. | AC5 requires redacted input events, artifact scanning, isolated temporary homes, and cleanup assertions. |
| Linux Expect currently owns all real-TUI coverage. | AC1 and AC6 move the harness into Rust, certify PTY primitives on Linux/macOS/Windows, run required real-TV coverage on Linux and macOS, and probe Windows without claiming unsupported product behavior. |
| Tests must remain compatible with Rust 1.85, `just` 1.53.0, and Television 0.15.9. | Phase 0 gates all proposed dev dependencies against MSRV and keeps external tool versions pinned in CI. |
| All subagents must use the requested model. | The Subagent Plan mandates exactly `gpt-5.6-terra`; high thinking is used for PTY, screen, workflow, and review work, medium for bounded contract/docs work; no subagent may use xhigh. |

### Non-goals and scope boundaries

- Do not replace Television with a `jtv`-owned TUI or test Television's internal implementation.
- Do not make pixel-perfect screenshots across fonts, terminal emulators, or operating systems a merge gate. The stable visual contract is a normalized character-cell screen.
- Do not snapshot every interaction or every terminal cell; semantic assertions remain primary and snapshots cover only high-value states.
- Do not change public CLI, `.jtv.toml`, cable-channel, or session contracts merely to simplify tests. A small internal process-launch seam is allowed only if black-box testing proves insufficient and introduces no public behavior.
- Do not keep a manual checklist as a correctness gate. It covers feel, discoverability, color, and ergonomics only.
- Do not silently promote Windows interactive support to a release guarantee. Windows PTY and fake-tool tests are required; real Television on Windows starts as an explicit capability probe because the existing application plan deferred Windows interactive certification.
- Do not delete the archived `jtv-0.3.0` scripts.

## Acceptance Criteria

### AC1 — Deterministic cross-platform terminal harness

**Explicit.** Tests can launch a command in an isolated pseudo-terminal with a fixed viewport, send text and named keys, resize the terminal, wait for semantic screen conditions with bounded deadlines, reconstruct the current character-cell screen, collect exit status, and always terminate/reap children. Failures emit a sanitized transcript, final screen, event history, dimensions, command metadata, and temporary artifact path. Harness self-tests pass on Linux, macOS, and Windows without fixed `sleep` calls as the synchronization mechanism.

### AC2 — Exact process and cable-protocol contracts

**Inferred — Safe to execute.** A test-only fake Television/`just` helper validates version probes, launch argv/cwd/environment, source/preview/action callbacks, opaque IDs, cancellation, one and many selections, nested picker results, nonzero status propagation, and malformed output. The helper is excluded from normal release artifacts. Existing unit tests for parsing, parameter collection, channel installation, safe argv construction, cleanup, and queue policy remain the lowest-level coverage.

### AC3 — Real Television behavioral workflows

**Explicit.** With pinned Television 0.15.9 and `just` 1.53.0, automated PTY tests browse and filter recipes, observe the selected preview, execute single and multiple recipes, collect every supported parameter type, preserve adversarial arguments literally, resolve aliases/defaults/flags/variadics, cancel at each interactive layer, stop on first failure, and propagate the exact status. Assertions combine visible screen state, exit status, exact fixture output, and absence of forbidden side effects.

### AC4 — Stable visual screen evidence

**Explicit.** The suite parses Television's ANSI/alternate-screen output into normalized frames and checks a deliberately small set of reviewed golden states: initial browser, filtered recipe with preview, multi-selected queue, nested choice/path picker, and redacted confirmation. Snapshots use fixed dimensions and scrub only documented volatile values. Any snapshot update is explicit and reviewable; normal CI cannot auto-accept new output.

### AC5 — Lifecycle, security, and terminal recovery

**Inferred — Safe to execute.** Escape and Ctrl-C/SIGTERM execute nothing unintended, return the documented status, remove session/picker files, and leave the parent terminal usable for a subsequent command. Secret text never appears in screen frames, transcripts, failure artifacts, command previews, or snapshots. No legacy history file is written, and shell metacharacters, whitespace, Unicode, and embedded newlines are never reinterpreted.

### AC6 — Operable test commands, CI, and exploratory guidance

**Inferred — Safe to execute.** The repository exposes clear commands for fast tests, fake-tool contracts, real-TUI behavior, visual snapshots, soak testing, and the full gate. CI runs unit/contract/harness tests on Linux/macOS/Windows, required real-TV tests on Linux and macOS, MSRV/audit checks, snapshot drift detection, and failure-artifact upload. `docs/testing.md` explains prerequisites, debugging, snapshot review, platform scope, and a repeatable optional human exploratory session with setup, expected observations, evidence, and cleanup.

## Repo Evidence

### Baseline worktree

- `git status --short` is clean.
- `HEAD` is `447f086 finished 1st goal oneshot`; its parent is the archived Bash baseline `0b034e1 add jtv-v0.3.0`.
- No repository `AGENTS.md` is present.
- The likely edit areas are currently clean: `Cargo.toml`, `Cargo.lock`, `justfile`, `.github/workflows/ci.yml`, `tests/`, `README.md`, and `docs/`.
- Planning adds only this file: `docs/plans/jtv-tui-testing-suite.md`.

### Current automated coverage

- `cargo test --all-features` currently passes **38 tests** across:
  - `tests/just_model.rs` and `tests/just_integration.rs`: JSON model, aliases/modules, real `just`, and preview generation.
  - `tests/config.rs`, `tests/parameters.rs`, and `tests/command_plan.rs`: all parameter types, defaults, flags, variadics, cancellation, redaction, and adversarial argv.
  - `tests/runner.rs`: ordered queues, stop-on-failure, exact OS arguments.
  - `tests/channel_install.rs` and `tests/tv_protocol.rs`: channel ownership/install behavior, opaque IDs, session permissions/lifecycle, preview/source/action templates, launch command, and TV version floor.
  - `tests/cli.rs`: CLI/version/init/doctor and Unix fake executable checks.
- `tests/e2e/run.sh` is a 219-line real-TV suite driven by `/usr/bin/expect`. It covers simple execution, Escape, Ctrl-C recovery, literal metacharacters, choice, file, secret redaction, multi-select failure, terminal readiness, temporary-state cleanup, and history absence.
- `tests/fixtures/e2e/bin/jtv` wraps preview callbacks and copies preview output to `JTV_E2E_PREVIEW_LOG`; this proves callback delivery but not the final rendered preview panel.
- The E2E script relies on elapsed delays (`after 800`, `after 300`, and similar) and raw prompt matching. There is no reusable PTY API, virtual-screen parser, checked-in screen snapshot, resize test, timeout artifact bundle, or flake/soak command.
- Configured E2E fixtures do not yet exercise boolean, directory, aliases/modules, literal and expression defaults, flags, variadics, Unicode/newline arguments, malformed TV output, or cancellation from every nested layer through real TV.

### Build and CI contracts

- Root `justfile` currently exposes `fmt`, `fmt-check`, `clippy`, `test`, `e2e`, `build`, and `check`.
- `.github/workflows/ci.yml` runs format/lint/test/release build on Ubuntu, macOS, and Windows; the real-TV E2E job runs only on Ubuntu and installs Expect.
- CI pins `just 1.53.0`, Television 0.15.9, and Rust 1.85 for MSRV; it also runs `cargo audit`.
- Current dev dependencies are `assert_cmd`, `predicates`, and `pretty_assertions`; no PTY, VT parser, or snapshot dependency exists.

### Runtime contracts relevant to tests

- `JTV_JUST` and `JTV_TV` select child binaries; `JTV_TV_CABLE_DIR` and `TELEVISION_CONFIG` isolate the channel installation.
- `JTV_SESSION` and `JTV_PICKER_STATE` point at private temporary JSON state. `JTV_BIN` is used by nested picker source commands.
- `src/television.rs` launches the real channel; `src/picker.rs` launches nested Television; `src/session.rs` creates random mode-0600 session files; `src/cleanup.rs` handles termination cleanup.
- `assets/jtv-recipes.toml` is the shell-facing cable contract. It extracts opaque IDs for preview and multi-selection actions and must remain injection-safe.

### Discovery commands and sources

- Local evidence came from `git status --short`, `git log --oneline`, `rg --files`, targeted `rg` over test names/environment variables, `find tests`, `Cargo.toml`, the root `justfile`, CI, the cable asset, TUI/process modules, and current documentation.
- `cargo test --all-features` was rerun during planning and all 38 tests passed.
- Candidate stack research used primary project documentation:
  - `portable-pty` 0.9.0 provides a cross-platform native PTY API, child spawning, fixed `PtySize`, reader/writer handles, and resize support: <https://docs.rs/portable-pty/0.9.0/portable_pty/>.
  - `vt100` 0.16.2 parses terminal bytes into an in-memory screen and declares Rust 1.70: <https://docs.rs/vt100/0.16.2/vt100/>.
  - `insta` 1.48.0 provides reviewed snapshots, filters/redactions, and CI-safe update policies: <https://docs.rs/insta/1.48.0/insta/>.
  - `expectrl` 0.9.0 is the fallback PTY driver if `portable-pty` cannot meet the MSRV/platform contract: <https://docs.rs/expectrl/0.9.0/expectrl/>.
- The latest candidate versions are not pre-approved: Phase 0 must select and lock the newest versions that compile under Rust 1.85 and pass `cargo audit` on the supported CI matrix.

### Pattern classification

**Reuse**

- Existing fixture Justfiles, `.jtv.toml`, exact execution log assertions, isolated `HOME`/config/temp directories, environment overrides, opaque-ID protocol tests, secret sentinel checks, and status assertions.
- Existing unit/integration tests remain authoritative at the lowest reliable layer.
- The real Television 0.15.9 and `just` 1.53.0 compatibility floors.

**Modify**

- Convert `tests/e2e/run.sh` from the primary Expect implementation to a thin entry point for the Rust TUI suite, or remove it only after command and CI parity is proven.
- Replace preview-wrapper evidence and timing sleeps with screen conditions, structured process events, and stable deadlines.
- Split the large E2E fixture into named scenarios with reusable setup and assertions.
- Expand CI from Ubuntu-only real-TUI execution to Linux/macOS, while adding portable harness coverage on Windows.

**Avoid**

- Raw ANSI substring matching as the main visual oracle.
- Arbitrary fixed sleeps, automatic retries that hide flakes, and accepting snapshots in CI.
- Full-screen snapshots for every keystroke, platform-specific absolute paths, frecency/history-dependent ordering, real user config, or network access during tests.
- Logging secret send events or uploading unsanitized raw transcripts.
- Test-only public CLI flags or production binaries included in release artifacts.

## Implementation Plan

### Phase 0 — Prove the dependency and platform architecture

1. Create a small isolated spike on the real project MSRV before building the suite:
   - Open a native PTY at a fixed 120×40 viewport.
   - Spawn a deterministic local fixture program.
   - Send text, Enter, Tab, Escape, Ctrl-C, and a resize.
   - Feed the byte stream into `vt100` and assert reconstructed content and cursor state.
   - Reap the child after normal exit, timeout, and interruption.
2. Evaluate `portable-pty` first. Pin the newest release that passes Rust 1.85 on Linux/macOS/Windows. Use `expectrl` only if the direct PTY API cannot meet that contract without unsafe platform-specific code.
3. Evaluate `insta` with default features disabled and only the features actually needed. If no Rust-1.85-compatible release exists, use a small checked-in golden-text comparator instead of raising the project MSRV.
4. Record the selected versions and rationale in `docs/testing.md`; update `Cargo.toml`/`Cargo.lock` only after the spike passes.
5. Stop and re-plan if Windows ConPTY behavior, signal delivery, or the Rust 1.85 dependency graph requires a production support promise or an MSRV increase.

Likely files: `Cargo.toml`, `Cargo.lock`, `tests/support/mod.rs`, `tests/support/pty.rs`, `tests/pty_harness.rs`.

### Phase 1 — Build the reusable test harness

1. Add a test-only `TestProject`/`TestSandbox` that owns:
   - Temporary project, home, config, cable, runtime, and artifact directories.
   - Fixed locale, `TERM=xterm-256color`, known viewport, isolated Television config, and pinned binary paths.
   - Fixture file creation and exact execution-event reading.
   - Cleanup and a failure-preservation mode that never writes outside the sandbox.
2. Add `PtySession` with a narrow API:
   - Spawn with explicit argv/cwd/env and viewport.
   - `send_text`, `send_key`, `resize`, `wait_for_screen`, `wait_for_exit`, and `interrupt`.
   - One reader task/thread that stores bounded raw bytes and incrementally updates the VT parser.
   - Condition-variable/event notification so waits react to output instead of sleeping.
   - A single scenario deadline plus short, documented quiet-period detection for stable snapshots.
3. Add `ScreenFrame` normalization:
   - Extract character cells, cursor/visibility, dimensions, and optionally style classes important to selection/redaction.
   - Trim only trailing blank cells/rows.
   - Replace the sandbox root, path separator differences, random IDs, and explicitly listed volatile status text with named tokens.
   - Reject unknown control-sequence loss or over-broad redactions.
4. Add structured diagnostics:
   - On failure, print the normalized frame and recent redacted events.
   - Persist sanitized raw transcript, final frame, event trace, exit status, and metadata under `target/jtv-test-artifacts/<scenario>/`.
   - Never record the contents of secret input events; scan artifacts for scenario secret sentinels before allowing upload.
5. Write harness self-tests for normal exit, timeout, kill-on-drop, resize, alternate screen, Unicode/wide cells, fragmented escape sequences, and a subsequent shell/command round trip.

Likely files: `tests/support/{mod,sandbox,pty,screen,keys,artifacts}.rs`, `tests/fixtures/terminal_probe.rs`, `tests/pty_harness.rs`.

### Phase 2 — Complete fake-tool and protocol contracts

1. Add a cross-platform, test-only helper executable that can act as `tv` or `just` according to a scenario file/environment setting. Prefer a Cargo binary with `required-features = ["test-support"]`, `test = false`, and an explicit assertion that a default release build does not produce/package it.
2. Fake `just` modes must cover supported/unsupported versions, valid/additive/malformed JSON, `--show`, exact argv recording, and configured exit codes.
3. Fake TV modes must cover:
   - `--version` and top-level channel argv/cwd/environment.
   - Source and preview callback invocation using opaque rows.
   - No selection, one selection, unordered many selections, duplicate/unknown/malformed IDs, and action exit propagation.
   - Nested choice/boolean/path picker output, cancellation, and malformed output.
4. Assert the cable templates and fake-tool observations agree. Display text—including tabs, quotes, shell syntax, Unicode, and newlines after sanitization—must never become executable syntax or a lookup key.
5. Retain module-level protocol tests; fake-process tests prove the OS boundary while unit tests prove internal transformation rules.

Likely files: `Cargo.toml`, `tests/helpers/jtv_test_tool.rs`, `tests/support/fake_tools.rs`, `tests/tv_contract.rs`, `tests/cli.rs`, `tests/tv_protocol.rs`, `tests/fixtures/protocol/`.

### Phase 3 — Port and expand real-Television behavioral E2E tests

1. Keep the current Expect suite green while porting scenarios one by one. Maintain a parity table mapping every old assertion to a Rust PTY test; remove direct Expect dependence only after all rows pass in the same commit.
2. Add ignored/explicit real-tool tests so ordinary `cargo test` never silently skips a missing dependency, while `just test-tui` deliberately runs every real-TV scenario with `--test-threads=1` and fails with actionable version diagnostics.
3. Cover these scenario groups with real Television 0.15.9:
   - Browse/filter, visible documentation/body preview, single execution, and confirmation.
   - Ordinary string, literal default, expression default before a later positional, secret, choice, boolean, file, and directory inputs.
   - Long/short flags, value-taking flags, star/plus variadics, aliases, and module namepaths.
   - Spaces, quotes, semicolons, command substitutions, Unicode, and supported embedded newline arguments as exact values.
   - Escape from recipe browsing, nested TV choice/path pickers, and scalar/secret prompts; decline confirmation.
   - Ctrl-C/SIGTERM while the root TV and nested picker are active; documented status, no recipe execution, no temp residue, and a usable terminal afterward.
   - Multi-selection with deterministic recipe-name order, exact first failure status, and proof that later recipes did not run.
   - Missing/incompatible `just` or TV, bad JSON, and channel conflict diagnostics where a TTY is relevant.
4. Use an append-only structured fixture event log rather than parsing command display text. Every test asserts both UI state and durable behavior.
5. Eliminate the preview tee wrapper once the virtual screen proves the real preview panel. Retain a lower-level preview callback assertion in contract tests.
6. Convert `tests/e2e/run.sh` to `exec just test-tui` for backward command compatibility, or replace callers atomically and remove it if no consumer remains.

Likely files: `tests/tui_workflows.rs`, `tests/support/scenarios.rs`, `tests/fixtures/e2e/{justfile,.jtv.toml,...}`, `tests/e2e/run.sh`, `tests/fixtures/e2e/bin/jtv`.

### Phase 4 — Add reviewed virtual-screen snapshots

1. Capture snapshots only after the corresponding semantic assertions pass.
2. Add canonical 120×40 Linux snapshots for:
   - Initial recipe browser with input/results/preview regions.
   - Filtered parameterized recipe and accurate preview.
   - Three marked selections before execution.
   - Nested choice and path selection.
   - Confirmation with a secret redacted and non-secret argv quoted readably.
3. Assert key semantics separately—selected item, preview text, prompt, redaction, and cursor/focus—so a snapshot diff is diagnostic rather than the only correctness signal.
4. Configure snapshot updates as explicit local review (`INSTA_UPDATE=no` in CI). Document the command to create `.snap.new` files and require review before acceptance.
5. Run semantic screen assertions on all supported PTY platforms. Keep canonical snapshots Linux-only unless evidence shows a single normalization contract is honest across macOS/Windows; otherwise use narrowly scoped platform snapshots rather than hiding differences.
6. Add a determinism test that repeats the high-value browser scenario at least ten times and compares normalized frame hashes. No retry may turn a failure into a pass.

Likely files: `.insta.yaml` or a small golden comparator config, `tests/tui_snapshots.rs`, `tests/snapshots/`, `tests/support/screen.rs`.

### Phase 5 — CI, task commands, documentation, and manual exploration

1. Expand the root `justfile` with stable commands:
   - `test-fast`: unit/integration/harness tests without real TV.
   - `test-contract`: fake-tool OS-boundary tests.
   - `test-tui`: pinned real-TV behavioral suite.
   - `test-snapshots`: canonical screen assertions with updates forbidden.
   - `test-tui-soak runs=10`: repeated deterministic scenarios.
   - `test-all`: format, Clippy, fast, contract, snapshot, real-TUI, MSRV, and audit as appropriate for the local platform.
2. Restructure CI:
   - Linux/macOS/Windows: format/lint/build plus fast, contract, and PTY harness tests.
   - Linux: required full real-TV behavioral suite, canonical snapshots, and a short soak.
   - macOS: required real-TV smoke covering browse/preview/run, nested picker, cancellation, and terminal restoration.
   - Windows: required native PTY harness and fake-tool contracts; add an explicitly named nonblocking real-TV capability probe until Windows interactive support is accepted as product scope.
   - Preserve Rust 1.85 MSRV and RustSec audit jobs.
   - Upload sanitized `target/jtv-test-artifacts` only on failure.
3. Add `docs/testing.md` covering architecture, layer selection, prerequisites, pinned versions, artifact interpretation, secret-safety rules, snapshot review, CI/platform matrix, and flake policy.
4. Add an optional human exploratory checklist:
   - Build and create an isolated temporary project/config.
   - Run init/doctor, browse/filter, inspect previews, nested pickers, multi-select, cancellation, resize, colors, and long content.
   - Run a normal command afterward and inspect cleanup.
   - Record observations or an optional asciinema/terminal screenshot without secrets.
   - Remove the isolated directories.
   This checklist assesses usability only; all correctness observations also have automated equivalents.
5. Update README development/testing links and remove obsolete manual-only language.

Likely files: `justfile`, `.github/workflows/ci.yml`, `docs/testing.md`, `README.md`.

### Phase 6 — Reliability, security, and final audit

1. Run every test layer from a clean checkout with only documented prerequisites.
2. Run the real-TV suite repeatedly and investigate every flake; do not add retries or wider sleeps as the fix.
3. Scan all snapshots/transcripts/artifacts for secret sentinels, random session paths, home paths, and legacy history files.
4. Verify default release artifacts contain only `jtv`, not fake tools or test support.
5. Have an independent reviewer audit PTY teardown, timeout paths, process trees, Windows/macOS conditionals, normalization scope, secret handling, and correspondence between BDD scenarios and tests.
6. Remove superseded Expect/wrapper code only after the parity matrix, CI callers, and documentation agree.

## BDD Scenarios

```gherkin
Feature: Trustworthy interactive Justfile execution

  Scenario: Browse, preview, and run a recipe with a literal argument
    Given an isolated project with a documented parameterized recipe
    When the user filters to that recipe in Television
    Then the selected recipe's documentation and body are visible in the preview
    When the user selects it, enters an argument containing spaces and shell metacharacters, and confirms
    Then the recipe receives that exact argument once
    And no shell fragment from the argument is executed

  Scenario: Cancel or interrupt without leaving state behind
    Given an interactive jtv session with no recipe executed yet
    When the user presses Escape or interrupts the active root or nested picker
    Then no recipe is executed
    And the documented status is returned
    And no session, picker, or legacy history file remains
    And the terminal accepts and displays the next command normally

  Scenario: Collect typed parameters without disclosing a secret
    Given a recipe configured with string, secret, choice, boolean, file, and directory parameters
    When the user supplies each value through its interactive control
    Then Television is used for enumerable and path values
    And the confirmation shows the non-secret values
    And the secret is redacted from every visible or retained test artifact
    And the recipe receives every exact value

  Scenario: Run a multi-selection deterministically and stop on failure
    Given three recipes whose name order is known and the second recipe fails
    When the user marks all three recipes and runs the selection
    Then the confirmation shows the deterministic recipe-name order
    And only the first and second recipes execute in that order
    And jtv returns the second recipe's exact failure status
```

### Scenario mapping

| Scenario | Acceptance criteria | Planned verification |
|---|---|---|
| Browse, preview, and literal run | AC1, AC3, AC4, AC5 | Real-TV PTY workflow, exact fixture event, filtered-preview snapshot, adversarial side-effect sentinel. |
| Cancel or interrupt | AC1, AC3, AC5 | Root/nested Escape and signal tests, exit status, temp scan, parent-terminal round trip. |
| Typed parameters and secret | AC2, AC3, AC4, AC5 | Fake nested-picker contracts, real-TV scenario, confirmation snapshot, secret scan, exact fixture event. |
| Deterministic multi-selection | AC2, AC3, AC4 | Fake unordered selection, real-TV Tab selection, queue snapshot, event order, exact failure status. |

## Subagent Plan

Subagents are used because PTY portability, virtual-terminal correctness, black-box contracts, real workflow coverage, and CI/security review are independently risky areas. **Every subagent must use exactly `gpt-5.6-terra`; no model-family substitution is allowed.** Implementation subagents use medium or high thinking only and never xhigh.

### Wave 1 — Evidence and architecture

1. **PTY/dependency investigator — high thinking**
   - Boundary: read-only spike and recommendation for `portable-pty`/fallback, `vt100`, snapshot mechanism, Rust 1.85, Linux/macOS/Windows behavior, and test-helper packaging.
   - Owns: AC1 architecture evidence and Phase 0 stop/re-plan decision.
   - Verification: minimal cross-platform commands or compile probes, dependency/MSRV/audit report, concise API risks. No production edits.

### Wave 2 — Core test infrastructure

2. **PTY harness builder — high thinking**
   - Boundary: `tests/support/pty.rs`, screen/event plumbing needed for harness self-tests, and `tests/pty_harness.rs`; no application behavior changes.
   - Owns: AC1 and teardown/timeout portions of AC5.
   - Verification: self-tests, Clippy, MSRV check, timeout/kill diagnostics, platform conditionals documented.
3. **Sandbox/artifact builder — high thinking**
   - Boundary: isolated project/config/runtime setup, sanitized failure artifacts, fixtures, and secret scans; coordinate interfaces with the harness builder before editing shared `mod.rs`.
   - Owns: isolation and artifact portions of AC1/AC5.
   - Verification: cleanup, path normalization, redaction, and failure-artifact tests.

### Wave 3 — Independent behavior layers

4. **Fake-tool contract builder — medium thinking**
   - Boundary: test-only helper executable and `tests/tv_contract.rs`; must prove it is excluded from normal release builds.
   - Owns: AC2.
   - Verification: targeted cross-platform contract tests and malformed/cancellation/status cases.
5. **Real-TV workflow builder — high thinking**
   - Boundary: `tests/tui_workflows.rs`, E2E fixtures, and old-Expect parity table; consume, do not redesign, the harness API.
   - Owns: AC3 and behavioral portions of AC5.
   - Verification: pinned real-TV suite, exact fixture events/statuses, root/nested cancellation, terminal recovery.
6. **Screen snapshot builder — high thinking**
   - Boundary: normalization rules, semantic screen assertions, selected snapshots, and determinism test; no workflow expansion beyond agreed states.
   - Owns: AC4.
   - Verification: snapshot check mode, ten-run frame stability, review of every redaction rule.

### Wave 4 — Integration and review

7. **CI/documentation integrator — medium thinking**
   - Boundary: `justfile`, CI matrix/artifact upload, `docs/testing.md`, and README links after test commands stabilize.
   - Owns: AC6.
   - Verification: workflow syntax, each documented command, platform/dependency matrix, manual checklist cleanup.
8. **Final reliability/security reviewer — high thinking, read-only**
   - Boundary: complete diff and verification evidence; focus on orphaned processes, timing flakes, secret/artifact leakage, over-normalized snapshots, release-helper leakage, and BDD coverage.
   - Owns: independent Definition-of-Done audit across AC1–AC6.
   - Verification: report findings with severity and file/line evidence; orchestrator resolves all blockers and reruns affected gates.

The orchestrator sequences waves to prevent shared-file conflicts, integrates all changes, runs the canonical suite itself, and remains accountable for the global definitions of done.

## Definitions Of Done

- AC1–AC6 are implemented without an unapproved public behavior or MSRV change.
- All 38 existing tests remain passing or are replaced by demonstrably stronger tests with a recorded one-to-one coverage mapping.
- The Rust PTY harness is exercised on Linux, macOS, and Windows and proves normal exit, timeout, resize, alternate screen, interruption, kill-on-drop, and terminal reuse.
- Fake-tool tests cover every `just`/TV boundary, selection cardinality, nested picker result, cancellation, malformed output, and status propagation.
- The complete BDD behavior is automated against real Television 0.15.9; no correctness assertion remains manual-only.
- Real-TV tests verify every supported parameter shape, aliases/modules, defaults, flags, variadics, adversarial values, one/many selections, cancellation, failure, cleanup, and terminal restoration.
- At least the five planned normalized visual states are checked in, semantically asserted, stable across ten consecutive canonical runs, and update-disabled in CI.
- No secret sentinel occurs in stdout/stderr capture, virtual screens, snapshots, event traces, or uploaded artifacts; the recipe-only temporary secret sink is deleted.
- No session/picker residue, child/grandchild process, `.just_history`, or `.just-tv-last-command` remains after success, cancellation, timeout, or signal.
- No arbitrary sleep is used as the success condition. Every wait has a named condition, deadline, and useful timeout artifact.
- Default `cargo build --release` produces no fake/test helper executable.
- Required CI gates pass on their stated platform matrix; Windows real-TV capability status is reported honestly and does not imply certification without approval.
- `docs/testing.md`, README, task commands, CI, and actual test behavior agree.
- The final implementation report lists files changed, dependency choices, commands and platforms run, exact results, snapshot/artifact locations, Windows probe result, residual risks, and cleanup performed.

## Verification Plan

| Check | Expected observable result | Coverage |
|---|---|---|
| `cargo fmt --all --check` | No formatting differences. | AC1–AC6 |
| `cargo clippy --all-targets --all-features -- -D warnings` | No warnings, including test helpers and platform conditionals. | AC1–AC6 |
| `just test-fast` | Unit, real-`just`, harness self-tests, screen parser, sandbox, cleanup, and artifact tests pass without Television. | AC1, AC5 |
| `just test-contract` | Fake `just`/TV sees exact argv/cwd/env and every callback/selection/cancellation/error contract passes. | AC2, BDD 1–4 lower-level contracts |
| `just test-tui` | Pinned real TV opens; every behavioral scenario reaches semantic screen conditions, emits exact fixture events/statuses, and cleans up. | AC3, AC5, all BDD scenarios |
| `just test-snapshots` | No `.snap.new` files or diffs; five high-value screens match reviewed normalized frames and secret scan passes. | AC4, BDD 1, 3, 4 |
| `just test-tui-soak 10` | Ten runs produce the same normalized frame hashes and behavior; zero retries/timeouts/orphans. | AC1, AC3, AC4, AC5 |
| `cargo +1.85.0 check --locked --all-targets --all-features` | Entire production and test-support graph compiles at declared MSRV. | AC1, AC2, AC6 |
| `cargo build --release` plus release-artifact inspection | `target/release/jtv` exists; no `jtv-test-*` helper is emitted. | AC2, AC6 |
| `cargo audit` | No known vulnerable locked dependency. | AC1, AC6 |
| `git diff --check` | No whitespace errors. | AC6 |
| Linux CI real-TV job | Full behavioral suite, canonical snapshots, and short soak pass with TV 0.15.9/just 1.53.0. | AC3–AC6 |
| macOS CI real-TV job | Browse/preview/run, nested picker, cancellation, and terminal reuse pass through native PTY. | AC1, AC3, AC5, AC6 |
| Windows CI portable job | Native PTY self-tests and fake-tool contracts pass; capability probe result is preserved separately. | AC1, AC2, AC6 |

### Failure evidence requirements

Every PTY or TUI failure must expose, without secrets:

- Scenario and platform.
- Exact executable/version, cwd, safe argv, viewport, and deadline.
- Exit status or timeout state.
- Final normalized screen and recent semantic events.
- Sanitized raw terminal capture when needed for parser diagnosis.
- Remaining process/temp-file scan.
- CI artifact location.

### Optional manual exploratory check

1. Start from a clean build and an isolated temporary `HOME`, Television config, runtime directory, and fixture project.
2. Run `jtv init` and `jtv doctor`; confirm pinned versions and the isolated cable path.
3. Launch `jtv` in a normal terminal; inspect fuzzy-search responsiveness, selected-row clarity, preview readability, resizing, long docs, colors, nested picker transitions, and confirmation readability.
4. Exercise Escape, Ctrl-C, secret entry, and multi-selection; verify a normal shell command works afterward.
5. Capture written observations and, optionally, a secret-free terminal recording or screenshot.
6. Remove all isolated directories and confirm no process or project history remains.

This manual check evaluates subjective interaction quality only and cannot substitute for a failing automated gate.

## Risks And Questions

### Safe-to-execute assumptions

- A character-cell VT reconstruction is the appropriate stable representation of “what the user sees”; raster pixels are not a portable product contract.
- Fixed 120×40 canonical snapshots plus semantic assertions at other sizes provide useful visual coverage without overspecifying Television.
- `portable-pty` plus `vt100` is the preferred design; compatible versions are selected by the MSRV spike rather than assumed.
- Real-TV scenarios may be serialized for determinism; isolated config/home/runtime state prevents cross-test frecency and file collisions.
- Linux is the canonical snapshot platform; Linux and macOS are required real-TV platforms under the existing product scope.
- Current public interfaces need no change. Internal refactoring is allowed only when test evidence demonstrates a necessary seam.

### Needs confirmation before expanding scope

- Promoting the Windows real-Television capability probe to a required release gate would certify interactive Windows behavior that the existing application plan explicitly deferred. If requested later, treat that as product compatibility work, not merely test plumbing.
- Adding raster terminal screenshots or recordings as committed/CI artifacts would require choosing a terminal emulator, font, renderer, image-diff policy, and storage budget. The planned virtual-screen snapshots already satisfy deterministic visual verification.

### Blocking questions

- None for the planned scope.

### Principal risks and mitigations

- **PTY/MSRV compatibility:** current cross-platform PTY releases may move faster than Rust 1.85. Mitigation: Phase 0 compile matrix, version pin, `expectrl`/older compatible release fallback, no MSRV bump without approval.
- **ANSI fidelity:** a VT parser may ignore a Television sequence such as synchronized output. Mitigation: parser fixture tests, raw transcript retention, semantic screen checks, and stop/re-plan on meaningful state loss.
- **Timing flakes:** asynchronous redraws and preview subprocesses can race. Mitigation: condition/event waits, bounded quiet period for snapshots, named deadlines, soak test, no retries or widening sleeps.
- **Snapshot brittleness:** TV spacing or status text may differ by platform. Mitigation: few snapshots, fixed dimensions/version, documented minimal normalization, semantic assertions, canonical Linux frames.
- **Secret leakage:** sent input or failure artifacts can expose secrets. Mitigation: typed redacted events, never log secret sends, sentinel scan before persistence/upload, temp cleanup.
- **Orphaned process trees:** timeout or Ctrl-C may leave TV/preview children. Mitigation: RAII process ownership, platform-aware termination/reaping, post-test process/temp scan.
- **Test helper leakage:** a fake binary could ship accidentally. Mitigation: required test-only feature/target and release artifact assertion.
- **Testing Television instead of `jtv`:** broad snapshots could fail on harmless upstream changes. Mitigation: pin TV 0.15.9 and assert only `jtv`'s integration contract and high-value user-visible states.
- **Dirty-file overlap:** baseline is clean now, but likely edits span most test/CI/docs files. Re-check before implementation and sequence agents around shared modules.
- **Performance:** real TUI tests and cargo-installed tools can slow CI. Mitigation: fast/contract/TUI layers, cached pinned tools, small required macOS subset, canonical snapshots only once, explicit soak job.
- **Accessibility:** keyboard-only operation is covered; screen-reader compatibility is not currently a declared Television/`jtv` contract and remains unchecked.
- **Data/migrations/rollback:** no persisted production data or schema migration is involved. Reverting the test harness/dependencies is sufficient rollback; user config must never be mutated during tests.

### Stop-and-re-plan triggers

- Required dependency cannot compile on Rust 1.85 or introduces an unresolved advisory.
- PTY abstraction cannot reliably terminate/reap process trees on a required platform.
- VT reconstruction loses meaningful Television content or focus/selection state.
- Stable testing requires a public CLI/config/session change or capturing secrets.
- A normalization rule would erase a user-visible regression rather than remove true volatility.
- Real-TV Windows probing exposes product changes needed for support; do not implement those under a testing-only goal.
- Pinned Television/`just` versions change, or CI cannot reproduce local frames after isolation and viewport controls.
- Existing test coverage would be deleted before stronger parity is demonstrated.

## /goal Execution Contract

The implementing orchestrator must:

1. Treat this plan as advisory but binding on scope until contradicted by fresh evidence.
2. Before editing, re-check the worktree baseline, dirty-path overlaps, key files, installed tool versions, dependency/MSRV assumptions, and current test/CI commands.
3. Stop and re-plan if those checks contradict scope, ownership, touched files, public or cable/session contracts, consumers, dependencies, data behavior, verification, risk, platform claims, user-visible behavior, or any Needs-confirmation/Blocking item.
4. Preserve unrelated user changes and keep the archived Bash implementation intact.
5. Use subagents exactly within the task boundaries above. Every subagent must use **`gpt-5.6-terra` exactly**. Use high thinking for architecture, PTY, visual, workflow, and review tasks; medium for bounded fake-tool and docs/CI work. No implementation or review subagent may use xhigh.
6. Continue implementation until every Definition of Done is met, including real-Television E2E, virtual-screen evidence, platform CI, soak testing, secret/artifact audit, and independent review—not merely compilation or unit tests.
7. Keep the existing Expect suite until an explicit parity map proves replacement coverage; remove or reduce it only after all consumers and CI commands are updated atomically.
8. Finish with evidence: files changed, selected dependency versions and MSRV result, tests/checks and platforms run, exact outcomes, snapshot/artifact evidence, Windows capability result, residual risks, and cleanup performed.
