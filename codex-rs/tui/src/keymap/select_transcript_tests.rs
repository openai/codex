use super::*;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

fn binding(spec: &str) -> KeybindingsSpec {
    KeybindingsSpec::One(KeybindingSpec(spec.to_string()))
}

fn dispatch_select(
    runtime: &RuntimeKeymap,
    prefix: crossterm::event::KeyEvent,
    contexts: KeymapContextSet,
) -> bool {
    let mut matcher = KeyChordMatcher::default();
    assert!(matches!(
        matcher.advance(prefix, &runtime.chords, contexts),
        KeyChordMatch::Pending(_)
    ));
    let KeyChordMatch::Completed(key) =
        matcher.advance(KeyCode::Char('[').into(), &runtime.chords, contexts)
    else {
        return false;
    };
    runtime.app.select_transcript.is_pressed(key)
}

#[test]
fn select_transcript_uses_the_shared_leader_in_main_activity_and_overlay_contexts() {
    let runtime = RuntimeKeymap::defaults();
    for contexts in [
        KeymapContextSet::new(KeymapContext::Global),
        KeymapContextSet::activity(),
        KeymapContextSet::transcript(),
    ] {
        assert!(dispatch_select(
            &runtime,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            contexts
        ));
    }
}

#[test]
fn select_transcript_can_be_remapped_or_unbound() {
    let mut config = TuiKeymap::default();
    config.global.leader = Some(binding("f6"));
    let runtime = RuntimeKeymap::from_config(&config).expect("remapped leader");
    assert!(dispatch_select(
        &runtime,
        KeyCode::F(6).into(),
        KeymapContextSet::new(KeymapContext::Global)
    ));

    config.global.select_transcript = Some(KeybindingsSpec::Many(Vec::new()));
    let runtime = RuntimeKeymap::from_config(&config).expect("unbound selection action");
    assert!(runtime.app.select_transcript.is_empty());
    assert!(
        runtime
            .chords
            .bindings
            .iter()
            .all(|binding| binding.action.action != "select_transcript")
    );
}

#[test]
fn default_select_suffix_yields_but_explicit_conflicts_fail() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("leader ["));
    let runtime = RuntimeKeymap::from_config(&config).expect("existing leader consumer wins");
    assert!(
        runtime
            .chords
            .bindings
            .iter()
            .all(|binding| binding.action.action != "select_transcript")
    );

    config.global.open_transcript = None;
    config.pager.scroll_up = Some(binding("ctrl-x"));
    let runtime = RuntimeKeymap::from_config(&config).expect("leader prefix consumer wins");
    assert!(
        runtime
            .chords
            .bindings
            .iter()
            .all(|binding| binding.action.action != "select_transcript")
    );

    config.pager.scroll_up = None;
    config.global.open_transcript = Some(binding("leader ["));
    config.global.select_transcript = Some(binding("leader ["));
    let error = RuntimeKeymap::from_config(&config).expect_err("explicit conflict");
    assert!(error.contains("select_transcript") && error.contains("open_transcript"));
}
