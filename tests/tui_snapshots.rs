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
use screen::ScreenFrame;

const COLUMNS: u16 = 120;
const ROWS: u16 = 40;
const SECRET: &str = "jtv-SNAPSHOT-secret-9173";

fn launch(name: &str) -> RealTvScenario {
    RealTvScenario::launch_with_viewport(name, COLUMNS, ROWS).unwrap()
}

fn assert_canonical(frame: &ScreenFrame) {
    assert_eq!((frame.columns, frame.rows), (COLUMNS, ROWS));
    assert!(frame.alternate_screen, "TV must own the alternate screen");
}

fn snapshot(name: &str, scenario: &RealTvScenario, frame: &ScreenFrame) {
    assert_canonical(frame);
    insta::assert_snapshot!(name, frame.snapshot_text(scenario.sandbox.root()));
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
    cancel_root(filtered);
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9 and just 1.53.0; run serialized"]
fn reviewed_three_marked_selections_frame() {
    let mut scenario = launch("snapshot-three-marked");
    scenario.select_recipe("queue-");
    scenario.key(Key::Tab);
    scenario.key(Key::Tab);
    scenario.key(Key::Tab);
    scenario.wait("three queue recipes marked", "● queue-c-after");
    // TV 0.15.9 resolves preview callbacks asynchronously after rapid marks.
    // Move away and back to establish a single known focus/preview state, then
    // require output quiescence; this is synchronization, not a retry.
    scenario.key(Key::Down);
    scenario.key(Key::Up);
    scenario.wait(
        "marked queue preview settled",
        "First recipe in the deterministic queue.",
    );
    let frame = scenario
        .session
        .wait_for_quiet(Duration::from_millis(150), Duration::from_secs(2))
        .unwrap();
    for recipe in ["queue-a", "queue-b-fail", "queue-c-after"] {
        assert!(frame.contains(recipe), "missing marked recipe {recipe}");
    }
    assert_eq!(
        frame.text.lines().filter(|line| line.contains('●')).count(),
        3,
        "exactly three visible results must be marked"
    );
    snapshot("three_marked_selections_120x40", &scenario, &frame);
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
    let rendered = frame.snapshot_text(scenario.sandbox.root());
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
        let normalized = frame.snapshot_text(scenario.sandbox.root());
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
