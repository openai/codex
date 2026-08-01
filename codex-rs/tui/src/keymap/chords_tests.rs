use super::KEY_CHORD_TIMEOUT;
use super::KeyChordMatch;
use super::KeyChordMatcher;
use super::KeymapContextSet;
use super::dispatch_binding;
use crate::key_hint;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::RuntimeKeymap;
use crate::keymap::bindings::KeymapContext;
use crate::keymap::bindings::keymap_action_ids;
use crate::keymap::keymap_action_id;
use crate::keymap::user_bindings;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use tokio::time::Instant;

fn binding(spec: &str) -> KeybindingsSpec {
    KeybindingsSpec::One(KeybindingSpec(spec.to_string()))
}

fn bindings(specs: &[&str]) -> KeybindingsSpec {
    KeybindingsSpec::Many(
        specs
            .iter()
            .map(|spec| KeybindingSpec((*spec).to_string()))
            .collect(),
    )
}

#[test]
fn rejects_fixed_resume_picker_keys_inside_list_chords() {
    for (chord, reserved_action) in [
        ("ctrl-c home", "cancel"),
        ("ctrl-t home", "resume_picker.toggle_transcript"),
        ("ctrl-e home", "resume_picker.toggle_expansion"),
        ("ctrl-o home", "resume_picker.toggle_density"),
    ] {
        let mut config = TuiKeymap::default();
        config.list.jump_top = Some(binding(chord));

        let error = RuntimeKeymap::from_config(&config)
            .expect_err("fixed picker actions must remain available");

        assert!(error.contains(reserved_action), "{error}");
    }
}

#[test]
fn rejects_ctrl_c_inside_approval_chords() {
    let mut config = TuiKeymap::default();
    config.approval.approve = Some(binding("ctrl-x ctrl-c"));

    let error =
        RuntimeKeymap::from_config(&config).expect_err("Ctrl+C must continue cancelling approvals");

    assert!(error.contains("cancel"), "{error}");
}

#[test]
fn resolves_chords_for_actions_in_different_contexts() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(bindings(&["ctrl-t", "ctrl-x ctrl-t"]));
    config.composer.submit = Some(binding("ctrl-x enter"));
    config.list.jump_top = Some(binding("ctrl-x home"));

    let actual = RuntimeKeymap::from_config(&config).expect("valid chord keymap");

    assert_eq!(
        user_bindings(&actual.app.open_transcript),
        vec![key_hint::ctrl(KeyCode::Char('t'))]
    );
    for (context, action, expected) in [
        (
            "global",
            "open_transcript",
            vec!["ctrl-t".to_string(), "ctrl-x ctrl-t".to_string()],
        ),
        ("composer", "submit", vec!["ctrl-x enter".to_string()]),
        ("list", "jump_top", vec!["ctrl-x home".to_string()]),
    ] {
        let action = keymap_action_id(context, action).expect("known action");
        assert_eq!(
            actual.chords.configured_specs(action),
            Some(expected.as_slice())
        );
    }
}

#[test]
fn composer_chord_inherits_global_fallback() {
    let mut config = TuiKeymap::default();
    config.global.submit = Some(binding("ctrl-x enter"));

    let actual = RuntimeKeymap::from_config(&config).expect("valid global fallback chord");
    let action = keymap_action_id("composer", "submit").expect("known action");

    assert_eq!(
        actual.chords.configured_specs(action),
        Some(["ctrl-x enter".to_string()].as_slice())
    );
}

#[test]
fn allows_shared_prefixes_with_distinct_completions() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-x ctrl-t"));
    config.global.copy = Some(binding("ctrl-x ctrl-o"));

    RuntimeKeymap::from_config(&config).expect("prefix trie can branch on the second stroke");
}

#[test]
fn allows_plain_prefixes_in_vim_and_repeated_chords_in_all_contexts() {
    let mut config = TuiKeymap::default();
    config.vim_normal.move_line_start = Some(binding("g g"));
    config.vim_operator.motion_line_start = Some(binding("g g"));
    config.global.open_transcript = Some(binding("ctrl-t ctrl-t"));

    RuntimeKeymap::from_config(&config)
        .expect("Vim owns printable prefixes and all contexts support repeated chords");
}

#[test]
fn rejects_plain_printable_prefixes_outside_vim() {
    let mut config = TuiKeymap::default();
    config.composer.submit = Some(binding("x enter"));

    let error = RuntimeKeymap::from_config(&config).expect_err("typing must not start a chord");

    assert!(error.contains("outside Vim"), "{error}");
    assert!(error.contains("ordinary text input"), "{error}");
}

#[test]
fn rejects_prefixes_that_shadow_overlapping_single_bindings() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-k ctrl-t"));

    let error = RuntimeKeymap::from_config(&config).expect_err("ctrl-k already edits text");

    assert!(error.contains("editor.kill_line_end"), "{error}");
    assert!(error.contains("ctrl-k ctrl-t"), "{error}");
}

#[test]
fn permits_the_same_chord_in_non_overlapping_contexts() {
    let mut config = TuiKeymap::default();
    config.editor.move_line_start = Some(binding("ctrl-x home"));
    config.list.jump_top = Some(binding("ctrl-x home"));

    RuntimeKeymap::from_config(&config)
        .expect("editor and list contexts are never active together");
}

#[test]
fn rejects_duplicate_chords_in_overlapping_contexts() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-x ctrl-t"));
    config.composer.submit = Some(binding("ctrl-x ctrl-t"));

    let error =
        RuntimeKeymap::from_config(&config).expect_err("one active chord must identify one action");

    assert!(error.contains("global.open_transcript"), "{error}");
    assert!(error.contains("composer.submit"), "{error}");
}

#[test]
fn rejects_duplicate_chat_and_list_chords_used_by_request_user_input() {
    let mut config = TuiKeymap::default();
    config.chat.interrupt_turn = Some(binding("ctrl-x home"));
    config.list.jump_top = Some(binding("ctrl-x home"));

    let error =
        RuntimeKeymap::from_config(&config).expect_err("request input activates both contexts");

    assert!(error.contains("chat.interrupt_turn"), "{error}");
    assert!(error.contains("list.jump_top"), "{error}");
}

#[test]
fn normalizes_ctrl_slash_terminal_aliases() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-7 ctrl-t"));

    let error = RuntimeKeymap::from_config(&config)
        .expect_err("Ctrl-7 must remain the Ctrl-/ side-conversation alias");

    assert!(error.contains("global.toggle_side_conversation"), "{error}");
}

#[test]
fn rejects_alias_equivalent_duplicate_chords() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-x ctrl-/"));
    config.global.copy = Some(binding("ctrl-x ctrl-7"));
    config.global.toggle_side_conversation = Some(KeybindingsSpec::Many(Vec::new()));

    let error = RuntimeKeymap::from_config(&config).expect_err("aliases must identify one action");

    assert!(error.contains("global.open_transcript"), "{error}");
    assert!(error.contains("global.copy"), "{error}");
}

#[test]
fn rejects_plain_escape_inside_a_chord() {
    let mut config = TuiKeymap::default();
    config.list.cancel = Some(binding("ctrl-x esc"));

    let error = RuntimeKeymap::from_config(&config).expect_err("Esc cancels pending chords");

    assert!(error.contains("reserved for cancelling"), "{error}");
}

#[test]
fn rejects_reserved_interrupt_keys_inside_a_main_chord() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-x ctrl-c"));

    let error = RuntimeKeymap::from_config(&config).expect_err("ctrl-c must remain available");

    assert!(error.contains("fixed.interrupt_or_quit"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_unix_suspend_key_in_any_chord_stroke() {
    for chord in ["ctrl-z ctrl-t", "ctrl-x ctrl-z"] {
        let mut config = TuiKeymap::default();
        config.list.jump_top = Some(binding(chord));

        let error = RuntimeKeymap::from_config(&config)
            .expect_err("the Unix suspend key must not be consumed");

        assert!(error.contains("suspending"), "{error}");
    }
}

#[cfg(windows)]
#[test]
fn rejects_windows_altgr_prefixes_outside_vim() {
    let mut config = TuiKeymap::default();
    config.composer.submit = Some(binding("ctrl-alt-x enter"));

    let error =
        RuntimeKeymap::from_config(&config).expect_err("AltGr may be ordinary Windows text input");

    assert!(error.contains("AltGr"), "{error}");
}

#[test]
fn dispatch_tokens_are_unique_and_cover_the_action_inventory() {
    let mut tokens = HashSet::new();
    let actions = keymap_action_ids().collect::<Vec<_>>();

    for action in &actions {
        assert!(
            tokens.insert(dispatch_binding(*action).expect("action inventory must fit token band")),
            "duplicate token for {}",
            action.config_path()
        );
    }

    assert_eq!(tokens.len(), actions.len());
}

#[test]
fn physical_dispatch_tokens_do_not_clear_pending_chords() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-x ctrl-t"));
    let keymap = RuntimeKeymap::from_config(&config).expect("valid chord keymap");
    let contexts = KeymapContextSet::new(KeymapContext::Global);
    let now = Instant::now();
    let mut matcher = KeyChordMatcher::default();

    assert!(matches!(
        matcher.advance(
            key_event(key_hint::ctrl(KeyCode::Char('x'))),
            &keymap.chords,
            contexts,
            now,
        ),
        KeyChordMatch::Pending(_)
    ));

    let action = keymap_action_id("global", "open_transcript").expect("known keymap action");
    let token = dispatch_binding(action).expect("internal dispatch token");
    assert_eq!(
        matcher.advance(key_event(token), &keymap.chords, contexts, now),
        KeyChordMatch::Ignored
    );
    assert!(matcher.is_pending());
    assert!(matches!(
        matcher.advance(
            key_event(key_hint::ctrl(KeyCode::Char('t'))),
            &keymap.chords,
            contexts,
            now,
        ),
        KeyChordMatch::Completed(_)
    ));
}

#[test]
fn matching_chords_dispatches_the_active_context_action() {
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(binding("ctrl-t ctrl-t"));
    config.editor.move_line_start = Some(binding("ctrl-x home"));
    config.list.jump_top = Some(binding("ctrl-x home"));
    let runtime = RuntimeKeymap::from_config(&config).expect("non-overlapping chord contexts");
    let now = Instant::now();

    for (context, prefix, completion, target, other) in [
        (
            KeymapContext::Global,
            key_event(key_hint::ctrl(KeyCode::Char('t'))),
            key_event(key_hint::ctrl(KeyCode::Char('t'))),
            runtime.app.open_transcript.as_slice(),
            runtime.app.copy.as_slice(),
        ),
        (
            KeymapContext::Editor,
            key_event(key_hint::ctrl(KeyCode::Char('x'))),
            KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE),
            runtime.editor.move_line_start.as_slice(),
            runtime.list.jump_top.as_slice(),
        ),
        (
            KeymapContext::List,
            key_event(key_hint::ctrl(KeyCode::Char('x'))),
            KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE),
            runtime.list.jump_top.as_slice(),
            runtime.editor.move_line_start.as_slice(),
        ),
    ] {
        let mut matcher = KeyChordMatcher::default();
        assert!(matches!(
            matcher.advance(prefix, &runtime.chords, KeymapContextSet::new(context), now,),
            KeyChordMatch::Pending(_)
        ));
        let KeyChordMatch::Completed(dispatch_event) = matcher.advance(
            completion,
            &runtime.chords,
            KeymapContextSet::new(context),
            now,
        ) else {
            panic!("second stroke must complete the active context chord");
        };
        assert!(target.is_pressed(dispatch_event));
        assert!(!other.is_pressed(dispatch_event));
    }
}

#[test]
fn matcher_cancels_on_escape_context_change_and_timeout() {
    let mut config = TuiKeymap::default();
    config.list.jump_top = Some(binding("ctrl-x ctrl-t"));
    let runtime = RuntimeKeymap::from_config(&config).expect("valid list chord");
    let list = KeymapContextSet::new(KeymapContext::List);
    let pager = KeymapContextSet::new(KeymapContext::Pager);
    let prefix = key_event(key_hint::ctrl(KeyCode::Char('x')));
    let now = Instant::now();

    let mut matcher = KeyChordMatcher::default();
    assert!(matches!(
        matcher.advance(prefix, &runtime.chords, list, now),
        KeyChordMatch::Pending(_)
    ));
    assert_eq!(
        matcher.advance(
            KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE),
            &runtime.chords,
            list,
            now,
        ),
        KeyChordMatch::Cancelled
    );

    assert!(matches!(
        matcher.advance(prefix, &runtime.chords, list, now),
        KeyChordMatch::Pending(_)
    ));
    assert!(matcher.expire(pager, now));

    assert!(matches!(
        matcher.advance(prefix, &runtime.chords, list, now),
        KeyChordMatch::Pending(_)
    ));
    assert!(matcher.expire(list, now + KEY_CHORD_TIMEOUT));
}

fn key_event(binding: crate::key_hint::KeyBinding) -> KeyEvent {
    let (code, modifiers) = binding.parts();
    KeyEvent::new(code, modifiers)
}
