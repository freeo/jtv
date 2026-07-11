#![cfg(all(unix, target_os = "linux"))]
#![allow(dead_code)]

#[path = "support/artifacts.rs"]
mod artifacts;
#[path = "support/keys.rs"]
mod keys;
#[path = "support/pty.rs"]
mod pty;
#[path = "support/sandbox.rs"]
mod sandbox;
#[path = "support/scenarios.rs"]
mod scenarios;
#[path = "support/screen.rs"]
mod screen;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use keys::Key;
use scenarios::RealTvScenario;
use screen::{ScreenColor, ScreenFrame};

const COLUMNS: u16 = 120;
const ROWS: u16 = 40;
const SECRET: &str = "jtv-SNAPSHOT-secret-9173";

fn launch(name: &str) -> RealTvScenario {
    RealTvScenario::launch_with_options(
        name,
        COLUMNS,
        ROWS,
        &["--color", "always", "--icons", "unicode"],
    )
    .unwrap()
}

fn assert_canonical(frame: &ScreenFrame) {
    assert_eq!((frame.columns, frame.rows), (COLUMNS, ROWS));
    assert!(frame.alternate_screen, "TV must own the alternate screen");
}

fn snapshot(name: &str, scenario: &RealTvScenario, frame: &ScreenFrame) {
    assert_canonical(frame);
    insta::assert_snapshot!(name, frame.styled_snapshot_text(scenario.sandbox.root()));
}

fn cancel_root(mut scenario: RealTvScenario) {
    scenario.key(Key::Escape);
    assert_eq!(scenario.exit().exit_code(), 0);
    scenario.assert_clean();
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_browser_and_preview_frames() {
    let mut initial = launch("snapshot-initial-browser");
    let initial_frame = initial.wait(
        "initial recipe browser and preview",
        "Accept the configured boolean picker value.",
    );
    assert!(
        initial_frame.contains("simple"),
        "recipe results are visible"
    );
    assert!(initial_frame.contains("Accept the configured boolean picker value."));
    assert!(
        initial_frame.cursor_visible,
        "search input owns keyboard focus"
    );
    snapshot("initial_recipe_browser_120x40", &initial, &initial_frame);
    cancel_root(initial);

    let mut filtered = launch("snapshot-filtered-preview");
    filtered.wait("initial recipe browser", "jtv-recipes");
    filtered.text("capture");
    let frame = filtered.wait(
        "filtered parameterized recipe and preview",
        "Capture one argument literally.",
    );
    assert!(frame.contains("capture value"));
    assert!(frame.contains("Capture one argument literally."));
    assert!(frame.contains("value"));
    snapshot("filtered_recipe_with_preview_120x40", &filtered, &frame);
    filtered.key(Key::Ctrl('f'));
    let definition = filtered.wait(
        "faithful Definition preview",
        "# Capture one argument literally.",
    );
    assert!(definition.contains("@printf 'capture:%s\\n'"));
    snapshot("filtered_recipe_definition_120x40", &filtered, &definition);
    cancel_root(filtered);
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_workspace_source_frames() {
    let mut scenario = RealTvScenario::launch_workspace_with_options(
        "snapshot-workspace-sources",
        COLUMNS,
        ROWS,
        &["--color", "always", "--icons", "unicode"],
    )
    .unwrap();
    let root = scenario.wait("root source recipe", "simple");
    snapshot("workspace_root_source_120x40", &scenario, &root);

    let subfolders = scenario.cycle_source("Subfolders");
    assert!(subfolders.contains("supabase/"));
    assert!(subfolders.contains("tools.just"));
    snapshot("workspace_subfolders_source_120x40", &scenario, &subfolders);

    let modules = scenario.cycle_source("Modules");
    assert!(modules.contains("ops::module-run"));
    snapshot("workspace_modules_source_120x40", &scenario, &modules);

    let all = scenario.cycle_source("All");
    assert!(all.contains("supabase/"));
    assert!(all.contains("ops::module-run"));
    snapshot("workspace_all_source_120x40", &scenario, &all);
    cancel_root(scenario);
}

#[test]
#[ignore = "upstream ANSI+display capability gate; requires a patched Television binary"]
fn patched_tv_preserves_semantic_styles_in_the_real_list_and_preview() {
    assert_eq!(
        std::env::var("JTV_TEST_TV_ANSI_DISPLAY").as_deref(),
        Ok("1"),
        "set JTV_TEST_TV_ANSI_DISPLAY=1 only when JTV_TEST_REAL_TV points to the patched build"
    );
    let mut scenario = RealTvScenario::launch_with_options(
        "patched-tv-semantic-styles",
        COLUMNS,
        ROWS,
        &["--color", "always", "--icons", "unicode"],
    )
    .unwrap();
    let frame = scenario.wait(
        "styled recipe list and preview",
        "Accept the configured boolean picker value.",
    );
    let capture_style = frame
        .style_at_text("capture")
        .expect("the unselected capture recipe is visible");
    assert_eq!(
        capture_style.foreground,
        ScreenColor::Indexed(6),
        "the unselected recipe-name cell must retain jtv's cyan role"
    );
    insta::assert_snapshot!(
        "patched_tv_colored_source_120x40",
        frame.styled_snapshot_text(scenario.sandbox.root())
    );
    scenario.text("capture");
    scenario.wait(
        "fuzzy matching retains the styled recipe",
        "Capture one argument literally.",
    );
    scenario.key(Key::Enter);
    scenario.wait(
        "opaque selection reaches the parameter callback",
        "[1/1] value",
    );
    scenario.key(Key::Ctrl('c'));
    assert_eq!(scenario.exit().exit_code(), 130);
    assert!(scenario.events().is_empty());
    scenario.assert_clean();
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_narrow_plain_ascii_frame() {
    let mut scenario = RealTvScenario::launch_with_options(
        "snapshot-narrow-plain-ascii",
        80,
        24,
        &["--color", "never", "--icons", "ascii"],
    )
    .unwrap();
    let frame = scenario.wait(
        "narrow ASCII recipe browser",
        "Accept the configured boolean picker value.",
    );
    assert_eq!((frame.columns, frame.rows), (80, 24));
    assert!(frame.contains("[core]"));
    assert!(frame.contains("boolean"));
    assert!(!frame.contains("🔷"));
    insta::assert_snapshot!(
        "narrow_plain_ascii_80x24",
        frame.styled_snapshot_text(scenario.sandbox.root())
    );
    cancel_root(scenario);
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_three_marked_selections_frame() {
    let mut scenario = launch("snapshot-three-marked");
    scenario.select_recipe("queue-");
    scenario.key(Key::Tab);
    scenario.key(Key::Tab);
    scenario.key(Key::Tab);
    scenario.wait("three queue recipes marked", "● 🔷 queue-c-after");
    // Establish a deterministic focus/preview after the rapid mark updates.
    scenario.key(Key::Down);
    scenario.key(Key::Up);
    scenario.wait(
        "marked queue preview settled",
        "First recipe in the deterministic queue.",
    );
    let frame = scenario
        .session
        .wait_for_quiet(Duration::from_millis(300), Duration::from_secs(3))
        .unwrap();
    for recipe in ["queue-a", "queue-b-fail", "queue-c-after"] {
        assert!(frame.contains(recipe), "missing marked recipe {recipe}");
    }
    assert_eq!(
        frame
            .text
            .lines()
            .filter(|line| line.contains("● 🔷 queue-"))
            .count(),
        3,
        "exactly three visible results must be marked"
    );
    // TV 0.15.9 emits nondeterministic partial modifier deltas while rapidly
    // repainting marked rows. This state snapshots the stable visible grid;
    // style manifests remain canonical in the browser, preview, picker,
    // confirmation, Definition, and narrow states.
    assert_canonical(&frame);
    insta::assert_snapshot!(
        "three_marked_selections_120x40",
        frame.snapshot_text(scenario.sandbox.root())
    );
    cancel_root(scenario);
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_nested_choice_and_path_picker_frames() {
    let mut choice = launch("snapshot-nested-choice");
    choice.select_recipe("choose");
    choice.key(Key::Enter);
    let choice_frame = choice.wait("nested choice picker", "development");
    for value in ["development", "staging", "production"] {
        assert!(choice_frame.contains(value));
    }
    assert_canonical(&choice_frame);
    snapshot("nested_choice_picker_120x40", &choice, &choice_frame);
    choice.key(Key::Escape);
    assert_ne!(choice.exit().exit_code(), 0);
    choice.assert_clean();

    let mut path = launch("snapshot-nested-path");
    path.select_recipe("pick-file");
    path.key(Key::Enter);
    let path_frame = path.wait("nested path picker", "sample.txt");
    assert!(path_frame.contains("sample.txt"));
    assert!(path_frame.contains("sample-directory"));
    snapshot("nested_path_picker_120x40", &path, &path_frame);
    path.key(Key::Escape);
    assert_ne!(path.exit().exit_code(), 0);
    path.assert_clean();
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_redacted_confirmation_frame() {
    let mut scenario = launch("snapshot-redacted-confirmation");
    scenario.add_secret(SECRET);
    scenario.select_recipe("secret");
    scenario.key(Key::Enter);
    scenario.wait("secret prompt", "token");
    scenario.secret(SECRET);
    scenario.key(Key::Enter);
    let frame = scenario.wait("redacted confirmation", "[REDACTED]");
    assert!(frame.contains("Run selected recipe(s)?"));
    assert!(frame.contains("secret"));
    assert!(frame.contains("[REDACTED]"));
    assert!(!frame.contains(SECRET));
    assert!(!String::from_utf8_lossy(&scenario.session.transcript()).contains(SECRET));
    let rendered = frame.styled_snapshot_text(scenario.sandbox.root());
    assert!(!rendered.contains(SECRET));
    insta::assert_snapshot!("redacted_confirmation_120x40", rendered);
    scenario.text("n");
    scenario.key(Key::Enter);
    assert_eq!(scenario.exit().exit_code(), 0);
    assert!(scenario.events().is_empty());
    scenario.assert_clean();
}

#[test]
#[ignore = "10-run real-TV determinism gate; requires pinned tools and serialized execution"]
fn initial_browser_frame_is_deterministic_across_ten_clean_runs() {
    let mut baseline: Option<(u64, String)> = None;
    for run in 0..10 {
        let mut scenario = launch(&format!("snapshot-determinism-{run}"));
        let frame = scenario.wait(
            "initial recipe browser and preview",
            "Accept the configured boolean picker value.",
        );
        assert!(frame.contains("Accept the configured boolean picker value."));
        let normalized = frame.styled_snapshot_text(scenario.sandbox.root());
        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some((expected_hash, expected)) = &baseline {
            assert_eq!(
                hash, *expected_hash,
                "normalized frame hash changed on run {run}"
            );
            assert_eq!(
                &normalized, expected,
                "hash collision or frame drift on run {run}"
            );
        } else {
            baseline = Some((hash, normalized));
        }
        cancel_root(scenario);
    }
}
