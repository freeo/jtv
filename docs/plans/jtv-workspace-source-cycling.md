# jtv workspace source cycling plan

## Proposed outcome

Plain `jtv` becomes a workspace-wide recipe browser whose `Ctrl-S` sources are
Root, Subfolders, Modules, and All, with recursive `justfile`, `.justfile`, and
`*.just` discovery, origin-specific row icons, collision-safe selection, and
execution in each recipe's natural Justfile directory while jtv remains in its
startup directory.

- **Execution readiness:** Ready
- **Discovery confidence:** High
- **BDD:** Used — source transitions, duplicate recipe identity, recursive
  discovery, execution cwd, and recoverable child errors are user-visible
  cross-process behaviors.

## User Problem Trace

### Stated goal

- A project such as:

  ```text
  project/
  ├── justfile
  └── supabase/
      └── justfile
  ```

  should expose `supabase/justfile` recipes when jtv starts in `project/`.
- A selected Supabase recipe executes as if invoked in `project/supabase`, but
  jtv and the user's shell remain in `project` after it finishes.
- `Ctrl-S` cycles sources in this exact order:
  1. Root
  2. Subfolders
  3. Modules
  4. All
- Subfolder rows have a distinct leading icon and path identity; existing
  module-specific icons and styling remain.
- Recursive discovery also includes every `*.just` file because this is a
  common project convention for the user.

### Agreed behavior carried into this plan

- Root rows remain compact and use the existing standalone/core icon rules.
- Subfolder/additional-Justfile rows use `📁`, with `[dir]` as the ASCII
  fallback and no glyph in icon-none mode.
- Module rows retain `🐳`, `🧪`, `🚀`, `📦`, and existing core behavior.
- In All, icons continue to describe recipe origin; they do not change merely
  because the active source changed.
- Discovery respects ignore rules, skips dependency/build metadata trees,
  never follows directory symlinks, canonicalizes paths, and runs once at
  startup. Cycling sources performs no new filesystem walk or `just` process.
- Child Justfile failures are reported visibly after Television exits while
  valid workspace recipes remain usable. The primary root Justfile remains
  strict: its failure aborts launch.

### Scope clarifications inferred safely

- “Subfolders” is the user-facing source name, but internally means every
  additional standalone Justfile below the startup directory. Therefore a
  root-level `docker.just` is included in Subfolders as well as nested
  `supabase/db.just`.
- Recognized candidates are:
  - Basename `justfile` or `.justfile`, case-insensitively, matching official
    `just` root conventions.
  - Any regular file whose extension is exactly lowercase `.just`, including
    hidden `*.just` files.
- A child Justfile's own module recipes remain in Subfolders, because its
  filesystem origin is the defining distinction. Root Justfile modules alone
  populate Modules.
- Files known from `just` JSON as module sources are not loaded again as
  standalone Subfolder targets. They remain available through Modules. Imports
  are not deduplicated unless `just` exposes authoritative source identity;
  an independently runnable imported `*.just` may legitimately remain a
  separate path-qualified target.
- Explicitly scoped launches (`--justfile`, `--module`, or `jtv NAME`) stay
  focused on their chosen target and do not recursively expand the workspace.
  Root/Modules/All filter that focused catalog; Subfolders is empty. This
  preserves the recently implemented target-selection contract.
- Existing root `.jtv.toml` metadata applies to the primary target only.
  Discovered child targets use safe default string prompting in this upgrade;
  namespaced/per-child metadata is deferred rather than allowing a root recipe
  name to configure an unrelated child recipe with the same name.

### Non-goals

- No parent-shell `cd`, directory stack mutation, shell wrapper, or history.
- No execution-history/Recent source, favorites, tags, or persistence.
- No source-specific color palette; origin icons/path labels carry the
  distinction even on stock Television where authored row colors are disabled.
- No per-source rescanning, per-row `just` call, symlink traversal, or shell
  construction.
- No child `.jtv.toml` schema or configuration inheritance redesign in this
  upgrade.
- No promise that marked selections survive a `Ctrl-S` transition until the
  pinned Television behavior is measured. Cross-origin queues are selected and
  tested from All regardless.

## Acceptance Criteria

### AC1 — Exact recursive workspace discovery

**Explicit.** An unscoped `jtv` invocation discovers the primary project through
normal `just` behavior and recursively catalogs additional `justfile`,
`.justfile`, and `*.just` files beneath the directory where jtv started.
Discovery includes root-level and nested `*.just`, respects `.gitignore`, skips
`.git`, `target`, `node_modules`, `.venv`, and equivalent ignored trees, does
not follow directory symlinks, excludes the primary root file, and
canonical-deduplicates candidates.

**Inferred — Safe to execute.** Module source paths exposed by each loaded JSON
dump are excluded as standalone targets, preventing `docker.just` from
appearing both as `docker::…` and as a separate file. Candidate and recipe order
is deterministic by normalized relative path then namepath. Files with
non-UTF-8 relative paths are skipped with a warning because Television's row
protocol is UTF-8.

### AC2 — Four accurate `Ctrl-S` sources

**Explicit.** The installed channel cycles Root → Subfolders → Modules → All:

- **Root:** public, non-module recipes in the primary/focused Justfile.
- **Subfolders:** every public recipe, including internal modules, from
  additional standalone Justfiles.
- **Modules:** module recipes from the primary/focused Justfile.
- **All:** the stable union of the preceding sets without duplicate opaque IDs.

The initial source is Root. Cycling invokes only session callbacks and never
reruns discovery or `just`. Empty sources render safely and remain cyclable.
The query behavior, selection reset/preservation, current-source indicator, and
named-source support are characterized against Television 0.15.9 before final
channel syntax is chosen. A minimum-version bump is permitted only if a required
behavior cannot be delivered safely on the pinned release.

### AC3 — Concise, origin-distinguishable presentation

**Explicit.** Rows remain glanceable:

```text
▶ build
📁 supabase/  migrate
📁 supabase/db.just  seed
🐳 docker::build
🧪 testing::integration
```

Subfolder path labels are relative to jtv's startup directory and are always
shown before the recipe. Conventional `justfile`/`.justfile` labels collapse to
their containing directory (`supabase/`); `*.just` labels retain the filename
(`supabase/db.just`). Two spaces separate path identity from recipe identity;
no arrows, summaries, or group text are introduced. Parameters, defaults, and
dependencies retain their compact existing grammar. Details and Definition add
a concise `Source`/`Justfile` origin line without changing recipe body fidelity.
Unicode, ASCII, no-icon, color, no-color, compact, and narrow modes retain the
same semantic distinctions.

### AC4 — Opaque identity across duplicate recipes and sources

**Inferred — Safe to execute.** Every selectable row resolves to a workspace
entry containing its target project, absolute Justfile path, relative source
label, recipe namepath, origin, invocation, and target configuration. The same
recipe name may exist in root, multiple child files, and modules without
collision. Source display/search text never becomes action identity. Preview,
Definition, parameter collection, dry-run, normal run, and multi-select all use
the invocation belonging to the selected opaque ID. Malformed, unknown,
duplicate, or cross-session IDs execute nothing.

### AC5 — Correct child execution directory and parent restoration

**Explicit.** Selecting `supabase/justfile` recipe `status` plans a distinct argv
equivalent to:

```text
just --justfile /project/supabase/justfile status
```

The `just` child observes `/project/supabase` as its recipe working directory
under normal `just` semantics. jtv's process cwd and an interactive parent
shell remain `/project` after success, failure, dry-run, cancellation, and
Ctrl-C. Paths with spaces, quotes, shell metacharacters, and Unicode remain
single OS arguments. An All-source queue spanning root and multiple child
Justfiles executes in deterministic source-path/namepath order and stops on the
first failure using the existing queue policy.

### AC6 — Bounded failure, compatibility, and lifecycle behavior

**Inferred — Safe to execute.** Workspace scanning is bounded and observable:
each accepted Justfile is dumped at most once at launch, with bounded parallel
workers; callbacks add zero `just` calls. Unreadable, non-UTF-8, or invalid child
targets are skipped and collected into one post-TUI stdout warning with paths;
the primary target and session/channel compatibility checks remain fatal.
Temporary state is private, bounded, protocol-validated, and removed on every
exit path. The ephemeral session protocol may be bumped without migration,
because channel callbacks and the parent use the same installed jtv binary.
Modified user channel files retain the existing refuse/backup-on-force policy.

## Repo Evidence

### Baseline worktree

The worktree is intentionally dirty from the just-completed named-target and
TAB-picker implementation plus earlier reviewed visual snapshots:

- Modified: `README.md`, `docs/configuration.md`, `docs/testing.md`,
  `src/cli.rs`, `src/lib.rs`, `src/parameters.rs`, `src/picker.rs`, CLI/prompt/
  picker tests, real-TV workflows, helper, contract tests, and six snapshots.
- Untracked: `src/input.rs`, `src/target.rs`, and
  `tests/target_resolution.rs`.
- Likely workspace edits overlap `src/cli.rs`, `src/lib.rs`, documentation,
  Television/session tests, and snapshots. All current changes are user-owned;
  implementation must integrate rather than reset or recreate them.

### Key evidence and current constraints

- `src/session.rs::SessionState` stores exactly one `Invocation`, one `Project`,
  one global `Config`, and maps opaque IDs only to recipe namepaths. This cannot
  safely represent duplicate namepaths from multiple Justfiles.
- `src/television.rs::{rows,preview,definition_preview}` always resolve through
  that one project/invocation/config. Source output currently has no view
  parameter.
- `src/cli.rs::tv_run` converts IDs into a `BTreeSet<String>` of namepaths,
  which would collapse same-named recipes from different targets. It builds
  every command from the global invocation and config.
- `src/just.rs::load_project` already uses machine-readable
  `just --dump --dump-format json`; the JSON contains an absolute `source` for
  module dumps, but current `Dump` discards it. No human-readable recipe output
  needs parsing.
- `src/command.rs::build_plan` already constructs argv without a shell and
  carries `cwd`; `src/runner.rs` executes that vector with inherited stdio.
- With an absolute `--justfile`, `just` owns recipe-directory switching. jtv's
  parent never calls `set_current_dir`; this is the right mechanism to preserve.
- `assets/jtv-recipes.toml` currently declares one source command,
  `jtv __tv-source`; Television supports source-command arrays and `Ctrl-S`.
  Named command objects exist in current upstream docs but require an explicit
  0.15.9 capability check before adoption.
- `src/presentation.rs::Icon` has standalone/core/module variants but no
  subfolder origin. `src/television.rs::recipe_row` already centralizes concise
  row grammar and is the correct extension point.
- `Config::load_upward` and validation are namepath-based and cannot safely
  apply one root config to duplicate child namepaths without namespacing.
- Existing unit, fake-process, PTY, VT-cell snapshot, real-TV workflow, soak,
  MSRV, Windows cross-check, release-artifact, and RustSec gates are reusable.

### Discovery commands used

- `git status --short`, `git log -3 --oneline`
- Targeted reads of `src/{session,just,command,model,cli,television,config,
  invocation,runner,presentation}.rs`, `assets/jtv-recipes.toml`, and existing
  tests/docs.
- `rg` for `--justfile`, `current_dir`, `module_filter`, source callbacks,
  row rendering, icon vocabulary, and test consumers.
- A real `just 1.53` JSON dump confirmed module `source` paths are available.

### Pattern classification

- **Reuse:** machine-readable `just` adapter, `Invocation`, argv-only command
  plans, process runner, private session file, opaque IDs, sanitizer/style
  vocabulary, channel installation ownership, PTY harness, fake tools, and
  VT-cell snapshots.
- **Modify:** session/catalog model, just JSON normalization, source callback,
  preview/action resolution, row origin grammar, config routing, channel source
  commands, fixture scenarios, and process-count assertions.
- **Avoid:** namepath-only identity, shell `find`, shell command construction,
  current-directory mutation, parsing `just --list`, per-source reloads,
  following symlinks, scanning ignored dependency trees, inferred hidden IDs,
  and manual-only acceptance.

### Consumers and migration

- Installed `jtv-recipes.toml` calls all hidden callbacks; source callback argv
  changes require a channel reinstall and exact contract tests.
- User scripts consume public flags and `jtv NAME`; these remain compatible.
- Session JSON is ephemeral and private, so a protocol bump has no persisted
  migration, but old/new binary or channel mismatch must fail clearly.
- No database, network API, authorization, secrets migration, or persistent
  user data is involved.

## Implementation Plan

### Phase 0 — Prove Television source-cycling behavior first

1. Build a minimal pinned-TV channel with four distinguishable source commands.
2. PTY-characterize 0.15.9 for:
   - exact `Ctrl-S` order and wraparound;
   - source callback timing and reload count;
   - query preservation/reset;
   - selected row and marked multi-selection behavior;
   - empty source behavior;
   - source indicator/header rendering;
   - string-array versus named `{name, run}` command syntax.
3. Check current upstream primary docs/source for the same contract. Clearly
   mark any inference, and do not depend on unreleased named-source syntax.
4. Choose the lowest compatible channel representation. Prefer named source
   commands (`🏠 Root`, `📁 Subfolders`, `📦 Modules`, `◉ All`) if 0.15.9 passes;
   otherwise use the supported ordered string array plus stable row origins and
   a concise `Ctrl-S source` footer. Do not bump minimum Television solely for
   decorative source names.

Likely files: a small test fixture/channel under `tests/fixtures/`,
`tests/tv_protocol.rs`, `tests/tui_workflows.rs`, and research notes only if
needed. No production mutation until this gate is understood.

### Phase 1 — Build a safe workspace discovery layer

1. Add `src/workspace.rs` with a testable walker and catalog builder. Use a
   Rust library interface, not shell text. Prefer the `ignore` crate for
   `.gitignore` semantics after confirming Rust 1.85 support and audit status.
2. Define candidate matching exactly as AC1. Use startup cwd as workspace root;
   canonicalize candidates, reject paths escaping through symlink components,
   and never follow directory symlinks.
3. Preserve conventional and exact relative labels:
   - `supabase/justfile` or `supabase/.justfile` → `supabase/`
   - `supabase/db.just` → `supabase/db.just`
   - root `docker.just` → `docker.just`
4. Load the primary target first through normal `just` discovery. For every
   accepted dump, retain authoritative module source paths and remove those
   canonical files from standalone candidates, regardless of candidate order.
5. Load remaining candidates once, in bounded parallel batches. Keep successful
   target projects; collect stable path-qualified warnings for unreadable,
   invalid, and non-UTF-8 candidates. Do not let worker completion order affect
   catalog or warning order.
6. For scoped launches, construct a one-target catalog without recursive walk.
7. Add discovery tests for root/nested conventional files, root/nested/hidden
   `*.just`, `.justfile`, capitalization, ignored/excluded trees, spaces,
   Unicode, non-UTF-8, file and directory symlinks, canonical duplicates,
   module-source exclusion, invalid child isolation, stable ordering, empty
   workspace, and a representative larger tree.

Likely files: `Cargo.toml`, `Cargo.lock`, `src/workspace.rs`, `src/lib.rs`,
`src/just.rs`, `tests/workspace.rs`, and JSON/Justfile fixtures.

### Phase 2 — Replace single-project session identity with a workspace catalog

1. Introduce serializable structures equivalent to:

   ```rust
   WorkspaceCatalog {
       root: PathBuf,
       targets: Vec<CatalogTarget>,
       selections: BTreeMap<String, SelectionRef>,
       warnings: Vec<WorkspaceWarning>,
   }

   CatalogTarget {
       origin: Root | Subfolder { relative_justfile: PathBuf },
       invocation: Invocation,
       project: Project,
       config: Config,
   }

   SelectionRef { target_index: usize, recipe_namepath: String }
   ```

2. Generate opaque IDs only after targets and recipes are deterministically
   sorted. Validate target index, namepath existence, session ID, and protocol
   at every callback boundary.
3. Replace `SessionState::{invocation,project,selections,config}` access with
   catalog resolution returning one coherent entry view: target, invocation,
   recipe, config, origin, display label.
4. Bump the ephemeral session protocol and add negative tests for old version,
   missing target, mismatched target index/namepath, duplicate IDs, unknown ID,
   malformed path/origin, and cross-session callback attempts.
5. Keep the TV binary/presentation/workspace cwd at session level where they
   are genuinely global; do not duplicate them per recipe unnecessarily.
6. Load/validate root config only for the primary target; child target configs
   are `Config::default()` in this phase. Ensure root namepath metadata cannot
   leak into a same-named child recipe.

Likely files: `src/session.rs`, `src/model.rs` or `src/workspace.rs`,
`src/config.rs`, and `tests/{session,config,tv_protocol}.rs`.

### Phase 3 — Add source views and concise origin-aware rows

1. Add a serialized/Clap `SourceView` enum in exact Root, Subfolders, Modules,
   All order and extend the hidden callback to
   `jtv __tv-source --view <view>`.
2. Filter resolved catalog entries by AC2. Sorting is stable by origin class,
   normalized relative path, module namepath, then recipe name; All contains
   each opaque ID once.
3. Extend `Icon` with `Subfolder` (`📁`, `[dir]`, empty) and make recipe-row
   rendering origin-aware. Subfolder origin takes icon precedence even when its
   project contains modules; internal `module::recipe` remains visible after
   the path label.
4. Render conventional folder and exact `*.just` labels using the compact
   two-space grammar in AC3. Include source paths in fuzzy search, while output
   remains the opaque ID.
5. Add origin to Details/Definition headers and execution-plan context without
   adding prose to Results. Preserve sanitization, safe truncation, wide glyph
   behavior, and plain-mode cues.
6. Update `assets/jtv-recipes.toml` with the Phase-0-compatible four-command
   source definition and `Ctrl-S` hint. Update channel protocol/ownership tests
   so an old installed channel fails with `jtv init` guidance.
7. Add semantic row tests and VT style snapshots for each source, All, duplicate
   names, Unicode/ASCII/no-icon, plain/color, wide/narrow, and empty views.

Likely files: `src/{cli,television,presentation,channel}.rs`,
`assets/jtv-recipes.toml`, `tests/{presentation,visual_presentation,
tv_protocol,tv_contract,tui_snapshots}.rs`, and snapshots.

### Phase 4 — Route preview, prompting, and actions through owning targets

1. Change preview and Definition resolution to retrieve the selected catalog
   entry and call `just --show` with that entry's invocation. Details show the
   relative Justfile source and keep config/secret handling target-local.
2. In `tv_run`, resolve IDs directly to distinct catalog entries instead of a
   `BTreeSet<namepath>`. Deduplicate repeated IDs only; never collapse equal
   namepaths from different targets.
3. Sort queues by relative source path then recipe namepath. For each entry:
   - render path-qualified queue context;
   - collect parameters using its recipe/config while retaining startup cwd for
     the ordinary TAB path picker;
   - clone its invocation and merge requested dry-run;
   - build an argv-only command plan.
4. Keep each command plan's process cwd at jtv startup cwd and pass the absolute
   `--justfile`; allow `just` to apply its official recipe-directory semantics.
   Never call process-wide `set_current_dir`.
5. Verify normal, dry-run, failure, decline, Escape, Ctrl-C, and mixed-origin
   queues restore terminal state and leave the interactive parent shell cwd
   unchanged.
6. Ensure child paths and recipe metadata cannot inject shell, ANSI controls,
   templates, callback IDs, or configuration into other targets.

Likely files: `src/{cli,television,command,parameters,session}.rs`,
`tests/{command_plan,just_integration,tv_action_pty,tv_contract,
tui_workflows}.rs`, support helpers, and workspace fixtures.

### Phase 5 — Complete operational UX, warnings, and documentation

1. Print one deterministic post-TUI stdout warning block for skipped child
   Justfiles, after any named-target overlap warning, without contaminating
   source/preview/action callback stdout.
2. Explain Root/Subfolders/Modules/All, the exact discovery patterns including
   `*.just`, ignore/symlink behavior, row labels/icons, scoped-launch behavior,
   child config limitation, error policy, execution cwd, and source-cycle key.
3. Preserve the established no-color limitation with stock Television and the
   external ANSI+display capability gate; workspace icons/text must work without
   that patch.
4. Update doctor/channel diagnostics only if the channel format introduces a
   new minimum capability. Ensure `jtv init` remains idempotent and modified
   channels are never overwritten without `--force` backup.

Likely files: `README.md`, `docs/{architecture,configuration,testing,
visual-design}.md`, `src/cli.rs`, channel assets/tests, and help text.

### Phase 6 — Full integration, performance, and final audit

1. Count external processes in a workspace fixture: one compatibility probe set,
   one JSON dump per accepted target, bounded parallelism, zero extra dump/show
   calls when cycling sources, and cached Definition behavior per target.
2. Exercise a realistic tree with ignored dependency directories and enough
   Justfiles/recipes to measure startup time, serialized session size, and TV
   responsiveness. Record thresholds from the current baseline rather than
   inventing unmeasured guarantees; stop and redesign if session/source callbacks
   become visibly slow or artifacts exceed existing bounds.
3. Run the complete native, contract, real-TV, snapshot, soak, MSRV, Windows,
   release-artifact, and RustSec matrix.
4. Run an independent read-only diff review for source classification, duplicate
   identity, cwd correctness, symlink/ignore escape, callback output ownership,
   terminal restoration, cross-platform behavior, and dirty-worktree preservation.

### Stop-and-replan triggers

- Television 0.15.9 cannot cycle four safe source commands or loses action
  identity in a way stable opaque IDs cannot solve.
- `just` does not preserve recipe-directory semantics for an absolute
  `--justfile` in the supported version, or requires process-wide cwd mutation.
- Module source paths cannot be obtained reliably from the supported JSON
  contract, causing unavoidable duplicate module/standalone rows.
- `.gitignore`-aware discovery cannot be implemented with a Rust-1.85-compatible,
  audited dependency and a small internal walker would materially diverge from
  Git semantics.
- Workspace session size/process count is unbounded or source callbacks require
  reparsing/reloading Justfiles.
- Existing scoped target selection, secret isolation, opaque IDs, queue order,
  or reviewed visual grammar would regress.
- Implementation would need to overwrite unrelated dirty visual/TAB/target work
  or accept unexplained snapshot drift.

## BDD Scenarios

```gherkin
Feature: Browse and run recipes across a Justfile workspace

  Scenario: Cycle through the four workspace sources
    Given the project root has a root recipe, a module recipe, and a Supabase child Justfile recipe
    When I start jtv in the project root
    Then Root initially shows only the root recipe
    When I cycle the source
    Then Subfolders shows the Supabase recipe with its folder identity
    When I cycle the source
    Then Modules shows the module recipe with its existing module icon
    When I cycle the source
    Then All shows each of those recipes exactly once

  Scenario: Discover conventional and named Justfiles recursively
    Given the workspace contains supabase/justfile, services/api/.justfile, docker.just, and db/seed.just
    And ignored and symlinked directories also contain Justfiles
    When I open the Subfolders source
    Then recipes from the four accepted workspace files are path-qualified and selectable
    And ignored or symlink-traversed recipes are absent

  Scenario: Same-named recipes keep their owning Justfile identity
    Given root, supabase/justfile, and db/seed.just each define a recipe named reset
    When I select the Supabase reset recipe
    Then its Details and Definition come from supabase/justfile
    And only supabase/justfile reset is planned and executed

  Scenario: A child recipe runs locally and returns to the startup directory
    Given jtv was started in the project root
    And supabase/justfile has a recipe that records its working directory
    When I run that recipe and return from jtv
    Then the recipe records the Supabase directory
    And my interactive shell remains in the project root
```

Mapping:

- Scenario 1 → AC2–AC4; real-TV source-cycle workflow, row semantics, All union.
- Scenario 2 → AC1, AC3, AC6; walker units, process contracts, source snapshot.
- Scenario 3 → AC4; duplicate-ID unit/contract plus preview/action real E2E.
- Scenario 4 → AC5; real `just` integration and interactive-shell PTY recovery.

## Subagent Plan

Subagents are warranted because discovery/catalog, session identity, rendering,
and real-TUI testing are separable high-risk areas. Do not spawn during planning;
use them only for `/goal implement the plan`.

Implementation subagents use **gpt-5.6-terra** by default. The orchestrator may
use **gpt-5.6-sol** up to high reasoning for a focused integration issue, in
accordance with the goal-plan skill.

1. **Television capability investigator — gpt-5.6-terra, high thinking**
   - Read-only/minimal-fixture Phase 0 spike.
   - Owns the 0.15.9 source-cycle behavior matrix and exact evidence.
   - Must not alter production code or publish upstream changes.
2. **Workspace discovery/catalog implementer — gpt-5.6-terra, high thinking**
   - Owns `src/workspace.rs`, `src/just.rs` source metadata, dependency choice,
     discovery fixtures/tests, AC1 and performance counters.
   - Must not edit TUI rendering or callback execution.
3. **Session/action routing implementer — gpt-5.6-terra, high thinking**
   - Starts after catalog interfaces stabilize.
   - Owns session protocol, catalog resolution, preview/run invocation routing,
     duplicate identities, config isolation, AC4–AC6 unit/contracts.
4. **Source UX implementer — gpt-5.6-terra, high thinking**
   - Owns channel commands, SourceView filtering, subfolder icons/path grammar,
     semantic rendering tests and only intentional snapshots, AC2–AC3.
5. **Interactive test implementer — gpt-5.6-terra, high thinking**
   - Starts after production interfaces stabilize.
   - Owns real-TV source cycling, duplicate selection, child cwd/parent shell,
     cancellation, cross-origin queue, and process-count E2E coverage.
6. **Final reviewer — gpt-5.6-terra, high thinking**
   - Read-only audit after integration for all stop triggers, acceptance criteria,
     dirty-worktree preservation, and unexplained snapshot changes.

Each subagent must preserve unrelated work and return concise evidence: files
touched/inspected, decisions, commands, failures, residual risks, and whether
its owned definition of done is met. The orchestrator alone integrates, runs the
global matrix, resolves findings, and decides completion.

## Definitions Of Done

- Plain `jtv` discovers exactly the accepted conventional and `*.just` files,
  obeys ignore/symlink boundaries, excludes root/module-source duplicates, and
  loads each accepted target at most once.
- `Ctrl-S` cycles Root → Subfolders → Modules → All and wraps safely; callbacks
  add no `just` calls and empty views do not crash or exit.
- Root, child, child-module, and root-module classifications match AC2; All is a
  duplicate-free union of stable opaque IDs.
- Results use the exact compact origin grammar and icon fallbacks in AC3, with
  no arrows, summaries, groups, or loss of existing param/default/dependency cues.
- Duplicate recipe names across arbitrary target paths resolve to the correct
  Details, Definition, config, prompt, dry-run, and execution invocation.
- A child recipe observes its own Justfile directory while jtv and an actual
  interactive parent shell remain at startup cwd after every exit path.
- Mixed root/child queues preserve literal argv, deterministic ordering,
  redaction, cancellation, stop-on-failure status, and temporary cleanup.
- Invalid/unreadable/non-UTF-8 child targets are skipped with one stable
  post-TUI warning; primary target failures remain actionable and fatal.
- Explicit `--justfile`, `--module`, and `jtv NAME` remain focused and their
  existing tests pass.
- Root config never leaks to same-named child recipes; deferred child config
  behavior is documented.
- Session/channel protocol changes reject incompatible callbacks clearly and
  preserve modified-channel backup/refusal behavior.
- Docs and help accurately cover sources, patterns including `*.just`, icons,
  ignore rules, cwd, errors, config scope, and scoped launches.
- Full formatting, lint, unit, integration, process-contract, PTY, real-TV,
  snapshots, 10-run soak, Rust 1.85, Windows cross-check, release artifacts,
  and RustSec gates pass.
- Independent final review reports no unresolved High/Medium finding.
- Final implementation report lists changed files, exact checks/results,
  measured source behavior/performance/process counts, visual evidence,
  warnings, residual platform risks, and cleanup.

## Verification Plan

| Scope | Command/check | Expected observable result | Coverage |
|---|---|---|---|
| Discovery | `cargo test --all-features --test workspace -- --test-threads=1` | Exact patterns, ignores, symlinks, module exclusion, warnings, order, scale | AC1, AC6; scenario 2 |
| Session identity | Session/TV protocol tests through `cargo test --all-features` | Duplicate namepaths resolve by target+recipe; hostile IDs rejected | AC4 |
| Real just semantics | `cargo test --all-features --test just_integration -- --test-threads=1` | Dumps/previews per file; child recipe cwd is child directory | AC4–AC5 |
| Fake process boundary | `just test-contract` | Exact dump count, callback argv/views, no callback reloads, correct per-target run argv | AC2, AC4–AC6 |
| Real source cycling | Targeted ignored `tui_workflows` source-cycle scenario | Root/Subfolders/Modules/All order, rows, empty view, wraparound | AC2–AC3; scenario 1 |
| Duplicate execution | Targeted real-TV duplicate scenario | Path-qualified same-name selection previews/runs only selected child | AC4; scenario 3 |
| Cwd restoration | Interactive-shell PTY scenario | Recipe logs child cwd; shell prints unchanged project cwd afterward | AC5; scenario 4 |
| Mixed queue | Real-TV All-source workflow | Root/child argv and order exact; stop-on-failure status preserved | AC4–AC5 |
| Visual contract | `just test-snapshots` plus semantic style assertions | Folder/module/root icons and compact rows correct in all modes | AC3 |
| Reliability | `XDG_RUNTIME_DIR=/tmp TMPDIR=/tmp just test-tui-soak 10` | Ten clean runs, no flakes, hangs, residue, terminal damage, or extra loads | AC2–AC6 |
| Native checks | `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && just test-fast` | Formatting/lint/all cross-platform tests green | All |
| Compatibility | `cargo +1.85.0 check --locked --all-targets --all-features` and Windows target check | MSRV and Windows compile remain green | AC6 |
| Release/security | `just verify-release-artifacts && cargo audit` | Release helper exclusion and no known vulnerable dependency | AC6 |

Manual exploratory verification supplements but does not replace automation:

1. In a disposable tree, create root, Supabase, nested `.justfile`, root
   `docker.just`, nested `seed.just`, ignored, symlinked, invalid, duplicate-name,
   space, and Unicode fixtures.
2. Start a clean interactive shell in the fixture root, run jtv, capture each
   `Ctrl-S` source and origin row, run the Supabase cwd recorder, exit, then run
   `pwd` and capture the unchanged root.
3. Exercise All multi-select across root and two child targets, dry-run,
   decline, Escape, and Ctrl-C. Capture exact redacted plans/status and verify no
   recipe ran on decline/cancel.
4. Exit and capture the consolidated invalid-child warning; delete the fixture
   tree and any opt-in sanitized test artifacts.

## Risks And Questions

### Safe-to-execute assumptions

- Root-level `*.just` belongs to the Subfolders source because it is an
  additional standalone Justfile despite not residing in a child directory.
- Lowercase `.just` is the exact extension match requested; conventional
  `justfile`/`.justfile` remain case-insensitive per official behavior.
- Child target modules remain Subfolders and use the folder icon; Root Modules
  contains only modules of the primary/focused target.
- Scoped launches remain non-recursive and focused.
- Root `.jtv.toml` is isolated from child targets; child config support is
  explicitly deferred.
- A skipped child warning is preferable to making an otherwise valid workspace
  unusable; primary root failure remains fatal.
- Import-origin duplication may remain when `just` does not expose sufficient
  source metadata; exact path-qualified rows make the standalone target honest.

### Needs confirmation

- None. The user explicitly accepted the prior design and added `*.just`; the
  assumptions above resolve implementation details without expanding product
  scope.

### Blocks implementation

- None at planning time. Phase 0 and stop triggers protect external contracts.

### Material risks

- **Identity:** the current namepath-only session model must be fully replaced;
  partial adaptation could preview or execute the wrong duplicate recipe.
- **Discovery:** `.gitignore`, symlinks, module files, imports, and root-level
  `*.just` can produce duplicates or escape scope without canonical tests.
- **Performance:** one `just` dump per candidate is unavoidable; bounded
  parallelism and cached callbacks are necessary for large monorepos.
- **Configuration:** current namepath-only metadata cannot safely span targets;
  explicit isolation is required until namespacing is designed.
- **Television:** source cycling and marked selections are external behavior;
  pinned real-TV tests must be authoritative.
- **Cwd:** `just` should own recipe cwd; adding `set_current_dir` or using shell
  `cd` would violate restoration and queue safety.
- **Presentation:** folder path identity must survive no-color/no-icon and narrow
  modes without reintroducing the verbose Results design the user rejected.
- **Dirty overlap:** likely files already contain uncommitted accepted work;
  implementation must preserve and extend it in place.

## /goal Execution Contract

1. Treat this plan as advisory but binding on scope until contradicted by fresh
   repository, pinned-tool, or runtime evidence.
2. Before editing, re-check the worktree baseline, dirty overlaps, Television
   0.15.9 source cycling, `just` JSON `source` fields, cwd semantics, config
   isolation, and current test helper contracts.
3. Stop and re-plan if evidence contradicts source order/classification,
   discovery patterns, scoped-launch behavior, identity, cwd, channel/session
   protocol, consumers, performance, verification, security, visual grammar,
   or any stop trigger.
4. Preserve unrelated user changes and never reset or regenerate unexplained
   snapshots.
5. Use subagents exactly within their bounded areas and require concise evidence
   before integration.
6. Continue until every definition of done is met, including actual Ctrl-S,
   duplicate-name, child-cwd, parent-shell, process-count, and 10-run real-TV
   verification.
7. Finish with evidence: changed files, source behavior/version decision,
   process/performance measurements, tests and real workflows, visual results,
   warning examples, residual risks, and cleanup.
