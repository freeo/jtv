# Goal Plan: Rebuild `jtv` as a Rust Application Integrated with Television

Proposed outcome: Replace the Bash prototype with a safe, testable Rust `jtv` binary that uses Television for recipe browsing, previews, choice/file selection, actions, and multi-selection while using `just` JSON as the semantic source of truth.

Execution readiness: **Ready**

Discovery confidence: **High**

BDD: **Used** — the feature is an interactive workflow with cancellation, parameter states, multi-selection, and observable execution outcomes.

## User Problem Trace

### Stated goal

- Preserve the core idea: a truly interactive mode for Justfiles using Television.
- Develop the prototype into a proper application with a clean redesign permitted.
- Implement the agreed target as a compiled Rust CLI, not as another large Bash script.
- Keep Television visibly responsible for fuzzy search and terminal selection rather than building a competing TUI.

### Reported friction, constraints, and gaps

| Symptom or constraint | Plan response |
|---|---|
| The current Bash implementation reconstructs and `eval`s commands. | AC3 and AC4; Phases 3–4 build and execute `Vec<OsString>` arguments without shell evaluation. |
| Recipe and parameter semantics are inferred from human-readable `just --list`/`--show` output. | AC1; Phase 1 parses `just --dump --dump-format json` once into a typed model. |
| Television is used as colored stdout and selected display text is reverse-parsed. | AC2; Phase 2 introduces an opaque-ID protocol and separate TV display/output fields. |
| File completion modifies Bash readline state and opens nested TV from a callback. | AC3; Phase 3 invokes TV as a normal child process for path and choice parameters. |
| The test script is mostly manual, references a missing binary and absent fixtures, and disagrees about the last-command path. | AC5; Phase 5 replaces it with unit, contract, integration, and PTY end-to-end tests. |
| Project-local history can expose secrets and shell subprocess history cannot update the parent shell. | AC4; v0.4 writes no `.just_history` or `.just-tv-last-command`; TV frecency remains available. |
| The idea must remain interactive and tightly integrated with TV. | AC2 and BDD scenarios 1–3; the cable channel owns browsing, previews, keybindings, and selection. |

### Non-goals and scope boundaries

No explicit non-goals were provided. The following are proposed v0.4 boundaries, inferred as safe because they prevent the rewrite from recreating unrelated systems:

- Do not implement a second TUI or import Television's internal Rust application modules.
- Do not preserve the prototype's shell-history or project-history files.
- Do not support arbitrary free-form text entirely inside one uninterrupted TV screen; TV handles collection selection, while a line editor handles scalar and secret input.
- Do not delete the two v0.3.0 scripts during the initial rewrite; retain them as archived reference until the Rust application is verified.
- Do not include package-manager publishing, release signing, or a stable plugin API in this implementation goal.
- Linux and macOS are the primary interactive targets. Windows must compile and pass non-PTY tests, but interactive Windows certification is a later release concern.

## Acceptance Criteria

### AC1 — Structured Justfile discovery and model

**Explicit.** Running `jtv` from a project, or with `--justfile <path>`, discovers the intended Justfile through `just`, invokes the JSON dump interface, and builds a typed model containing public recipe namepaths, documentation, dependencies, modules/groups, positional parameters, defaults, flags, and variadic kinds. Human-readable `just` output is not parsed for semantics.

### AC2 — First-class Television workflow

**Explicit.** `jtv init` installs an idempotent, versioned `jtv-recipes.toml` cable channel. Running `jtv` opens that channel with searchable recipe signatures and live previews. TV receives structured rows with an opaque ID distinct from display text, supports Escape cancellation and Tab multi-selection, and cannot cause a recipe name or display string to be reinterpreted as shell syntax.

### AC3 — Interactive parameter collection

**Inferred — Safe to execute.** After selection, `jtv` collects every parameter required by the selected recipe: terminal input for ordinary strings, hidden input for secrets, TV lists for choices/booleans, and TV file/directory selection for paths. Defaults, flags, and variadic values follow `just` semantics. Optional `.jtv.toml` metadata may refine parameter types; unconfigured parameters remain strings.

### AC4 — Safe execution and lifecycle behavior

**Inferred — Safe to execute.** Commands are executed with a direct process argument vector, preserving spaces, quotes, Unicode, newlines where supported by the platform, and shell metacharacters literally. Cancellation executes nothing. Because TV 0.15.9 exposes selected entries as an unordered set, multiple selected recipes run in deterministic recipe-name order and stop at the first failure. `jtv` propagates the `just` exit status, restores the terminal, redacts secret values, and writes no legacy history files.

### AC5 — Automated verification and operability

**Inferred — Safe to execute.** The repository provides automated parser, protocol, parameter-planner, command-safety, real-`just`, fake-`tv`, and real-TV PTY tests; canonical formatting/lint/test commands; CI; installation and configuration documentation; `jtv doctor`; and an end-to-end demonstration that executes a fixture recipe through TV without manual pre-seeding of user configuration.

## Repo Evidence

### Baseline

- `git status --short` was clean before this plan artifact was created.
- The expected planning-turn change is only `docs/plans/jtv-rust-application.md`.
- `main` contains one commit, `0b034e1 add jtv-v0.3.0`.
- The only pre-existing tracked files are:
  - `jtv-0.3.0/just-tv-0.3.0.sh`
  - `jtv-0.3.0/test-just-tv-0.3.0.sh`
- There is no Cargo manifest, README, task file, CI configuration, automated Rust test structure, or repository-level agent instruction file.

### Toolchain observed

- Rust `1.90.0`, Cargo `1.90.0`
- `just 1.53.0`
- Television `0.15.9`
- `shellcheck` and `/usr/bin/expect` are installed.
- `cargo-nextest` and `cargo-audit` are not currently installed.

The v0.4 compatibility floor should initially be the observed `just 1.53.0` and TV `0.15.9`. Lower floors may be claimed only after they are exercised in CI.

### Key evidence and commands

- `git status --short`, `git ls-tree -r --name-only HEAD`, and `git log --oneline --decorate -5` established the baseline.
- `rg -n 'tv|--source-command|--preview-command|just --list|just --show|eval|prompt_param|history|read -p' jtv-0.3.0/*.sh` located integration and safety hotspots.
- `just --dump --dump-format json` on a sample recipe confirmed structured top-level `recipes`, `modules`, `groups`, `aliases`, `settings`, and parameter fields including `default`, `flag`, `kind`, `long`, `short`, and `pattern`.
- `tv --help` confirmed current support for source display/output templates, preview commands, input configuration, custom config/cable directories, and remote-control suppression.
- Static inspection found the legacy `eval` at `jtv-0.3.0/just-tv-0.3.0.sh:870`, generated TV commands at lines 633–635, human-readable parameter parsing beginning at line 658, and dynamic completion source strings beginning at line 759.
- The legacy test targets a nonexistent filename at `jtv-0.3.0/test-just-tv-0.3.0.sh:24` and contains repeated manual `read -p` gates.

### Pattern classification

**Reuse**

- The UX intent: searchable recipes, documentation/body preview, module visibility, defaults, path selection, multi-selection, and clear failure reporting.
- `just` as the parser and execution authority.
- Television as the fuzzy finder and selection UI.
- The current CLI concepts `--justfile`, `--module`, `--version`, and `--help`, adjusted to normal structured argument parsing.

**Modify**

- Replace ad-hoc colored TV rows with a versioned opaque-ID row protocol and TV display/output templates.
- Replace parameter inference with JSON model fields and optional `.jtv.toml` annotations.
- Replace nested readline callbacks with explicit TV child processes.
- Replace local history with TV frecency only for v0.4.

**Avoid**

- `eval`, `sh -c`, or interpolating user values into shell source strings.
- Parsing ANSI display text, `just --list`, or `just --show` to recover recipe semantics.
- Generated Bash formatter/preview scripts.
- Bash/Zsh readline binding manipulation.
- Tests that depend on human confirmation or files in the user's home directory.

### Consumers and unchecked areas

- The only in-repository consumer is the legacy manual test script; it is already broken and will not constrain the new public interface.
- External aliases, dotfiles, or wrappers invoking the old versioned script are not visible from this repository and remain unchecked.
- The new public contracts are the `jtv` CLI, `.jtv.toml` schema, installed cable channel, and documented environment/config precedence.
- Hidden `__tv-*` subcommands and `JTV_SESSION` are internal protocols and must not be advertised as stable APIs.

## Implementation Plan

### Phase 0 — Establish contracts and scaffold the Rust project

Create:

- `Cargo.toml` and `Cargo.lock`
- `src/main.rs`, `src/lib.rs`, and `src/error.rs`
- `README.md`, `.gitignore`, and a root `justfile`
- `assets/jtv-recipes.toml`
- `tests/fixtures/`, `tests/integration/`, and `tests/e2e/`
- `.github/workflows/ci.yml`

Use Rust edition 2024 with `rust-version = "1.85"`. Prefer a small dependency set:

- `clap` for public and hidden CLI subcommands.
- `serde`, `serde_json`, and `toml` for contracts.
- `thiserror` plus `miette` or `anyhow` at the CLI boundary for actionable errors.
- `dialoguer` for scalar, confirmation, and secret prompts.
- `directories` for platform-aware config/state paths.
- `semver` for compatibility diagnostics.
- `tempfile` for private session state.
- `assert_cmd`, `predicates`, and `pretty_assertions` as test dependencies.

Define the public CLI before adding behavior:

```text
jtv [--justfile PATH] [--module NAMEPATH] [--dry-run]
jtv init [--force]
jtv doctor
jtv --version
```

Define hidden internal commands used only by the cable channel:

```text
jtv __tv-source
jtv __tv-preview OPAQUE_ID
jtv __tv-run OPAQUE_ID...
```

The root `justfile` should expose at least `fmt`, `lint`, `test`, `check`, `e2e`, and `ci` recipes so the project dogfoods `jtv` once the vertical slice works.

### Phase 1 — Build the `just` adapter and typed domain model

Likely files:

- `src/just.rs`
- `src/model.rs`
- `src/invocation.rs`
- `tests/just_model.rs`
- `tests/fixtures/json/*.json`
- `tests/fixtures/justfiles/*`

Implementation details:

1. Represent the execution context as a canonical working directory plus optional explicit Justfile path and module filter. Never convert a path into shell text.
2. Invoke `just --dump --dump-format json` once per `jtv` session, adding `--justfile` only when explicitly requested.
3. Deserialize with tolerant structs using `#[serde(default)]` and explicit handling for unknown fields so additive `just` JSON changes do not crash the application.
4. Recursively flatten public recipes and modules into stable namepaths. Exclude private recipes by default; include aliases as distinct searchable entries pointing to their resolved target.
5. Preserve documentation, dependencies, groups, attributes, parameter order, defaults, flags, help text, and variadic kind in the domain model.
6. Build a preview model from the JSON data. If exact source rendering still requires `just --show`, call it with direct arguments only and treat it as presentation, never semantic input.
7. Turn parse failures, missing Justfiles, unsupported JSON shapes, and `just` version problems into specific diagnostics with remediation.
8. Cache the normalized model in the session state so source and preview helpers do not launch `just` once per row.

Compatibility rule: target `just 1.53.0` first. Add fixtures from any older version before lowering the documented minimum.

### Phase 2 — Implement the Television channel, session, and opaque-ID protocol

Likely files:

- `assets/jtv-recipes.toml`
- `src/channel.rs`
- `src/session.rs`
- `src/television.rs`
- `tests/channel_install.rs`
- `tests/tv_protocol.rs`

Implementation details:

1. Embed `assets/jtv-recipes.toml` into the binary and version it alongside `jtv`.
2. `jtv init` resolves Television's config directory using its documented environment/platform precedence, creates `cable/` if needed, and installs `jtv-recipes.toml` atomically.
3. Installation must be idempotent. If an existing file differs from the currently managed version, fail without overwriting and explain `--force`; `--force` must create a backup before replacement.
4. `jtv doctor` reports `just`, `tv`, channel, config-path, and JSON-contract compatibility independently, with nonzero status when launch would fail.
5. At launch, create a mode-0600 temporary session containing the normalized model, invocation context, a random session identifier, and a map from opaque IDs to recipe namepaths. Do not store parameter values or secrets in it.
6. Export only the session locator needed by hidden helpers, launch `tv jtv-recipes` as a child, wait for it, propagate its status, and remove session state on normal exit or signal-driven unwinding.
7. Emit tab-delimited source records containing a validated opaque ID, display signature, and searchable metadata. Strip or encode tabs, newlines, and control characters in display-only fields.
8. Configure TV's `source.display` to render the signature and `source.output` to return only the opaque ID. The preview and action commands receive only IDs matching a strict ASCII format.
9. Bind Enter to an `actions:run` action in `execute` mode. Preserve TV's default Escape cancellation and Tab multi-selection. Validate actual multi-selection placeholder/separator behavior against TV 0.15.9 before building downstream assumptions on it.
10. Keep all source, preview, and action command templates constant. User input, paths, recipe names, and parameter values must never be interpolated into those shell templates.
11. Tests use a temporary Television config via environment/config overrides and never modify the developer's real TV configuration.

### Phase 3 — Add parameter metadata and the interactive parameter state machine

Likely files:

- `src/config.rs`
- `src/parameters.rs`
- `src/picker.rs`
- `tests/config.rs`
- `tests/parameters.rs`

Configuration contract:

- Search upward from the invocation working directory for an optional `.jtv.toml` unless an explicit config path is later added.
- Key recipe configuration by full namepath, not display label.
- Support initial parameter types `string`, `secret`, `choice`, `boolean`, `file`, and `directory`.
- Treat unknown recipe/parameter entries as actionable configuration errors rather than silently ignoring typos.
- Keep the schema optional; a Justfile with no `.jtv.toml` must remain fully usable.

State-machine behavior:

1. Iterate parameters in `just` order and distinguish positional, flag, and variadic forms from JSON.
2. Ordinary required/default values use a line editor. Empty input accepts an available default; required empty input is rejected with a local message.
3. Secret input disables echo and is redacted from confirmation, logs, errors, and test snapshots.
4. Choice and boolean values launch an ad-hoc TV selection with an internal constant source command and safe opaque output.
5. File and directory values launch an ad-hoc TV picker rooted at the invocation directory. Generate entries inside `jtv`; do not construct `find | grep` shell strings from the query.
6. Variadic parameters collect zero-or-more or one-or-more values according to their `just` kind and validate cardinality before confirmation.
7. Explicit cancellation returns to a safe prior state or exits without execution. Ctrl-C exits with conventional interrupt status and restores echo/terminal state.
8. Before execution, show a redacted, shell-escaped representation for human review, but retain the original argument vector as the only executable representation.

### Phase 4 — Implement command planning and execution

Likely files:

- `src/command.rs`
- `src/runner.rs`
- `tests/command_plan.rs`
- `tests/runner.rs`

Implementation details:

1. Build a `CommandPlan` containing executable path, working directory, and `Vec<OsString>` arguments.
2. Pass `--justfile` as its own argument when applicable, followed by the full recipe namepath and planned parameter arguments.
3. Preserve `just` default semantics: omit accepted trailing defaults where possible; materialize preceding defaults only when a later supplied value makes them positionally necessary.
4. Translate current `just` flag metadata into its documented invocation syntax and cover short, long, value-taking, and boolean flags with fixtures.
5. Implement `--dry-run` by passing `just --dry-run`, not by inventing a second command renderer.
6. Execute selected recipes sequentially in deterministic recipe-name order. TV 0.15.9 does not expose toggle order for a multi-selection. Stop at the first nonzero result and return that exact exit status.
7. Inherit stdin/stdout/stderr so recipes remain genuinely interactive. Ensure the TV process has restored the terminal before `just` begins.
8. Never write `.just_history`, `.just-tv-last-command`, or parent-shell history. Never persist secrets.
9. Add adversarial tests for spaces, single/double quotes, semicolons, command substitutions, pipes, redirects, leading dashes, Unicode, and newlines. Each value must reach the fixture recipe as exactly one intended argument.

### Phase 5 — Replace manual testing with a layered automated suite

Likely files:

- `tests/fixtures/justfiles/basic.just`
- `tests/fixtures/justfiles/modules.just`
- `tests/fixtures/justfiles/parameters.just`
- `tests/fixtures/justfiles/adversarial.just`
- `tests/cli.rs`
- `tests/just_integration.rs`
- `tests/fake_tv.rs`
- `tests/e2e/tv.exp`
- `tests/support/*`

Test layers:

1. **Unit:** JSON compatibility, recursive module flattening, row encoding, ID validation, parameter states, default resolution, flag planning, redaction, and exit-policy logic.
2. **Contract:** fake `just` records its OS-level argv; fake `tv` records its argv/environment and emits selected IDs. Verify cancellation and multi-selection without a terminal.
3. **Real `just`:** generate JSON and execute fixture recipes with `just 1.53.0`, checking observable output and status.
4. **Channel validation:** install the embedded channel into a temporary cable directory and have TV load/list it; assert source/display/output/preview/action contracts.
5. **PTY end to end:** use `expect` with real TV 0.15.9 to navigate a deterministic fixture, inspect the preview, select one and multiple recipes, cancel, enter a parameter, and observe the recipe's marker output.
6. **Regression:** retain an adversarial parameter scenario proving that shell syntax is transmitted literally and no second command runs.

The legacy manual test script remains archived but is no longer a canonical check.

### Phase 6 — Documentation, CI, and migration posture

Likely files:

- `README.md`
- `docs/configuration.md`
- `docs/architecture.md`
- `.github/workflows/ci.yml`
- root `justfile`

Documentation must cover:

- Installation of `jtv`, `just`, and Television.
- `jtv init`, conflict/backup behavior, and `jtv doctor`.
- Default launch, explicit Justfile, module filtering, dry run, multi-selection, cancellation, and exit behavior.
- `.jtv.toml` examples for every supported parameter type, including secret redaction.
- Compatibility floor and tested platforms.
- Why scalar input may temporarily leave the TV screen while file/choice input re-enters TV.
- A migration note: v0.3.0 scripts are archived; `.just_history` and `.just-tv-last-command` are neither read nor written.

CI should run:

- Formatting, Clippy with warnings denied, all Rust tests, and release build on Linux, macOS, and Windows.
- Real-`just` integration on pinned `just 1.53.0`.
- Real-TV channel and PTY E2E on Linux with pinned TV 0.15.9.
- Dependency advisory scanning after the CI workflow installs the selected audit tool; absence of `cargo-audit` locally must not cause an undocumented skip.

Do not delete the legacy scripts in this goal. Mark them archived in the README. A later cleanup may remove them after at least one usable Rust release.

### Stop-and-replan triggers

Stop implementation and update this plan if fresh evidence shows any of the following:

- `just` JSON lacks sufficient information for modules, flags, variadics, or stable namepaths at the supported version.
- TV 0.15.9 cannot deliver multiple opaque IDs to an external action without lossy shell joining.
- TV channel installation/config precedence differs materially across supported platforms from the documented model.
- Secure hidden action commands require interpolating untrusted display or recipe text rather than opaque IDs.
- The requested UX changes to require every scalar keystroke inside one uninterrupted TV instance; that likely requires an upstream TV capability or a custom TUI and is a scope change.
- Existing external consumers or newly discovered repository files depend on versioned legacy script paths or history files.
- Implementation requires importing unstable Television internals instead of the channel/CLI contract.
- The working tree contains overlapping user changes in files a subagent is assigned to modify.

## BDD Scenarios

```gherkin
Feature: Run Justfile recipes interactively with Television

  Scenario: Browse and run a parameterized recipe safely
    Given a project has a documented recipe requiring a target value
    When the user opens jtv, selects that recipe in Television, and enters a value containing shell punctuation
    Then the preview identifies the selected recipe
    And only that recipe runs
    And the recipe receives the entered value exactly as one argument

  Scenario: Cancel without running a recipe
    Given the Television recipe browser is open for a valid Justfile
    When the user cancels the browser
    Then no recipe runs
    And no project history or last-command file is created
    And the terminal is ready for the next shell command

  Scenario: Stop a multi-recipe queue after a failure
    Given the user selects three recipes whose names have a deterministic order
    And the second recipe fails
    When jtv runs the selected queue
    Then the first recipe runs before the second
    And the third recipe does not run
    And jtv returns the second recipe's failure status

  Scenario: Explain an incompatible installation before opening the UI
    Given the required just or Television capability is missing
    When the user runs jtv doctor or attempts to open jtv
    Then jtv identifies the missing capability and how to correct it
    And no partial Television channel or session state is left behind
```

Mapping:

- Scenario 1 → AC1, AC2, AC3, AC4; unit argv tests, fake-tool contract test, and real-TV PTY E2E.
- Scenario 2 → AC2 and AC4; fake-TV cancellation test plus PTY cancellation test.
- Scenario 3 → AC2 and AC4; runner integration test and multi-select PTY E2E.
- Scenario 4 → AC1, AC2, and AC5; `doctor` CLI tests and incompatible fake-tool tests.

## Subagent Plan

Subagents should be used during implementation because the rewrite has three separable high-risk contracts: `just` semantics, Television integration, and safe parameter/execution planning.

All subagents—including implementation workers and reviewers—must use **`gpt-5.6-sol` with low thinking**. Do not substitute another model or thinking level. Every subagent must preserve unrelated user changes and return concise evidence: files inspected/touched, decisions, commands and results, failing checks, residual risks, and owned definition-of-done status.

1. **Just model implementer — `gpt-5.6-sol`, low thinking**
   - Boundary: Phase 1 only after the orchestrator creates the shared Cargo scaffold.
   - Owns: `src/just.rs`, `src/model.rs`, `src/invocation.rs`, JSON/Justfile fixtures, and parser tests.
   - Acceptance criteria: AC1.
   - Verification: targeted parser/model tests and real `just --dump` fixture comparison.

2. **Television integration implementer — `gpt-5.6-sol`, low thinking**
   - Boundary: Phase 2, isolated from parameter/execution code.
   - Owns: channel asset, channel installation, session/opaque-ID protocol, TV helper subcommands, and protocol tests.
   - Acceptance criteria: AC2 and the Television portion of AC5.
   - Verification: temporary config installation, TV channel load, fake-TV tests, and a multi-selection contract spike against TV 0.15.9.

3. **Parameter and runner implementer — `gpt-5.6-sol`, low thinking**
   - Boundary: Phases 3–4 after the domain model interfaces are fixed.
   - Owns: `.jtv.toml` parsing, prompt state machine, command planning, execution policy, redaction, and adversarial argv tests.
   - Acceptance criteria: AC3 and AC4.
   - Verification: unit state-machine tests, fake-`just` argv contract tests, and real-`just` runner tests.

4. **Final security/test reviewer — `gpt-5.6-sol`, low thinking**
   - Boundary: read-only review after integration, followed by bounded fixes assigned by the orchestrator.
   - Owns no implementation area by default.
   - Reviews: all ACs, especially untrusted shell boundaries, temp permissions/cleanup, secrets, installer overwrite behavior, signals, and PTY restoration.
   - Verification: diff review, adversarial static search, and review of final E2E evidence.

The orchestrator owns the scaffold, CLI integration, conflict resolution, documentation, CI, final security fixes, and all global verification. Parallel work must not begin until shared structs and module ownership are committed or otherwise stable enough to avoid overlapping edits.

## Definitions Of Done

- Every acceptance criterion is implemented or explicitly deferred with user approval.
- `jtv` is a Rust binary; no Bash application logic is introduced as its runtime core.
- A valid Justfile opens in the installed TV channel with searchable structured rows and an accurate preview.
- Recipe selection crosses the TV boundary only as validated opaque IDs.
- Required/default, flag, variadic, string, secret, choice, boolean, file, and directory parameter behaviors have tests.
- Adversarial values reach `just` literally through OS arguments; no user-controlled data reaches `eval`, `sh -c`, or a dynamic source/action command string.
- Escape and parameter cancellation execute nothing and restore terminal state.
- Multi-selected recipes run in deterministic recipe-name order, stop on failure, and return the failing status.
- `jtv init` is atomic and idempotent, refuses an unforced overwrite, and backs up before forced replacement.
- `jtv doctor` produces actionable, independently tested dependency/channel diagnostics.
- No execution creates `.just_history`, `.just-tv-last-command`, or a secret-bearing persistent file.
- Unit, contract, real-`just`, channel-load, and real-TV PTY tests pass.
- `cargo fmt`, Clippy with warnings denied, the full test suite, and a release build pass.
- CI covers Linux/macOS/Windows build and test checks, with real TV PTY E2E on Linux.
- README and configuration/architecture documentation describe actual behavior and migration boundaries.
- The two legacy scripts remain untouched or receive only an explicit archival notice; unrelated user changes are preserved.
- Final end-to-end evidence shows a fixture recipe selected in real TV, parameterized, executed, and observed; a separate cancellation check shows no execution.
- The final implementation report lists files changed, commands run, exact results, E2E evidence, residual risks, and cleanup or follow-up work.

## Verification Plan

| Check | Expected observable result | Coverage |
|---|---|---|
| `cargo fmt --all --check` | No formatting diff. | AC5 |
| `cargo clippy --all-targets --all-features -- -D warnings` | No warnings or errors. | AC1–AC5 |
| `cargo test --all-features` | Unit, protocol, configuration, planner, runner, and CLI tests all pass. | AC1–AC5; all BDD scenarios at lower levels |
| `cargo test --test just_integration -- --nocapture` | Real `just` discovers fixtures, emits the expected model, receives exact argv values, and propagates failures. | AC1, AC3, AC4; scenarios 1 and 3 |
| `cargo test --test channel_install -- --nocapture` | Temporary install is idempotent, conflict-safe, backed up on force, and loadable by TV. | AC2, AC5; scenario 4 |
| `cargo test --test fake_tv -- --nocapture` | Selection, multi-selection, cancellation, environment, and status contracts match expectations without a TTY. | AC2, AC4; scenarios 2 and 3 |
| `just e2e` | Real TV 0.15.9 opens the fixture, preview is visible, Enter runs one recipe, Tab queues multiple, Escape runs none, and terminal output remains usable. | AC2–AC5; all BDD scenarios except dependency correction text, which is covered by CLI tests |
| `cargo build --release` | A release `jtv` binary is produced and `target/release/jtv --version` reports the planned version. | AC5 |
| `rg -n 'eval|sh -c|Command::new\([^)]*sh|\.just_history|\.just-tv-last-command' src tests assets` | No runtime shell evaluation or legacy history writes; any fixture/documentation match is reviewed and explained. | AC4 |

### Manual end-to-end check

This check supplements, but does not replace, the PTY automation:

1. Create an isolated temporary project and Television config directory.
2. Add a fixture Justfile containing a documented no-argument recipe, a parameterized recipe, a failing recipe, and a marker-writing recipe.
3. Put the freshly built `jtv` first on `PATH` and point Television configuration at the temporary directory.
4. Run `jtv init`, then `jtv doctor`; capture the reported versions, channel location, and success state.
5. Run `jtv`, filter to the parameterized recipe, verify the preview, select it, enter a value with spaces and `;$(...)`, confirm, and capture the fixture's exact received argument.
6. Reopen `jtv`, select multiple recipes with Tab, and verify execution order and stop-on-failure behavior.
7. Reopen `jtv`, press Escape, and verify no marker or history file appears.
8. Run a normal terminal command afterward to demonstrate restored echo/input state.
9. Remove the temporary project/config/session directories.

Expected evidence is the command transcript, fixture marker contents, exact process statuses, and absence of legacy history files—not merely an exit code or screenshot.

## Risks And Questions

### Safe-to-execute assumptions

- The first Rust release continues the prototype's lineage as `jtv` v0.4.x.
- Initial supported integration versions are `just 1.53.0` and Television 0.15.9; compatibility claims expand only after tests.
- Linux and macOS receive interactive support first; Windows receives compile and non-PTY coverage.
- `.jtv.toml` is optional and project-local; Justfiles remain usable without it.
- TV owns fuzzy selection and frecency; v0.4 does not implement application or shell history.
- Scalar and secret prompts may temporarily use a line-editor screen, while enumerable and path parameters use TV.
- Multi-recipe execution stops on first failure instead of continuing unpredictably.

### Needs confirmation before expanding scope

- License selection and public package publication.
- Whether Windows interactive/PTTY certification is required for the first public release.
- Whether a later version should expose a stable machine-readable `jtv` API or upstream the enhanced channel to Television.
- Whether legacy history import is desired after v0.4; it is intentionally excluded here because of secret-handling risk.

These questions do not block this implementation plan because their associated work is outside the defined v0.4 goal.

### Blocking questions

- None with current evidence.

### Principal risks

- **TV shell boundary:** cable commands are shell commands even though `jtv` itself is safe. Mitigation: constant templates and strict opaque IDs only.
- **TV multi-select ordering:** the Phase 2 spike proved TV 0.15.9 supplies all selected entries but not toggle order. Mitigation: extract every opaque ID through TV's template pipeline and document/test deterministic recipe-name ordering.
- **Just JSON evolution:** additive or changed fields may break strict models. Mitigation: tolerant deserialization, fixtures, version diagnostics, and a declared floor.
- **Terminal cleanup:** nested file/choice pickers and interrupted secret input can leave terminal modes altered. Mitigation: RAII guards plus PTY cancellation tests.
- **Installer ownership:** users may customize the channel. Mitigation: content/version checks, refusal by default, and backup on `--force`.
- **Secrets:** confirmation, snapshots, or errors could expose them. Mitigation: secret type, redacted model, and tests that scan captured output.
- **Dirty-file overlap:** the planning artifact makes the worktree intentionally non-clean. Recheck before implementation and distinguish it from unrelated user changes.

## /goal Execution Contract

The implementing orchestrator must:

1. Treat this plan as advisory but binding on scope until contradicted by fresh evidence.
2. Before editing, re-check the worktree baseline, dirty-path overlaps, key files, installed tool capabilities, and assumptions affecting likely edits.
3. Stop and re-plan if those checks contradict scope, ownership, touched files, contracts, consumers, dependencies, data behavior, verification, risk, user-visible behavior, or any Needs-confirmation/Blocking item.
4. Preserve unrelated user changes and never use destructive cleanup to simplify integration.
5. Use subagents exactly within the boundaries above; every implementation and review subagent must use `gpt-5.6-sol` with low thinking, with no model or thinking-level substitutions.
6. Continue implementation until every definition of done is met, including real-TV end-to-end verification—not merely compilation or unit tests.
7. Finish with evidence: files changed, tests/checks/manual E2E run, exact results, residual risks, and any cleanup or follow-up required.
