# jtv shell history integration plan

## Proposed outcome

Add an opt-in `jtv shell-init zsh` wrapper that leaves standalone jtv unchanged,
adds every non-secret attempted `just ...` command to live/persistent zsh history,
feeds zsh-autosuggestions naturally, and records the same command lifecycle in
Atuin.

- **Execution readiness:** Ready
- **Discovery confidence:** High
- **BDD:** Used — correctness crosses interactive zsh history, plugin behavior,
  per-command execution state, and Atuin's start/end lifecycle.

## User Problem Trace

- A plain `jtv` invocation currently leaves only `jtv` in shell history, so the
  repeatable composed command is absent from recall and autosuggestions.
- With opt-in integration, history must read:

  ```text
  jtv
  just --justfile /project/justfile deploy
  ```

- Keep both entries; never replace the user's typed `jtv` entry.
- Multi-selection produces one history entry per command actually attempted, in
  jtv's real deterministic execution order. Television 0.15.9 exposes marked
  selections as an unordered set, so mark order is unavailable and must not be
  inferred.
- Record successful, failed, and dry-run attempts. Do not record declined,
  cancelled, or never-reached queue entries.
- If an emitted command contains a parameter explicitly configured as
  `type = "secret"`, silently omit it from native history and Atuin. Do not guess
  secrets and do not add a redacted placeholder entry.
- Support zsh-autosuggestions and Atuin as first-class contracts.
- Installation is documentation plus `eval "$(jtv shell-init zsh)"`; never edit
  `.zshrc` automatically.

Non-goals: Bash/fish adapters in this increment, direct `$HISTFILE`/Atuin SQLite
editing, history replacement, secret heuristics, a persistent jtv history store,
or changing Television selection/execution order.

## Acceptance Criteria

### AC1 — Strictly optional zsh integration

**Explicit.** Standalone `jtv` has identical behavior and writes no synthetic
history. `jtv shell-init zsh` prints a sourceable wrapper; only calls through that
wrapper enable integration. The wrapper preserves stdout/stderr, TTY ownership,
arguments, signals, and jtv's exact exit status, and never modifies shell startup
files.

### AC2 — Exact native history and autosuggestions

**Explicit.** After an integrated run, live zsh history contains the typed `jtv`
entry followed by one executable, shell-quoted `just` entry per command actually
attempted, in execution order. Entries persist under normal zsh history modes and
are immediately returned by zsh-autosuggestions' `history` strategy. Spaces,
quotes, metacharacters, Unicode, newlines, absolute Justfile paths, flags,
variadics, and dry-run argv round-trip without execution or reinterpretation
during insertion.

### AC3 — Accurate Atuin records

**Explicit.** When the wrapper detects an active Atuin zsh session, jtv brackets
each non-secret attempted command with Atuin's supported `history start` and
`history end --exit ... --duration ...` lifecycle, using the command's real cwd,
status, and measured duration. The ordinary outer `jtv` record remains. Missing,
inactive, filtered, or failing Atuin is a best-effort no-op and never changes jtv's
status or terminal output.

### AC4 — Exact secret and queue policy

**Explicit.** `CommandPlan` marks a command history-ineligible only when an
argument emitted for a parameter explicitly tracked by `CollectedParameters` as
secret. That command produces no native or Atuin synthetic record and no redacted
substitute. No name/value/token heuristics are introduced. Mixed queues record
eligible attempted commands independently; stop-on-failure excludes later plans.
Secret suppression is silent during normal use and explained by `jtv doctor` plus
documentation.

### AC5 — Private, failure-isolated transport

**Inferred — Safe to execute.** The wrapper and nested `__tv-run` callback exchange
history records through a private, bounded, versioned session transport that
survives the jtv → Television → callback process chain. It is symlink-safe,
owner-only, unambiguous for arbitrary supported argv, cleaned on every exit path,
and never becomes a command parser. Malformed/cross-session records are ignored
without executing text.

## Repo Evidence

- Baseline worktree: only `justfile` is dirty, with user-owned `build` and
  `install` recipes; implementation must preserve it.
- Reuse:
  - `CollectedParameters::{is_secret,redact}` already tracks only configured
    secret parameter names (`src/parameters.rs`).
  - `CommandPlan` owns exact `program`, `cwd`, `Vec<OsString>` argv and safe
    display quoting (`src/command.rs`).
  - `run_queue` is the authoritative ordered attempt/stop-on-failure boundary
    (`src/runner.rs`).
  - Existing fake-process, PTY, interactive-shell, secret-sentinel, cleanup, and
    real-TUI harnesses cover the needed boundaries.
- Modify:
  - `CommandPlan` needs explicit history eligibility and a zsh-executable
    serializer distinct from diagnostic `display_redacted()`.
  - `Executor`/`run_queue` must return or observe per-attempt status/duration.
  - CLI gains `shell-init zsh`; `doctor` gains wrapper/plugin/Atuin diagnostics.
- Avoid: parsing rendered execution plans, writing `$HISTFILE` directly, editing
  Atuin SQLite, invoking private plugin functions, treating `$SHELL` as the active
  shell, or allowing history integration failure to affect recipe execution.
- Local evidence: zsh 5.9.1, Atuin 18.16.1, and zsh-autosuggestions are available.
  Atuin exposes `history start [COMMAND]...` and
  `history end --exit <status> --duration <duration> <id>`; its official model is
  preexec/start followed by precmd/end.
- Public consumers affected: CLI help/completions and documented setup only. No
  persistent schema or migration exists; the transport is ephemeral/private.

## Implementation Plan

### 1. Make attempted execution observable

- Add `contains_secret`/`history_eligible` to `CommandPlan`, set while building
  emitted argv from the existing configured-secret metadata. Omitted secret
  parameters do not suppress an otherwise safe command; emitted secret values do.
- Add a shell-specific executable serializer with byte/argument round-trip tests;
  do not reuse `[REDACTED]` diagnostic rendering.
- Extend the runner with an observer/result containing plan, start/duration,
  status, and attempted state. Preserve deterministic order and early failure.

Likely files: `src/{command,runner,parameters}.rs`,
`tests/{command_plan,runner}.rs`.

### 2. Add a private optional integration transport

- Generate a private wrapper session only from `shell-init zsh`; pass a versioned
  marker and sink through the Television callback environment.
- Have `__tv-run` emit only completed attempt records, one per eligible command.
  Use NUL-safe/length-prefixed data or an equivalently unambiguous protocol; bound
  size/count, validate ownership/session, create files atomically, and clean up.
- Standalone launches and hidden callbacks without the marker perform zero history
  I/O. Transport failure is silent/best-effort and cannot change command status.

Likely files: new `src/history.rs`, `src/{cli,session,runner,cleanup}.rs`, protocol
and hostile-input tests.

### 3. Generate the zsh wrapper

- Add `jtv shell-init zsh`, outputting static reviewed zsh that calls
  `command jtv "$@"`, imports eligible records with zsh builtins, removes private
  state, and returns the saved jtv status.
- Keep the typed `jtv` history event and append synthesized entries afterward.
- Do not touch ZLE widgets or plugin-private functions; zsh-autosuggestions must
  discover the entries through native history alone.
- Cover common default, `INC_APPEND_HISTORY`, and `SHARE_HISTORY` behavior without
  forcibly rewriting `$HISTFILE`.

Likely files: `src/{cli,history}.rs`, an embedded zsh asset if clearer,
`tests/{cli,zsh_history_pty}.rs`.

### 4. Integrate Atuin at the real process boundary

- The wrapper enables Atuin only when both its executable and active session are
  present, passing the resolved executable as data, not shell text.
- Immediately before each eligible `just`, invoke `atuin history start -- <line>`
  in the plan cwd; after it exits, invoke `history end` with actual status and
  measured duration. Handle empty/filtered IDs and all Atuin errors silently.
- Never call Atuin for configured-secret, cancelled, declined, or unattempted
  commands. Keep the outer Atuin-managed `jtv` event untouched.
- Pin a tested Atuin version in CI while keeping absence fully supported.

Likely files: `src/{history,runner,cli}.rs`, fake Atuin contracts and an isolated
real-Atuin integration test.

### 5. Diagnostics, docs, and full verification

- `jtv doctor`, when invoked through the wrapper, reports integration active,
  native zsh support, zsh-autosuggestions detection, and Atuin availability/
  session/capability. Outside the wrapper it explains that history integration is
  optional. It must never expose history contents or secrets.
- Document the single `.zshrc` line, two-entry behavior, deterministic queue order,
  Atuin duplication by design, secret policy, removal, and troubleshooting.
- Run the complete native/contract/PTY/real-TV/snapshot/soak/MSRV/Windows/release/
  RustSec matrix; standalone snapshot and process-count behavior must not drift.

Likely files: `README.md`, `docs/{architecture,configuration,testing}.md`, CLI and
doctor tests, CI/tool setup if Atuin/zsh-autosuggestions fixtures are pinned.

### Stop-and-replan triggers

- Television drops or sanitizes the private transport across action callbacks.
- Native zsh insertion cannot produce immediate autosuggestions without private
  plugin APIs or cannot round-trip supported argv.
- Atuin's supported CLI cannot represent nested per-command records while its
  outer `jtv` lifecycle is active.
- Achieving persistence requires direct history-file/SQLite mutation.
- Secret values reach any sink, Atuin argv/log, diagnostic, artifact, or test
  transcript, or integration changes standalone execution/status/TTY behavior.

## BDD Scenarios

```gherkin
Feature: Recall commands executed through jtv

  Scenario: Opt-in execution becomes immediately reusable
    Given zsh history integration, zsh-autosuggestions, and Atuin are active
    When I use jtv to run a recipe successfully
    Then history retains the jtv command followed by its executable just equivalent
    And typing the beginning of that just command offers it as an autosuggestion
    And Atuin records the just command with its cwd, success status, and duration

  Scenario: A failing multi-selection records only attempted commands
    Given I selected several recipes in jtv
    When an eligible recipe fails before the queue finishes
    Then native history and Atuin contain each attempted command in execution order
    And the failed command carries its failure status in Atuin
    And unattempted commands are absent

  Scenario: Configured secrets leave no synthetic history
    Given one attempted recipe emits a parameter configured as secret
    And another attempted recipe has no configured secret
    When jtv finishes the queue
    Then neither native history nor Atuin contains any record for the secret command
    And no redacted substitute is added
    And the eligible command is recorded normally

  Scenario: Standalone jtv remains unchanged
    Given shell history integration is not loaded
    When I run or cancel jtv
    Then jtv performs no synthetic history integration
    And its terminal behavior and exit status match the current application
```

Mapping: scenarios 1–4 cover AC1–AC5 through native zsh/Atuin PTY E2E, queue
contracts, secret sentinels, and standalone regression tests respectively.

## Subagent Plan

Use subagents during implementation because zsh/plugin behavior and Atuin lifecycle
can be proven independently before integration. Implementation subagents use
**gpt-5.6-terra** by default; **gpt-5.6-sol** up to high reasoning may be used for
a focused integration problem.

1. **Zsh investigator/implementer — terra, high:** wrapper, native history,
   persistence modes, zsh-autosuggestions PTY contract; no runner/Atuin edits.
2. **Atuin investigator/implementer — terra, high:** supported start/end contract,
   nested lifecycle, fake and isolated real-Atuin tests; no zsh widget work.
3. **Core execution implementer — terra, high:** secret eligibility, serializer,
   attempt observer, private transport and unit/contracts.
4. **Final reviewer — terra, high:** read-only audit for secret leakage, shell
   injection, status/TTY regression, plugin coupling, cleanup, and dirty-worktree
   preservation.

Each reports files, decisions, commands/results, residual risks, and owned DoD.
The orchestrator integrates and owns global verification.

## Definitions Of Done

- Standalone jtv performs no history I/O and all existing behavior/tests remain
  unchanged.
- The opt-in wrapper yields `jtv` followed by exact attempted `just` commands in
  native zsh history; persistence and immediate zsh-autosuggestions are proven in
  real interactive zsh.
- Atuin contains matching per-attempt records with cwd, status, and duration while
  preserving its ordinary `jtv` entry.
- Queue order matches actual deterministic execution; failures are recorded and
  later unattempted commands are absent.
- Every emitted configured-secret command is silently absent from both stores;
  no guessing, redacted entry, plaintext leak, or routine notice exists.
- Decline, Escape, Ctrl-C, SIGTERM, malformed transport, missing plugins, and
  integration failures leave no synthetic records/residue and preserve jtv status
  and terminal usability.
- `shell-init` output is sourceable/idempotent, never edits startup files, and
  composes with plugin load orders without private APIs.
- Doctor and docs accurately explain activation, support, deterministic selection
  order, Atuin behavior, secret boundary, removal, and troubleshooting.
- Full project gates pass, plus pinned real zsh-autosuggestions and isolated real
  Atuin E2E. Final report includes evidence and residual version/platform risks.

## Verification Plan

| Check | Observable proof | Coverage |
|---|---|---|
| Command/runner units | Exact zsh round-trip, eligibility, timing/status, early-stop attempted set | AC2, AC4 |
| Fake process contracts | Transport survives TV callback; Atuin argv/cwd/start/end exact; failures do not alter status | AC1, AC3, AC5 |
| Interactive zsh PTY | Two entries in order, recall works, status/TTY preserved, clean cancellation | AC1–AC2, scenario 4 |
| Pinned zsh-autosuggestions PTY | Typing a `just` prefix exposes the injected command via history strategy | AC2, scenario 1 |
| Isolated real Atuin | Search returns outer `jtv` plus synthetic commands with real statuses/cwd/durations | AC3–AC4, scenarios 1–3 |
| Secret sentinel E2E | Sentinel absent from history files, Atuin search/DB-facing output, diagnostics, transcripts, artifacts | AC4, scenario 3 |
| Queue real-TV E2E | Eligible success+failure recorded in execution order; secret/unattempted plans absent | AC3–AC4, scenario 2 |
| Canonical gates | `cargo fmt --check`, strict Clippy, full tests/contracts/TUI/snapshots, 10-run soak, Rust 1.85, Windows check, release artifacts, `cargo audit` | All |

Manual confirmation supplements automation: source the generated wrapper in an
isolated zsh home, run one normal and one secret fixture, verify Up-arrow,
zsh-autosuggestion, Atuin Ctrl-R/inspection, exact status, then remove the eval line
and confirm standalone behavior. Delete the isolated home afterward.

## Risks And Questions

### Safe-to-execute assumptions

- Additive history (`jtv`, then `just`) is intentional.
- Execution order—not Television mark order—is authoritative.
- Only emitted values already marked secret by jtv suppress a command; undeclared
  sensitive values behave like ordinary manually typed shell arguments.
- Silent suppression means no redacted entry and no per-run notice; doctor/docs are
  the explanation surface.
- zsh is the only shell adapter in this increment.

### Needs confirmation / blocks implementation

- None. The user resolved all product-policy questions.

### Material risks

- Shell quoting and transport bugs could create non-repeatable or injectable
  history text; round-trip without evaluation is mandatory.
- Atuin's optional CLI is external/versioned and its outer `jtv` hook overlaps the
  nested synthetic lifecycle; pinned real tests must prove coexistence.
- Plugin load order/history options can affect persistence or suggestions.
- Any secret reaching Atuin is especially serious because it may sync remotely.
- Dirty `justfile` changes are user-owned and must not be overwritten.

## /goal Execution Contract

1. Treat this plan as scope-binding until fresh repository/tool evidence contradicts it.
2. Re-check the dirty worktree, current zsh/Atuin/plugin contracts, callback
   environment propagation, and exact runner/secret model before editing.
3. Stop and re-plan on contradiction of optionality, history ordering, Atuin API,
   secret suppression, shell quoting, status/TTY behavior, or any stop trigger.
4. Preserve unrelated user changes, especially the dirty `justfile`.
5. Use subagents only within their bounded areas and integrate centrally.
6. Continue through all definitions of done and real end-to-end verification.
7. Finish with changed files, exact checks/results, zsh/Atuin versions, history
   evidence, secret-leak evidence, residual risks, and cleanup.
