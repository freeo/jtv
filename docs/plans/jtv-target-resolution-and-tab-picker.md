# jtv target resolution and TAB file picker plan

## Proposed outcome

`jtv docker` deterministically opens a root module or one of the four approved
standalone Justfile layouts, reports every overlap after Television exits, and
ordinary string prompts regain the archived TAB-triggered recursive Television
path picker without weakening selection safety.

- **Execution readiness:** Ready
- **Discovery confidence:** High
- **BDD:** Used — both features are user-visible workflows with precedence,
  cancellation, and state-preservation rules that are clearest as examples.

## User Problem Trace

### Stated goal

- Make selecting a named Justfile collection the default shorthand:
  `jtv docker`.
- Retain the accepted precedence rule: a matching module in the normally
  discovered root Justfile wins; otherwise resolve a standalone file.
- Support exactly these standalone layouts, in this order:
  1. `docker.just`
  2. `docker/justfile`
  3. `justfiles/docker.just`
  4. `justfiles/docker/justfile`
- Do not add `docker/mod.just`, `docker/.justfile`,
  `justfiles/docker/mod.just`, `justfiles/docker/.justfile`, `just.docker`, or
  other inferred aliases.
- When more than one target exists, continue with deterministic precedence and
  print this warning to stdout after leaving the root Television UI:

  ```text
  WARNING: 'docker' resolves to multiple targets:
    <target one>
    <target two>
  ```

- Restore the archived prompt behavior: while entering an ordinary string,
  TAB opens nested Television over recursively discovered paths rooted at the
  current working directory.

### Reported gaps and mapping

| Gap or constraint | Plan coverage |
|---|---|
| TAB currently does nothing in string input | AC4; phases 3–5; BDD scenarios 3–4 |
| Existing typed text should help find a path | AC4; pass the buffer as Television's initial input |
| `jtv docker` is currently rejected by Clap | AC1; phases 1–2 |
| Only four standalone layouts are wanted | AC1–AC2; candidate table and negative tests |
| Root modules retain priority | AC1; scenario 1 |
| Overlap must be visible only once outside jtv | AC3; deferred stdout warning after root TV exits |
| Selection must remain safe | AC5; reuse opaque picker IDs and argv-based execution |

### Non-goals

- No Television source patch and no recipe-row color work in this change.
- No additional filename conventions, directory heuristics, globbing, or
  fuzzy target-name resolution.
- No automatic recipe execution from `jtv docker`; it opens the selected
  recipe browser.
- No filesystem completion for secret, choice, boolean, explicitly typed
  `file`, or explicitly typed `directory` prompts; their existing behavior
  remains intact. TAB completion applies to ordinary string entry, including
  each value of a variadic string parameter.
- No symlink traversal during recursive path enumeration.

## Acceptance Criteria

### AC1 — Deterministic named target resolution

**Explicit.** `jtv NAME` resolves candidates in this exact order:

1. A public-recipe-bearing `NAME` module in the root Justfile found by normal
   `just` discovery.
2. `NAME.just`
3. `NAME/justfile`
4. `justfiles/NAME.just`
5. `justfiles/NAME/justfile`

The first existing candidate is opened. A module candidate reuses the root
invocation with `module_filter = NAME`; a file candidate becomes the explicit
Justfile and has no implicit module filter.

### AC2 — Exact convention and explicit CLI behavior

**Explicit.** Only the four approved standalone paths are considered. A name
with no module or file candidate exits nonzero and reports the four searched
paths. Existing `jtv`, `jtv init`, `jtv doctor`, hidden callback commands,
`jtv --justfile PATH`, and `jtv --module NAMEPATH` keep their behavior.

**Inferred — Safe to execute.** Target names are a single non-empty path
component: no `/`, `\\`, `.` or `..`. This prevents a positional convenience
from becoming a second arbitrary-path syntax; arbitrary paths remain the job
of `--justfile`. Combining positional `NAME` with `--justfile` or `--module`
is rejected as ambiguous rather than silently ignoring an argument.

### AC3 — Overlap warning after the TUI

**Explicit.** If at least two candidates exist, jtv selects the first by AC1,
retains every existing candidate label in precedence order, and prints exactly
one warning block to stdout after the root Television process returns,
including on normal cancellation:

```text
WARNING: 'docker' resolves to multiple targets:
  module docker
  docker.just
  justfiles/docker/justfile
```

No warning is printed for zero or one candidate or when the user supplied
`--justfile`/`--module` directly. The warning does not enter Television's
alternate screen, source protocol, callback stdout, or recipe output.

### AC4 — TAB-triggered nested path selection

**Explicit.** At every ordinary, non-secret string prompt, pressing TAB opens
a nested Television picker containing files and directories recursively below
the shell's current working directory, matching the archived implementation.
The current editable buffer is supplied as Television's initial query. A
selection replaces the buffer with its path relative to that working
directory and returns to the prompt; Enter submits it normally. Cancelling the
nested picker returns to the same prompt with the prior buffer and cursor
state preserved. Manual input without TAB behaves as before.

**Inferred — Safe to execute.** Hidden paths are included, symlinks are listed
but directory symlinks are not followed, entries are deterministically sorted,
and an empty directory produces an empty picker that can be cancelled without
losing input. This follows the existing `WalkDir::follow_links(false)` safety
boundary and does not silently omit user paths.

### AC5 — Terminal, identity, and execution safety

**Inferred — Safe to execute.** Nested selection continues to use generated
opaque IDs; displayed paths never become callback identities. Paths containing
spaces, quotes, shell metacharacters, and valid Unicode remain one OS argument.
Malformed/unknown picker output cannot execute a recipe. Ctrl-C/Escape returns
130 through the existing cancellation contract, temporary picker state is
removed, terminal input/echo is restored, and no shell interpolation is added.

## Repo Evidence

### Baseline worktree

Planning was performed on a dirty worktree containing the in-progress visual
UX restoration:

- Modified: `src/television.rs`, `docs/visual-design.md`,
  `tests/tv_protocol.rs`, `tests/visual_presentation.rs`, and six TUI snapshots.
- These changes are user-owned and must be preserved. This feature should not
  require rewriting the modified presentation logic; snapshot updates must be
  limited to genuinely changed prompt/workflow evidence.

### Files inspected

- `src/cli.rs`: Clap currently has flags plus a closed `CliCommand` enum and no
  positional target. `launch` constructs and canonicalizes `Invocation` before
  project loading and blocks until the root Television process exits.
- `src/invocation.rs`: owns cwd, explicit Justfile, module filter, dry-run, and
  binary paths; explicit paths are canonicalized without shell text.
- `src/just.rs`, `src/model.rs`: `just --dump --dump-format json` is recursively
  flattened; recipes retain `module`/`namepath`; exact module filtering already
  exists and should be reused.
- `src/parameters.rs`: `DialoguerPrompter` uses `dialoguer::Input`, whose current
  API does not expose TAB as a product event. The `Prompter` trait is already a
  deterministic unit-test seam.
- `src/picker.rs`: `TvPicker` already materializes recursive paths with
  `WalkDir`, launches a nested TV process without a shell, maps displayed text
  to opaque IDs, validates returned IDs, and cleans temporary JSON state. It
  lacks an initial-query argument and a combined file/directory mode.
- `jtv-0.3.0/just-tv-0.3.0.sh:676-790`: archived Bash intercepts TAB, passes
  the current Readline buffer to nested TV, recursively lists both files and
  directories, and replaces the line on selection.
- `jtv-0.3.0/test-just-tv-0.3.0.sh:122-174`: documents TAB with empty and
  partial input plus unaffected manual entry; these were manual-only tests.
- `tests/parameters.rs`: fake prompt/picker unit seams cover types, defaults,
  cancellation, and argv-safe values.
- `tests/tv_action_pty.rs`, `tests/tui_workflows.rs`: fake-process and real-TV
  PTY coverage already verifies nested pickers, cancellation, execution, and
  terminal behavior.
- `tests/support/keys.rs`: already models `Key::Tab`.
- `justfile`, `.github/workflows/ci.yml`: define fast, contract, real-TV,
  snapshot, soak, MSRV, release-artifact, and audit gates.

### Official just behavior considered

- Root discovery recognizes `justfile` and `.justfile`; module conventions are
  broader than the four standalone layouts selected for jtv.
- This feature intentionally implements a jtv convenience resolver, not a
  claim that all four paths are automatically discovered by `just`.
- Existing explicit `--justfile PATH` remains the authoritative arbitrary-path
  interface.

### Pattern classification

- **Reuse:** `Invocation`, `Project::filtered_by_module`, normal `just` JSON
  loading, `TvPicker` opaque IDs, `WalkDir`, temporary-state cleanup, PTY
  harness, `Key::Tab`, fake tools, and argv-based runner.
- **Modify:** Clap parsing, launch preparation, prompt result contract,
  `TvPicker::select`, and recursive picker modes.
- **Avoid:** Bash `bind -x`, shell-built `find | grep` commands, label-to-action
  lookup, hidden ANSI identifiers, broad module filename emulation, and
  manual-only acceptance.

### Consumers and dependencies

- CLI consumers: shell aliases/scripts using current flags and the reserved
  `init`/`doctor` command names.
- Internal callback consumers: the installed Television channel invokes hidden
  subcommands; parsing changes must leave these byte-for-byte compatible.
- Session consumers: callback processes deserialize `Invocation`; the new
  resolver should finish before session creation so persisted session schema
  need not change.
- Dependencies: `just >= 1.53`, Television `>= 0.15.9`, existing `console`,
  `dialoguer`, `walkdir`, `tempfile`, and PTY test stack. Prefer no new runtime
  dependency; add one only if the existing terminal API cannot deliver correct
  Unicode editing and terminal restoration under tests.
- No database, network API, permission, migration, or persistent data changes.

## Implementation Plan

### Phase 1 — Specify and isolate named-target resolution

1. Add a small resolver module, likely `src/target.rs`, with pure candidate
   construction and a filesystem-backed resolution boundary. Keep target
   resolution out of `Invocation::canonicalized` so explicit paths and
   convenience names remain distinct concepts.
2. Define `ResolvedTarget` containing the chosen `justfile`/`module_filter`,
   the user-facing candidate labels in precedence order, and an optional
   overlap warning payload.
3. Validate `NAME` as one path component. Construct only the four approved
   relative paths. Check regular-file candidates relative to the original cwd;
   do not follow directory symlinks while searching.
4. Use normal root Justfile discovery/project loading to determine whether
   `NAME` is a usable module. Do not reimplement `just` parsing. A module is a
   candidate when the loaded root project contains public recipes whose exact
   module namepath is `NAME` or begins with `NAME::`.
5. Preserve meaningful root parse/load errors. If implementation cannot
   reliably distinguish “no discoverable root Justfile” from a broken root
   using existing APIs, stop and add a typed discovery result rather than
   matching localized stderr text.
6. Unit-test candidate order, each individual layout, module precedence,
   nested module namepaths, all overlap combinations, unsupported aliases,
   invalid names, and not-found diagnostics.

Likely files: `src/target.rs`, `src/lib.rs`, `src/just.rs`, `src/model.rs`, and
new `tests/target_resolution.rs`.

### Phase 2 — Add the positional CLI without disturbing commands

1. Extend Clap so a non-reserved positional token is `target: Option<String>`
   while `init`, `doctor`, and all hidden callback commands remain subcommands.
   Verify actual Clap precedence with parser tests before altering launch code.
2. Express conflicts between positional target, `--justfile`, and `--module`
   in Clap where possible; otherwise return one stable user-facing error.
3. Resolve the target before final `Invocation` canonicalization and session
   creation. Feed a module choice into the existing module filter; feed a file
   choice into the existing explicit Justfile path.
4. Retain overlap data in the top-level launch frame only. After
   `television::launch` returns, print one warning block to stdout. Never put
   it into `SessionState`, hidden callback output, picker state, or stderr.
5. Document precedence, exact paths, conflicts, overlap behavior, and examples
   in `README.md` and the CLI help text.

Likely files: `src/cli.rs`, `src/target.rs`, `tests/cli.rs`,
`tests/tv_contract.rs`, and `README.md`.

### Phase 3 — Turn TAB into a first-class prompt event

1. Replace the string-only `Prompter::input` result with an explicit event,
   for example `Submitted(String)`, `BrowsePaths { buffer, cursor }`, and
   `Cancelled`. Keep secrets on the existing password path, which never emits
   `BrowsePaths`.
2. Implement a focused line editor for ordinary strings using the existing
   terminal abstraction if it supports: printable Unicode insertion,
   left/right/home/end, backspace/delete, Enter, TAB, Escape, and Ctrl-C.
   Defaults retain current semantics. Redraw using display-cell widths so wide
   Unicode and cursor placement remain correct.
3. Do not hold a raw-mode guard, borrowed terminal lock, or hidden cursor state
   while launching nested Television. Restore the prompt terminal before the
   child starts, then redraw the prompt and buffer when it returns.
4. Keep the editor behind `Prompter` so state transitions are exhaustively unit
   tested without a terminal. If `console` cannot meet the behavior safely,
   stop and compare a minimal Rust-1.85-compatible editor dependency rather
   than growing an ad hoc terminal parser.

Likely files: `src/parameters.rs`, possibly a new `src/input.rs`,
`tests/parameters.rs`, and `tests/pty_harness.rs`.

### Phase 4 — Extend the nested Television picker for completion

1. Add a completion-specific picker operation accepting `root` and
   `initial_query`, distinct from configured file-only/directory-only pickers.
2. Reuse `TvPicker::select`, adding optional initial input and passing it to
   Television as separate argv: `--input <buffer>`. Never construct shell
   source expressions from the query.
3. Enumerate both files and directories recursively below `Invocation.cwd`,
   exclude the root itself, do not follow symlinked directories, retain paths
   relative to cwd, sanitize display only, sort deterministically, and preserve
   opaque IDs as output.
4. In parameter collection, loop on `BrowsePaths`: invoke the completion
   picker, replace the edit buffer only after a valid selection, or resume with
   the unchanged buffer/cursor on cancellation. Apply this loop to singular
   and variadic ordinary strings, including the later-positional materializing
   prompt; do not alter typed file/directory/choice/boolean/secret handling.
5. Define nested TV failure separately from cancellation: malformed output or
   non-cancellation process failure aborts safely and executes nothing.

Likely files: `src/picker.rs`, `src/parameters.rs`, `tests/parameters.rs`,
`tests/picker.rs` or `tests/tv_contract.rs`, and fake-tool fixtures.

### Phase 5 — Automate the formerly manual workflows

1. Unit-test prompt state transitions: empty TAB, partial-query TAB, selection,
   cancellation, selection replacement, manual entry, defaults, variadic
   values, Unicode editing, and secrets ignoring TAB completion.
2. Extend fake-TV contracts to assert exact `--input` argv, recursive source
   display, opaque returned ID, relative selected value, unknown/malformed
   output rejection, and temporary-state cleanup.
3. Add real PTY workflows:
   - type `docs`, press TAB, observe nested TV filtered by `docs`, select a
     recursive path, return to the prompt, submit, confirm, and assert exact
     recorded argv;
   - type a partial value, TAB, cancel nested TV, verify the same text remains,
     then finish manually;
   - Ctrl-C at the prompt and Escape in nested TV both restore the terminal and
     execute nothing.
4. Add CLI integration scenarios for every filename layout, module priority,
   overlap warning timing/content, zero matches, conflicts, and unsupported
   paths. Use fake Television to prove the warning appears after its recorded
   exit marker rather than in callback/source output.
5. Update reviewed snapshots only if the normal prompt or nested picker header
   visibly changes. Add a concise prompt hint such as `TAB files` if it fits the
   established visual grammar; do not add explanatory chatter.

Likely files: `tests/target_resolution.rs`, `tests/cli.rs`,
`tests/tv_contract.rs`, `tests/tv_action_pty.rs`, `tests/tui_workflows.rs`,
`tests/support/fake_tools.rs`, `tests/support/scenarios.rs`, fixtures, and
possibly targeted snapshots.

### Phase 6 — Documentation and final integration

1. Document the four-path convention, module-first rule, exact precedence,
   overlap warning, explicit-flag conflicts, and unsupported conventions.
2. Document ordinary string prompt controls, TAB query behavior, recursive
   scope, cancellation, relative-path result, and why secret/configured picker
   types behave differently.
3. Re-run the full cross-platform and real-Television matrix. Confirm no visual
   UX work already present in the dirty tree was overwritten.
4. Review help output and diagnostics for concision and stable stdout/stderr
   ownership.

Likely files: `README.md`, `docs/configuration.md`, `docs/testing.md`, and CLI
help text.

### Stop-and-replan triggers

- Clap cannot support optional positional targets alongside reserved and hidden
  subcommands without changing an existing invocation.
- Normal root discovery cannot be distinguished from root parse failure without
  scraping unstable/localized stderr.
- The terminal abstraction cannot release and restore input state safely across
  nested TV or cannot edit Unicode correctly at the supported MSRV.
- Television 0.15.9 does not honor `--input` for this nested source mode.
- A selected path cannot remain a relative, single OS argument through the
  current command plan.
- New work overlaps unresolved user changes in the presentation files or would
  require accepting unrelated snapshots.

## BDD Scenarios

```gherkin
Feature: Select a named jtv target and complete string parameters with paths

  Scenario: A root module wins over standalone files
    Given the discovered root Justfile has a docker module
    And docker.just and justfiles/docker/justfile also exist
    When I run jtv docker and leave the recipe browser
    Then jtv browses recipes from the root docker module
    And stdout warns that docker resolved to all three targets in precedence order

  Scenario Outline: A supported standalone layout opens as a Justfile
    Given there is no docker module in the discovered root Justfile
    And only <path> exists
    When I run jtv docker
    Then jtv opens <path> as the selected Justfile
    And no overlap warning is printed

    Examples:
      | path                        |
      | docker.just                 |
      | docker/justfile             |
      | justfiles/docker.just       |
      | justfiles/docker/justfile   |

  Scenario: TAB completes an ordinary string from the current directory
    Given a recipe asks for an ordinary string
    And the current directory contains docs/guide.md
    When I type docs and press TAB
    Then nested Television opens with docs as its query
    When I select docs/guide.md and submit the prompt
    Then the recipe receives docs/guide.md as one argument

  Scenario: Cancelling nested Television preserves the unfinished input
    Given a recipe asks for an ordinary string
    And I have typed docs into the prompt
    When I press TAB and cancel nested Television
    Then I return to the same prompt with docs unchanged
    And no recipe has executed
```

Mapping:

- Scenario 1 → AC1, AC3; CLI resolver integration and warning-order PTY test.
- Scenario outline → AC1, AC2; table-driven filesystem/CLI integration test.
- Scenario 3 → AC4, AC5; prompt unit test, fake-TV contract, real-TV PTY E2E.
- Scenario 4 → AC4, AC5; prompt state unit test and cancellation PTY E2E.

## Subagent Plan

Subagents are useful because CLI resolution and terminal/picker interaction are
independent implementation areas, while a final review can focus on regression
and terminal safety. Do not spawn them during planning; use them only during
`/goal implement the plan`.

All implementation subagents use **gpt-5.6-terra** by default, as requested for
this jtv effort. The orchestrator may use gpt-5.6-sol up to high reasoning only
if a focused integration problem needs it.

1. **Target resolver implementer — gpt-5.6-terra, high thinking**
   - Boundary: `src/target.rs`, minimal `src/cli.rs` integration, resolver/CLI
     tests, and target-resolution docs.
   - Owns: AC1–AC3.
   - Must preserve hidden callback parsing and report exact changed files,
     parser/resolver commands, overlap output, and residual ambiguity.
2. **Prompt and picker implementer — gpt-5.6-terra, high thinking**
   - Boundary: `src/input.rs` if needed, `src/parameters.rs`, `src/picker.rs`,
     focused unit/contract tests and fake picker support.
   - Owns: AC4–AC5 below the full real-TV layer.
   - Must prove terminal state is released across nested processes, query is a
     separate argv, opaque IDs remain intact, and arbitrary path text is not
     shell interpreted.
3. **Interactive test implementer — gpt-5.6-terra, high thinking**
   - Starts after prompt/picker interfaces stabilize.
   - Boundary: PTY scenarios, real-TV workflows, fixtures, and only necessary
     snapshots; no production behavior changes.
   - Owns: BDD scenarios 3–4 at E2E level plus overlap-warning timing evidence.
4. **Final diff reviewer — gpt-5.6-terra, high thinking**
   - Read-only review after integration for CLI compatibility, unsupported
     filename creep, prompt regressions, terminal restoration, path identity,
     cancellation, Windows/MSRV issues, and dirty-tree preservation.

The orchestrator integrates in phase order, keeps subagent file ownership from
overlapping, resolves findings, runs the global matrix, and alone decides done.

## Definitions Of Done

- `jtv docker` resolves exactly AC1's module plus four file candidates and no
  other convention.
- Every individual file layout, precedence combination, invalid name, no-match
  case, and explicit-flag conflict has deterministic automated coverage.
- Overlap produces one stdout warning after the root TV exits, in exact
  precedence order, without contaminating source/callback output.
- TAB from an ordinary string prompt opens real nested Television recursively
  at invocation cwd; the typed buffer is the initial query.
- Selecting replaces the buffer with a relative path; cancellation preserves
  buffer/cursor; manual input and defaults remain unchanged.
- Files, directories, hidden entries, spaces, metacharacters, Unicode, empty
  results, symlinks, malformed picker output, and large-enough fixture trees
  have targeted behavior or safety tests.
- Choice, boolean, configured file/directory, secret, singular, variadic, and
  later-positional paths retain their specified behavior.
- No displayed path or colored text is trusted as action identity; all process
  arguments remain argv elements, never shell fragments.
- Ctrl-C/Escape status, temporary cleanup, echo/cursor/raw-mode restoration,
  and a subsequent normal shell command are verified under PTY.
- Existing visual UX changes and unrelated user files are preserved.
- Help, README, configuration, and testing docs match actual behavior.
- Formatting, lint, unit, contract, real-TV, snapshot, soak, MSRV, release
  artifact, and audit checks pass, or any unrelated pre-existing failure is
  evidenced and reported.
- Final implementation report lists changed files, exact commands and results,
  real E2E observations, warning examples, residual platform risks, and cleanup.

## Verification Plan

| Scope | Command/check | Expected observable result | AC / BDD |
|---|---|---|---|
| Resolver unit/integration | `cargo test --all-features --test target_resolution -- --test-threads=1` | Exact order; only four layouts; module first; conflicts/no-match stable | AC1–AC3; scenarios 1–2 |
| Prompt/picker units | `cargo test --all-features --test parameters -- --test-threads=1` plus picker tests | TAB event/state, selection replacement, cancellation preservation, types/defaults unchanged | AC4–AC5; scenarios 3–4 |
| OS contracts | `just test-contract` | Exact `--input` argv, opaque output, warning timing, malformed output rejection | AC2–AC5 |
| Real TV workflows | `just test-tui` | Actual TAB opens TV, query filters, selection/cancel works, argv is exact | AC4–AC5; scenarios 3–4 |
| Reviewed rendering | `just test-snapshots` | No unintended snapshot drift; any prompt hint is reviewed | AC3–AC4 |
| Reliability | `just test-tui-soak 10` | Ten independent runs without hangs, flakes, leaked children, or terminal damage | AC4–AC5 |
| Native gate | `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && just test-fast` | Formatting, lint, and cross-platform tests pass | all |
| Compatibility | `cargo +1.85.0 check --locked --all-targets --all-features` | MSRV remains valid | AC2, AC4 |
| Release/security | `just verify-release-artifacts && cargo audit` | Release contents unchanged except intended binary behavior; no known vulnerable dependency | AC5 |

Manual exploratory verification remains useful but is not the acceptance gate:

1. In a temporary project, create a root `docker` module plus two approved file
   candidates; run `jtv docker`, inspect module recipes, exit, capture the exact
   warning, then remove the fixture files.
2. Select a recipe with an ordinary string, type a partial nested path, press
   TAB, select a file, submit and decline confirmation; capture the terminal
   frame and verify nothing executed.
3. Repeat, cancel nested TV, confirm the buffer remains, finish manually, then
   run `printf 'terminal-ok\n'` to prove restoration. Delete the fixture tree.

## Risks And Questions

### Safe-to-execute assumptions

- “All files recursively” means the archived behavior: both files and
  directories are selectable.
- Candidate precedence is module first, followed by the four paths in the exact
  order supplied by the user.
- An overlap warning is informational; it does not prompt and does not block
  deterministic selection.
- Reserved `init`, `doctor`, and hidden command names remain commands rather
  than possible target names.
- Explicit selectors do not participate in convenience resolution or warnings.

### Needs confirmation

- None. The plan avoids adding conventions or prompt types beyond the request.

### Blocks implementation

- None at planning time.

### Material risks

- **CLI parsing:** optional positional plus subcommands can subtly change help
  or error behavior; parser tests must precede launch changes.
- **Root discovery:** duplicating `just` discovery would drift. Use `just` as
  authority and introduce typed internal outcomes instead of stderr scraping.
- **Terminal state:** nested fullscreen TV from a live line editor is the main
  engineering risk; raw mode and cursor state must not cross process launch.
- **Unicode:** byte-index editing will corrupt multibyte input or cursor cells;
  use character/grapheme boundaries and display widths.
- **Scale:** recursive enumeration is O(entries) and currently materialized.
  Test a representative tree and record latency/memory; do not introduce an
  undocumented cap because the user requested all paths. Stop and redesign as
  a streaming source if interaction becomes visibly slow.
- **Platform:** PTY E2E is strongest on Linux/macOS; Windows must retain unit
  and process-contract coverage and remain nonblocking for real TV until its
  upstream capability is reliable.
- **Dirty overlap:** existing visual files are modified. Do not normalize or
  regenerate unrelated snapshots.

## /goal Execution Contract

1. Treat this plan as advisory but binding on scope until contradicted by fresh
   repository or runtime evidence.
2. Before editing, re-check `git status --short`, dirty-path overlaps, current
   CLI parsing, prompt/picker interfaces, root discovery behavior, Television
   0.15.9 `--input`, and the four candidate assumptions.
3. Stop and re-plan if evidence contradicts scope, file ownership, CLI
   contracts, callback consumers, terminal behavior, path identity,
   verification feasibility, risk, or any acceptance criterion.
4. Preserve unrelated user changes, especially the active visual UX work and
   its snapshots.
5. Use subagents exactly within their task boundaries and require concise
   evidence: files touched, decisions, commands, failures, residual risks, and
   owned definition-of-done status.
6. Run implementation until every definition of done is met, including real
   nested-Television E2E verification; do not substitute manual testing for the
   PTY gate.
7. Finish with evidence: changed files, checks and real workflows run, results,
   exact warning output, residual risks, and cleanup performed.
