# Goal Plan: Restore and Advance `jtv`'s Visual UX

Proposed outcome: Restore the archived `jtv` recipe-list, preview, prompt, and status visual language in the Rust application, then add clearer metadata, accessibility modes, responsive behavior, and native Television actions without weakening the opaque-ID security model.

Execution readiness: **Partial** — preview, prompt, accessibility, and test work is ready; colored result rows require a small upstream Television capability because Television 0.15.9 and current `main` treat `source.display` and `source.ansi` as incompatible.

Discovery confidence: **High**

BDD: **Used** — visual hierarchy is coupled to browsing, preview cycling, parameter collection, confirmation, cancellation, and terminal capability behavior.

## User Problem Trace

### User's stated goal

- Recover all carefully crafted visual features from the archived Bash `jtv`, including its color coding and command-list presentation.
- Retain the safe, testable Rust application and its tight Television integration.
- Add genuinely useful UX improvements rather than merely recreating the old implementation mechanically.

### Reported friction and plan response

| Symptom or constraint | Evidence | Plan response |
|---|---|---|
| The Rust version works but looks materially worse than the Bash version. | The archived renderer assigns semantic colors, icons, spacing, module identity, preview sections, and prompt styling; `src/television.rs` now emits plain strings. | AC1–AC3; Phases 1–4 restore the presentation contract in safe Rust. |
| Existing snapshots certified the degraded interface. | `tests/support/screen.rs` retains text and cursor state but discards VT cell colors and modifiers. | AC6; Phase 6 makes styles first-class test evidence. |
| ANSI was removed for safety. | `sanitize_field()` strips every control character after display construction. | AC5; Phase 1 sanitizes untrusted fragments first, then applies only renderer-owned SGR sequences. |
| The old script mixed good design with fragile parsing and dead or inconsistent code. | `PREVIEW_TITLE` is unused; `ICON_CORE` ignores `NO_ICONS` in one branch; list parameter/dependency inference parses `just --show` text. | Restore intended behavior and manual-test expectations, not bugs; retain typed Just JSON and opaque IDs. |
| Valuable additions should improve the experience without creating a competing TUI. | Television already supplies preview cycling, actions, panels, word wrap, status/help UI, and adaptive layouts. | AC3–AC4; use native Television capabilities and keep `jtv` responsible only for semantic presentation and safe actions. |
| Rich result rows conflict with the current secure row protocol. | Television 0.15.9 and current `main` select `DisplayProcessor` whenever `display` exists, ignoring `ansi`; the channel specification explicitly calls the fields incompatible. | Phase 0 is a release gate. Add a combined ANSI-display processor upstream or stop for user approval; never expose display text as the action key. |

### Scope boundaries and non-goals

- Do not return to generated Bash formatters, `eval`, shell reconstruction, or parsing styled display text to recover recipe identity.
- Do not replace Television with a `jtv`-owned TUI or override the user's global Television theme.
- Do not reproduce archived defects, dormant constants, duplicate noise, or legacy history behavior.
- Do not add selectable fake module/group header rows. The archived list had core-first ordering and per-row module identity, not section-header entries.
- Do not make `bat` a required runtime dependency. Syntax highlighting remains an optional, validated enhancement with a deterministic fallback.
- Do not make color or emoji the only carrier of meaning.
- Do not add a project-level custom palette in this upgrade. Use the archived ANSI-16 semantic palette, which respects terminal themes; palette customization can follow after the restored design is stable.
- Do not implement copy-to-clipboard or edit-at-source actions: Television 0.15.9 copies raw source rows, and Just JSON does not expose reliable source locations.
- Preserve the unrelated untracked `justfiles/` directory.

## Acceptance Criteria

### AC1 — Faithful, safe recipe-list presentation

**Explicit.** At an empty query, the recipe list presents core recipes first and module recipes afterward, with the archived icon vocabulary and spacing: `▶` for a non-modular file, `🔷` for core recipes in a modular project, `🐳` for docker, `🧪` for test/testing, `🚀` for deploy/deployment, and `📦` for other modules. Recipe names are cyan; module prefixes use their historical semantic color; required parameters are bright red; optional parameter names are bold yellow with green defaults; dependencies are bright magenta. ASCII/no-icon mode has equivalent text labels. Fuzzy selection and actions still use private opaque IDs, never visible or reverse-parsed display text.

### AC2 — Structured, faithful preview with source-quality body rendering

**Explicit.** The default preview restores the module banner, dim separators, parameter/default/required rows, dependency arrows, magenta documentation, light-blue attributes, cyan signature, and readable recipe definition/body. It adds typed parameter help, flag/variadic/type labels, alias target, group, quiet, and shebang metadata where available. A second native Television preview exposes the faithful `just --show` definition. Optional `bat` highlighting is shell-free, SGR-validated, and falls back silently and deterministically.

### AC3 — Cohesive prompts, picker context, confirmation, and diagnostics

**Explicit.** After selection, `jtv` shows a concise styled recipe or queue context; scalar and secret prompts retain immutable parameter identity, required/default/type information, and `[current/total]` progress; nested choice/file/directory pickers identify their purpose; the execution plan is readable, ordered, safely quoted, and redacted; confirmation remains mandatory; failures are concise and visibly distinct on a capable terminal. Plain mode conveys the same information without ANSI.

### AC4 — Valuable native Television additions

**Inferred — Safe to execute.** The channel adds concise input/results/preview headers and accurate key hints, Details/Definition preview cycling, preview wrapping/scroll guidance, and a secondary dry-run action in Television's action picker. Narrow terminals start in a compact/portrait presentation that preserves filtering, selected-recipe identity, preview access, and cancellation; wide terminals retain the current landscape split. No global Television configuration is mutated.

### AC5 — Accessibility, terminal compatibility, security, and performance

**Inferred — Safe to execute.** `--color=auto|always|never`, `--icons=auto|unicode|ascii|none`, `NO_COLOR`, legacy `NO_ICONS=1`, and `TERM=dumb` behave predictably. Every semantic distinction has a text/glyph cue. Dynamic Justfile/config content cannot inject CSI other than whitelisted renderer SGR, OSC 8/52, BEL, CR, C1 controls, erase/cursor commands, or bidi overrides. The source callback remains O(number of recipes) with no per-row subprocess; a preview performs at most one `just --show` and one optional `bat` invocation and remains cacheable by Television.

### AC6 — Style-aware automated evidence and operable release gates

**Inferred — Safe to execute.** Unit, process-contract, PTY, and real-TV tests assert both visible text and VT cell style roles. Canonical Linux snapshots include stable style runs for wide, narrow, no-color, and ASCII-icon states without accepting snapshots in CI. Cross-platform semantic tests, real-TV workflows, MSRV, security, release-artifact, and soak gates remain green. Documentation defines the visual contract, accessibility controls, optional `bat`, channel upgrade, and snapshot review procedure.

## Repo Evidence

### Baseline worktree

- `HEAD` is `6b1eb68 add test suite`.
- The only current dirty path is unrelated and untracked: `justfiles/`.
- Likely edit paths are otherwise clean: `src/`, `assets/jtv-recipes.toml`, `tests/`, `Cargo.toml`, `README.md`, `docs/`, `justfile`, and CI.
- This planning turn adds only `docs/plans/jtv-visual-ux-upgrade.md`.

### Archived visual contract

- `jtv-0.3.0/just-tv-0.3.0.sh:72-108` defines the ANSI-16 palette and icon vocabulary.
- `:217-400` renders classic and modular source rows: icon, module-colored prefix, cyan recipe, two-space metadata gap, yellow/green defaults, red required parameters, and magenta dependencies.
- `:460-539` renders module heading, required/default parameter rows, dependency arrows, and dim separators.
- `:556-625` renders documentation, attributes, signature, body, and optional shebang-aware `bat` highlighting.
- `:633-637` explicitly launches Television with ANSI parsing.
- `:825-853` renders the selected recipe signature before parameter input.
- `jtv-0.3.0/test-just-tv-0.3.0.sh:244-269` makes cyan recipe, yellow parameter, red required marker, and minimal output explicit manual acceptance behavior.
- Archived inconsistencies to correct rather than copy: dormant bold-green `PREVIEW_TITLE`, hard-coded `🔷` despite `NO_ICONS`, and human-output parsing for parameter/dependency semantics.

### Current application and test architecture

- `src/television.rs::signature`, `rows`, and `preview` are the current plain presentation layer.
- `SourceRow` safely separates opaque ID, display, and search fields; `assets/jtv-recipes.toml` selects the opaque ID for preview/actions.
- `src/model.rs` already retains module, group, alias target, dependencies, attributes, quiet, shebang, flags, help, defaults, and variadic kinds.
- `src/config.rs` knows secret, choice, boolean, file, and directory parameter types, but this presentation metadata is not currently available to source/preview callbacks.
- `src/just.rs::render_preview` already obtains `just --show` for presentation without using it as the semantic parser; production preview does not currently use it.
- `src/parameters.rs::DialoguerPrompter`, `src/picker.rs`, and `src/cli.rs::tv_run` own prompts, nested pickers, and confirmation.
- `tests/support/pty.rs` and `vt100` 0.16.2 already reconstruct terminal state. `vt100::Cell` exposes indexed/RGB colors and bold, dim, italic, underline, and inverse modifiers.
- `tests/support/screen.rs` currently throws those attributes away; six canonical snapshots therefore prove layout/text only.
- `docs/testing.md` still calls color and overflow manual concerns while also claiming every objective correctness observation is automated.

### Television integration constraint

- Television 0.15.9 is the newest published tag found by `git ls-remote --tags`.
- Its channel specification marks `source.display` as incompatible with `source.ansi = true`: [channel specification](https://github.com/alexpasmantier/television/blob/0.15.9/docs/reference/03-channel-spec.md).
- `ChannelKind::new` selects `DisplayProcessor` for every `(ansi, Some(display))` combination, both at 0.15.9 and current `main`: [processor selection](https://github.com/alexpasmantier/television/blob/0.15.9/television/channels/channel.rs#L541-L571).
- The present jtv `search` field is effectively not searched because `display = "{split:\t:1}"` makes Television match only that displayed field. This should be corrected, not silently preserved as a claimed feature.
- Preview ANSI, multiple preview commands, headers/footers, word wrap, action picker, and panel configuration are supported by 0.15.9: [channel spec](https://github.com/alexpasmantier/television/blob/0.15.9/docs/reference/03-channel-spec.md), [actions](https://github.com/alexpasmantier/television/blob/0.15.9/docs/reference/02-actions.md).

### Pattern classification

**Reuse**

- Typed Just JSON model, config validation, opaque selection IDs, session file permissions, safe argv planning, redaction, deterministic queue execution, Television's native panels/actions/previews, the PTY/VT harness, sanitized failure artifacts, real-TV workflows, and Linux snapshots.

**Modify**

- Split presentation out of `src/television.rs` into semantic renderers.
- Change sanitization from “style then strip all controls” to “sanitize every untrusted fragment, then serialize trusted style spans.”
- Extend `SessionState` with presentation options and config-derived parameter kinds/secret markers using backward-compatible serde defaults.
- Upgrade the channel contract and installed-channel hash; users will run `jtv init` or `jtv init --force` for modified local copies.
- Make screen snapshots style-aware rather than broadening text normalization.
- Style Dialoguer prompts and the execution plan while preserving cancellation and redaction.

**Avoid**

- Passing recipe/display strings to shell actions, selecting by recipe name or rendered text, concealed/zero-width opaque-ID hacks, trusting arbitrary ANSI from Justfiles or `bat`, true-color assumptions, global Television theme changes, fake selectable headers, per-row `just` calls, and timing sleeps.

### Known consumers and unchecked areas

- In-repository consumers: `src/cli.rs` hidden callbacks, embedded channel installation, fake-TV contracts, real-TV fixtures, screen snapshots, doctor/channel-current checks, README/testing docs, and CI.
- External consumers: installed `jtv-recipes.toml`, shell aliases invoking current CLI flags, and user-modified channel files. The exact-channel ownership policy already protects modified files.
- The session JSON and hidden callback commands are private, same-run contracts; add defaults rather than a persistent migration.
- No database, network service, public API, or persisted user data migration is involved.
- Explicitly unchecked until implementation: user terminal fonts/themes and a tagged Television release containing combined ANSI-display processing.

## Implementation Plan

### Phase 0 — Resolve the Television ANSI-display gate before broad edits

1. Reproduce the constraint with a minimal 0.15.9 channel containing `id<TAB>styled-display<TAB>match-text`, `ansi = true`, `display`, and `output`. Prove current rendering, matching, and selected output behavior in a PTY.
2. Build a local Television spike for a combined ANSI-display processor:
   - Evaluate `display` against the raw row.
   - Parse only the resulting display's ANSI for rendered spans.
   - Match against its ANSI-stripped visible text.
   - Continue evaluating `output` against the untouched raw row, yielding the opaque ID.
   - Add upstream unit/integration tests for ANSI, display, output, fuzzy match, width truncation, and hostile controls.
3. Confirm whether the patch can be accepted in a tagged Television release. Publishing an issue/PR or raising jtv's minimum Television version requires user approval before external writes or a compatibility change.
4. Reject concealed IDs, visible random tokens, recipe-name action keys, and full-row shell callbacks as unsafe/degraded fallbacks.
5. Stop and re-plan if no tagged compatible Television path is acceptable. Preview/prompt work may be implemented separately only with explicit partial-scope approval; AC1 remains incomplete.

Likely external area: `television/channels/{channel,entry_processor}.rs` and its tests/docs. No external write is authorized by this plan alone.

### Phase 1 — Introduce a semantic presentation layer and trusted ANSI boundary

1. Add `src/presentation.rs` with:
   - `StyleRole` values for recipe, module variants, parameter name, required, default, dependency, module header, documentation, attribute, signature, separator/dim, success, warning, and error.
   - `StyledText`/`StyledSpan` that stores sanitized content separately from semantic style.
   - ANSI and plain serializers; only constant, whitelisted SGR sequences may be emitted, with a reset after every span and line.
   - The archived ANSI-16 palette, including exact indexed color and bold/dim semantics.
   - `ColorMode` and `IconMode` resolution without using callback `stdout.is_terminal()`—callbacks are intentionally pipes.
2. Add public `--color=auto|always|never` and `--icons=auto|unicode|ascii|none`. `auto` honors `NO_COLOR`, `NO_ICONS=1`, `TERM=dumb`, and Unicode-capable locale; explicit flags win.
3. Preserve ordinary Unicode, combining marks, and wide glyphs. Neutralize ESC/CSI/OSC/BEL/CR/C1 and bidi override/isolate controls. Single-line fragments replace tabs/newlines; multiline renderers introduce line structure themselves.
4. Derive presentation-only parameter metadata from `.jtv.toml` during launch so previews can label choice/boolean/file/directory/secret types and redact any secret default. Do not store entered values.
5. Unit-test plain/styled visible-text parity, exact style roles, reset discipline, no style bleed, Unicode, and hostile payloads.

Likely files: `src/presentation.rs`, `src/lib.rs`, `src/cli.rs`, `src/invocation.rs`, `src/session.rs`, `src/config.rs`, `Cargo.toml` only if a direct Rust 1.85-compatible styling/terminal-size dependency is genuinely needed, and `tests/presentation.rs`.

### Phase 2 — Restore the recipe list and secure channel protocol

1. Render rows from typed `Recipe`/`Parameter` data, never `just --list` or display parsing:
   - Non-modular: `▶ recipe  metadata` or `[recipe] recipe`.
   - Modular: core/module icon, module-colored prefix, cyan recipe name.
   - Required/default/dependency styling and exact two-space metadata separation.
   - Typed flag spellings and `*`/`+` markers as additive cues; long metadata collapses in compact mode and remains complete in preview.
2. Preserve core-first then module/name ordering for the empty query. Do not disable fuzzy ranking while a query is active.
3. Make the visible row sufficiently descriptive for matching: namepath, compact parameter names, and a bounded dim documentation/group cue where it improves discovery. Do not claim the current hidden third field is searchable.
4. After the Phase 0 Television prerequisite exists, set `ansi = true` while retaining `display` and `output` so the raw row still yields only the opaque ID.
5. Parse-test the embedded TOML and assert source/preview/action templates, not just substrings. Bump channel metadata/version as needed and update init/doctor diagnostics.
6. Add exact hostile-display contracts proving no dynamic ANSI reaches Television and no display/search text becomes an action key.

Likely files: `src/presentation.rs`, `src/television.rs`, `src/channel.rs`, `assets/jtv-recipes.toml`, `tests/tv_protocol.rs`, `tests/tv_contract.rs`, and fake-tool fixtures.

### Phase 3 — Restore and improve the preview using native Television features

1. Default **Details** preview:
   - Bold module banner and dim divider.
   - Recipe title/signature and alias/group/quiet/shebang badges.
   - `Parameters` rows with required/default, flag spelling, variadic cardinality, `.jtv.toml` type, and help text.
   - `Dependencies` as magenta arrow rows.
   - Magenta documentation and light-blue attributes.
   - A readable definition/body with no JSON-array artifacts.
2. Add a **Definition** preview command using existing `just::render_preview` (`just --show`) strictly for presentation. Do not derive execution semantics from it.
3. Restore optional shebang-aware `bat` highlighting:
   - Invoke by argv with sanitized stdin, never a shell.
   - Use an explicit language map and plain style.
   - Accept only SGR from `bat`; strip/reject OSC, cursor/erase, and other controls.
   - Fall back to internal ANSI/plain rendering on absence, failure, timeout, `NO_COLOR`, or invalid output.
   - Force deterministic `bat` absence or a fake binary in canonical tests.
4. Configure preview cycling, header/footer, wrapping, scrollbar, padding, and accurate `Ctrl-F`/PgUp/PgDn hints using 0.15.9 channel capabilities.
5. Render calm empty/stale/error preview states without leaking raw IDs, paths, or host metadata.

Likely files: `src/just.rs`, `src/presentation.rs`, `src/television.rs`, `src/cli.rs`, `assets/jtv-recipes.toml`, `tests/just_integration.rs`, `tests/tv_contract.rs`, and preview fixtures.

### Phase 4 — Make prompts, nested pickers, confirmation, and failures cohesive

1. Replace plain Dialoguer defaults with a presentation-aware theme while retaining its safe input/password behavior.
2. Before parameter collection, show one concise recipe/queue context. Prompts show `[n/total]`, immutable parameter name, required/default/type, and help without echoing secrets.
3. Improve nested picker headers: `Choose <parameter> — choice`, `Select file for <parameter>`, and `Select directory for <parameter>`; preserve direct typed configuration rather than restoring shell-specific TAB heuristics.
4. Present an `Execution plan` with stable recipe order, readable safe quoting, and `[REDACTED]` values. Style the confirmation prompt without adding “Selected/Processing” chatter.
5. Add a Television action-picker entry for **Dry run selected recipes** using the same opaque IDs, parameter flow, redaction, and confirmation; do not add a surprising direct hotkey.
6. Style success/failure/cancellation diagnostics only when the selected color mode allows it. Plain stderr remains stable for pipes and tests.

Likely files: `src/parameters.rs`, `src/picker.rs`, `src/command.rs`, `src/cli.rs`, `src/main.rs`, `assets/jtv-recipes.toml`, and their existing tests.

### Phase 5 — Accessibility and responsive terminal behavior

1. Never rely on color alone: preserve `<required>`, `name:default`, arrows, section labels, type/variadic/flag text, and ASCII icon labels.
2. Use ANSI-16 roles rather than hard-coded RGB so terminal themes control actual contrast. Do not alter Television's global theme.
3. Determine width once before Television launch using a portable, Rust 1.85-compatible terminal API:
   - Wide/default: landscape, current 55% preview, descriptive rows.
   - Narrow (initial target: below 100 columns): portrait/compact rows, full metadata in preview.
   - Store the chosen presentation mode in the private session so callbacks agree.
4. Ensure 80×24, 120×40, wide, and live-resize scenarios keep filter focus, selected identity, preview access, scrolling, key hints, and Escape/Ctrl-C behavior usable.
5. Bound displayed row length and handle emoji width, combining characters, and long docs without panic or broken borders. ASCII mode is the deterministic fallback.
6. If portable width detection or portrait mode is unreliable on a supported platform, stop and choose an explicit `--compact` fallback rather than guessing from `COLUMNS`.

Likely files: `src/presentation.rs`, `src/television.rs`, `src/invocation.rs`, `tests/support/scenarios.rs`, and real-TV fixtures.

### Phase 6 — Make visual semantics a tested product contract

1. Extend `ScreenFrame` with stable style runs derived from `vt100::Screen::cell`: row/column range, text, foreground/background color, bold, dim, italic, underline, inverse, and wide/continuation state.
2. Add helpers such as `style_at_text`, `find_text_in_region`, and focus/cursor assertions. Snapshot normalized visible text plus non-default style runs, including styled blanks needed to represent selection backgrounds.
3. PTY self-tests cover indexed/RGB colors, modifiers, resets, fragmented ANSI/Unicode, combining/wide cells, resize, and alternate screen behavior.
4. Renderer/property tests cover CSI erase, OSC 8/52, BEL, CR, C1 CSI, tabs/newlines, bidi overrides, long content, and malformed `bat` output. The allowed escape vocabulary is SGR-only.
5. Real-TV semantic and snapshot coverage includes:
   - Classic and modular result rows with exact legacy roles.
   - Details and Definition previews.
   - Required/default/secret/flag/variadic/type metadata.
   - Styled prompt, nested picker, execution plan, and failure.
   - `NO_COLOR`, `TERM=dumb`, Unicode icons, ASCII labels, 80×24 compact, 120×40 canonical, and resize.
6. Keep exact style snapshots Linux-only with pinned Television. Run renderer, protocol, VT, and semantic checks on Linux/macOS/Windows.
7. Add a visual determinism soak and secret/path/control scans. CI never auto-accepts snapshots or uploads unsanitized ANSI transcripts.
8. Update `docs/testing.md` so objective color/style/overflow behavior is automated; retain manual exploration only for font rendering, theme taste, and subjective ergonomics.

Likely files: `tests/support/screen.rs`, `tests/support/pty.rs`, `tests/pty_harness.rs`, `tests/tui_snapshots.rs`, `tests/tui_workflows.rs`, `tests/snapshots/`, `docs/testing.md`, `docs/visual-design.md`, `README.md`, `justfile`, and `.github/workflows/ci.yml` if a distinct visual gate is useful.

### Phase 7 — Reliability, compatibility, and release audit

1. Run the complete fast, contract, real-TV, snapshot, narrow-terminal, no-color, release, MSRV, Windows cross-check, audit, and soak matrix from documented prerequisites.
2. Verify source rendering starts no per-row subprocesses; preview cache behavior avoids repeated `just --show`/`bat` calls for the same entry.
3. Scan raw callbacks, screens, snapshots, and failure artifacts for secret sentinels, host paths, untrusted escape sequences, session IDs, and temporary residue.
4. Verify a default release contains only `jtv`; `bat` remains optional; no external TV fork or test helper is packaged accidentally.
5. Independently review visual parity against the archived matrix, ANSI/output security, accessibility/plain modes, Windows/macOS conditionals, and Television patch correspondence.
6. Present before/after canonical screens and style manifests for human review. Subjective refinements may adjust spacing or hints, but cannot remove semantic/accessibility assertions without approval.

## BDD Scenarios

BDD applicability: **Used** because the change defines observable behavior across browsing, preview, parameter collection, confirmation, and terminal modes.

```gherkin
Feature: A safe and expressive interactive Justfile browser

  Scenario: Browse a modular Justfile with the restored visual language
    Given a Justfile with core, docker, test, deploy, and other module recipes
    When the user opens jtv in a color- and Unicode-capable terminal
    Then each recipe has the intended icon and semantic color roles
    And required values, defaults, dependencies, module identity, and recipe identity remain understandable
    And selecting any displayed row resolves only through its opaque internal identifier

  Scenario: Understand a parameterized recipe before running it
    Given a recipe with documentation, attributes, required and default parameters, flags, variadics, dependencies, and a shebang body
    When the user focuses that recipe and cycles its previews
    Then Details explains each semantic part with text and visual hierarchy
    And Definition shows the faithful Just recipe with safe optional syntax highlighting
    And no Justfile-provided terminal control sequence is executed

  Scenario: Collect values and review a safe execution plan
    Given a selected recipe with scalar, secret, choice, file, and directory parameters
    When the user completes each prompt and picker
    Then every step identifies the recipe, parameter, type, and progress
    And the final ordered execution plan shows safe quoting while hiding every secret
    And declining confirmation executes nothing

  Scenario: Remain usable without color, emoji, or a wide terminal
    Given color and icons are disabled in an 80 by 24 terminal
    When the user filters, previews, selects, and cancels a recipe
    Then text labels and symbols preserve every semantic distinction
    And the focused result, preview access, key hints, and cancellation remain usable
    And the terminal is restored afterward
```

Mapping:

- Scenario 1 → AC1, AC5; unit row contracts plus real-TV styled list snapshot.
- Scenario 2 → AC2, AC5; preview callback contracts, hostile-control tests, and Details/Definition snapshots.
- Scenario 3 → AC3, AC5; real PTY prompt/picker/confirmation workflows and fixture-event assertions.
- Scenario 4 → AC4–AC6; 80×24 no-color/ASCII real-TV workflow, resize, cleanup, and style/plain parity tests.

## Subagent Plan

Subagents are useful because presentation/security, Television integration, prompt flow, and style-aware testing have separable high-risk boundaries.

Implementation subagents use `gpt-5.6-terra` by default, but the orchestrator may use `gpt-5.6-sol` up to **high** reasoning based on complexity.

1. **Television protocol investigator/upstream patch author — `gpt-5.6-terra`, high**
   - Boundary: Phase 0 only; local Television checkout, combined ANSI-display processor, upstream tests/docs.
   - Owns: the AC1 prerequisite and a precise tagged-release/compatibility report.
   - Must not publish an issue/PR or change jtv's version floor without user authorization.
2. **Presentation/security implementer — `gpt-5.6-terra`, high**
   - Boundary: `src/presentation.rs`, options/session metadata, sanitization, source row renderer, unit/protocol tests.
   - Owns: AC1 and AC5.
   - Verifies exact palette, plain/styled parity, hostile controls, opaque ID/output, MSRV, and Clippy.
3. **Preview and interaction implementer — `gpt-5.6-terra`, high**
   - Boundary: preview cycling/body rendering/optional bat, prompts, nested picker context, execution plan, dry-run action.
   - Owns: AC2–AC4.
   - Verifies redaction, cancellation, fallback, no shell invocation, and real PTY workflows.
4. **Style-harness and E2E implementer — `gpt-5.6-terra`, high**
   - Boundary: `tests/support/screen.rs`, PTY style tests, snapshots, responsive/no-color scenarios, docs/CI test commands.
   - Owns: AC6 and the cross-platform evidence for AC1–AC5.
5. **Independent final reviewer — `gpt-5.6-sol`, high**
   - Read-only final audit of archived parity, ANSI injection, secrets, style resets, opaque callback IDs, accessibility, process cleanup, platform behavior, snapshot honesty, and every definition of done.

The orchestrator integrates in phase order, prevents overlapping edits, preserves unrelated work, runs the full canonical matrix, resolves reviewer findings, and alone decides global completion.

## Definitions Of Done

- Every active archived presentation behavior in the parity inventory is restored or explicitly superseded by a safer/better behavior documented in `docs/visual-design.md`.
- Dormant constants and archived bugs are identified as such; they are not falsely claimed as parity.
- Colored result rows work through an accepted Television contract while action/preview callbacks still receive opaque IDs only.
- Preview, prompts, nested pickers, execution plan, dry-run action, failures, no-color, ASCII, and compact states meet AC1–AC5.
- Color and emoji are never the sole semantic cue; `NO_COLOR`, `NO_ICONS`, explicit modes, and `TERM=dumb` are documented and tested.
- Justfile/config strings and optional highlighter output cannot inject terminal actions; secret defaults/inputs never appear in list, preview, plan, transcript, artifact, or snapshot.
- No source row invokes `just` or another subprocess per recipe; preview subprocess work is bounded and cached.
- Style-aware unit, contract, PTY, real-TV, and snapshot tests cover all four BDD scenarios.
- Existing execution, cancellation, signal, cleanup, terminal-recovery, MSRV, release-artifact, and security behavior remains green.
- Channel upgrade and minimum Television version are documented; modified user channel files remain protected by the existing ownership/backup flow.
- The independent reviewer reports no unresolved High/Medium correctness, security, accessibility, or parity blockers.
- Final implementation report lists files changed, upstream Television status/version, commands and E2E scenarios run, before/after visual evidence, results, optional dependencies, residual platform/theme risks, and cleanup.

## Verification Plan

| Criterion | Targeted verification | Expected observable result |
|---|---|---|
| AC1 | Presentation unit tests; `cargo test --all-features --test tv_protocol`; fake-TV callback contracts; real modular list snapshot | Exact palette/icon/style roles, core-first initial order, styled/plain text parity, opaque ID is the only callback output. |
| AC2 | `just --show` integration test; fake `bat` success/failure/hostile cases; Details/Definition PTY snapshots | Structured metadata is accurate, body is readable, cycling works, optional highlighting is safe and fallback is identical in content. |
| AC3 | Parameter unit tests; nested picker contracts; real-TV prompt/secret/queue/dry-run workflows | Progress/type context is visible; secrets are absent; ordering/quoting are exact; decline/dry-run has no unintended execution. |
| AC4 | 80×24, 120×40, wide, and resize real-TV tests; channel TOML parse/contract tests | Correct layout mode, usable focus/preview/hints, native action and preview cycling, clean terminal recovery. |
| AC5 | Hostile renderer property table; NO_COLOR/NO_ICONS/TERM tests; artifact scan; source invocation counter | SGR-only trusted output, no style bleed/spoofing/secrets, no per-row subprocess, bounded preview work. |
| AC6 | PTY style self-tests; `INSTA_UPDATE=no just test-snapshots`; cross-platform fast/contracts; soak | Text plus style runs are deterministic; CI rejects drift; semantic tests pass on Linux/macOS/Windows. |

Canonical final commands:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
just test-contract
INSTA_UPDATE=no just test-snapshots
just test-tui
just test-tui-soak 10
just verify-release-artifacts
cargo +1.85.0 check --locked --all-targets --all-features
cargo check --target x86_64-pc-windows-gnu --locked --all-targets --all-features
cargo build --release
cargo audit
git diff --check
```

Implementation must also run Actionlint/YAML validation, scan for `.snap.new`, secrets, host paths, disallowed escape sequences, temp/session/history residue, and surviving `jtv`/`tv`/`bat` processes.

Manual visual review setup:

1. Use an isolated HOME/config/runtime/project containing classic, modular, long, Unicode, secret, alias, shebang, and typed-parameter fixtures.
2. Capture 120×40 color/Unicode, 120×40 no-color/ASCII, and 80×24 compact screens with synthetic values only.
3. Compare list/preview roles against `docs/visual-design.md` and the archived parity matrix; assess light/dark terminal themes without changing the user's global TV theme.
4. Exercise Details/Definition cycling, dry-run action, nested pickers, confirmation decline, Escape/Ctrl-C, and a normal shell command afterward.
5. Preserve sanitized evidence only; remove isolated directories and recordings.

Automated cell-style assertions are the correctness gate. Human review covers terminal-font rendering, theme taste, spacing, and subjective discoverability.

## Risks And Questions

### Safe-to-execute assumptions

- The archived manual expectations and active rendering branches define visual parity; dead constants and obvious bugs do not.
- ANSI-16 semantic roles are preferable to fixed RGB because they respect user terminal themes.
- Current safety improvements—typed JSON, opaque IDs, redacted plans, explicit typed pickers, confirmation, and no legacy history—remain authoritative.
- Adding Details/Definition previews, contextual hints, dry-run action, parameter progress, no-color/icon modes, and compact layout directly improves the requested workflow without changing recipe execution semantics.

### Needs confirmation

- **Television compatibility:** there is no tagged version newer than 0.15.9 and current `main` still lacks combined ANSI-display processing. Publishing an upstream change and later raising jtv's minimum Television version must be approved before those external/public compatibility actions.
- **Partial delivery:** if the upstream prerequisite cannot be obtained, implementing preview/prompt improvements while deferring colored result rows requires explicit user approval; it cannot be called complete recovery.

### No current user-answer blocker for planning

- Phase 0 can be executed locally and produce evidence without external writes. The plan becomes blocked only if it reaches the publish/version decision without prior approval.

### Technical and product risks

- Television fuzzy highlighting/selected-row styling may override semantic source colors. Assert renderer roles directly and real-TV styles on unselected rows; do not hide differences with snapshot scrubbing.
- ANSI result rows in current Television are not reliably width-truncated. Bound row content and test Unicode/narrow terminals before enabling descriptive snippets.
- Emoji cell width varies by font/platform. ASCII labels must remain first-class and canonical tests must not claim font-pixel portability.
- Optional `bat` output varies by version/theme. Validate SGR, use `BAT_THEME=ansi` or a controlled fake for tests, and keep the fallback authoritative.
- Adding prompt styling must not accidentally echo secrets or change Ctrl-C/Escape status behavior.
- The channel update makes existing installed channels outdated by design; modified user copies must never be overwritten without `--force` backup.
- Exact style snapshots remain pinned-Linux evidence; cross-platform gates assert semantic roles and visible behavior rather than dishonest identical screens.

## `/goal` Execution Contract

1. Treat this plan as advisory but binding on scope until contradicted by fresh evidence.
2. Before editing, re-check `git status --short`, dirty-path overlap, Television latest tags/current processor behavior, current channel/session contracts, and likely edit files.
3. Execute Phase 0 first. Stop and re-plan if its evidence contradicts the ANSI-display approach, opaque output, matching behavior, supported Television version, or user-approved external scope.
4. Preserve unrelated user changes, especially `justfiles/`.
5. Use subagents exactly within their bounded file/acceptance areas and require concise evidence of files, commands, failures, residual risks, and owned definition-of-done status.
6. Do not publish upstream changes, alter external repositories, or raise the minimum Television version without explicit authority.
7. Continue until every approved acceptance criterion and definition of done is met, including real end-to-end visual verification and independent review; do not declare partial preview work to be full visual recovery.
8. Finish with evidence: files changed, upstream Television resolution/version, checks and E2E scenarios run, snapshot/style results, before/after visuals, optional dependencies, residual risks, and cleanup.
