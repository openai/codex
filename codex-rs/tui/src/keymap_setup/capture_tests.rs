use super::super::KeymapEditOutcome;
use super::super::build_keymap_replace_binding_menu_params;
use super::super::keymap_with_bindings;
use super::super::keymap_with_edit;
use super::KeymapCaptureView;
use crate::app_event::AppEvent;
use crate::app_event::KeymapCaptureMode;
use crate::app_event::KeymapEditIntent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;

fn capture_view() -> (KeymapCaptureView, UnboundedReceiver<AppEvent>) {
    let (tx, rx) = unbounded_channel();
    let view = KeymapCaptureView::new(
        "list".to_string(),
        "jump_top".to_string(),
        KeymapEditIntent::AddAlternate,
        KeymapCaptureMode::Chord,
        "Jump Top".to_string(),
        "home".to_string(),
        AppEventSender::new(tx),
    );
    (view, rx)
}

fn ctrl_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn capture_lines_at_width(view: &KeymapCaptureView, width: u16) -> String {
    view.lines(width)
        .into_iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn chord_capture_instruction_snapshots() {
    let (mut view, _rx) = capture_view();

    insta::assert_snapshot!(capture_lines_at_width(&view, /*width*/ 80), @r"
    Remap Shortcut
    Action: Jump Top  list.jump_top
    Current: home
    Press the first key, then the second. Esc cancels.
    ");

    view.handle_key_event(ctrl_key(KeyCode::Char('x')));

    insta::assert_snapshot!(capture_lines_at_width(&view, /*width*/ 80), @r"
    Remap Shortcut
    Action: Jump Top  list.jump_top
    Current: home
    First key: ctrl-x. Press the second key. Esc cancels.
    ");
}

#[test]
fn chord_capture_instructions_wrap_to_narrow_panes() {
    let (mut view, _rx) = capture_view();

    insta::assert_snapshot!(capture_lines_at_width(&view, /*width*/ 24), @r"
    Remap Shortcut
    Action: Jump Top  list.jump_top
    Current: home
    Press the first key,
    then the second. Esc
    cancels.
    ");
    assert_eq!(view.desired_height(/*width*/ 24), 6);

    view.handle_key_event(KeyEvent::new(
        KeyCode::F(/*n*/ 24),
        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT,
    ));

    insta::assert_snapshot!(capture_lines_at_width(&view, /*width*/ 24), @r"
    Remap Shortcut
    Action: Jump Top  list.jump_top
    Current: home
    First key: ctrl-alt-
    shift-f24. Press the
    second key. Esc cancels.
    ");
    assert_eq!(view.desired_height(/*width*/ 24), 6);
}

#[test]
fn captures_repeated_strokes_but_ignores_auto_repeat() {
    let (mut view, mut rx) = capture_view();

    view.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 25)));
    assert!(view.error_message.is_some());
    view.handle_key_event(ctrl_key(KeyCode::Char('x')));
    assert_eq!(view.error_message, None);
    view.handle_key_event(KeyEvent {
        kind: KeyEventKind::Repeat,
        ..ctrl_key(KeyCode::Char('x'))
    });

    assert!(!view.is_complete());
    assert_eq!(view.first_stroke.as_deref(), Some("ctrl-x"));
    assert!(rx.try_recv().is_err());

    view.handle_key_event(ctrl_key(KeyCode::Char('x')));

    let AppEvent::KeymapCaptured {
        context,
        action,
        key,
        intent,
    } = rx.try_recv().expect("completed chord capture event")
    else {
        panic!("expected a captured key chord");
    };
    assert_eq!(context, "list");
    assert_eq!(action, "jump_top");
    assert_eq!(key, "ctrl-x ctrl-x");
    assert_eq!(intent, KeymapEditIntent::AddAlternate);
    assert!(view.is_complete());
}

#[test]
fn modified_escape_is_captured_as_a_chord_stroke() {
    let (mut canceled_view, mut canceled_events) = capture_view();
    canceled_view.handle_key_event(ctrl_key(KeyCode::Char('x')));
    canceled_view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(canceled_view.is_complete());
    assert!(canceled_events.try_recv().is_err());

    let ctrl_x = ctrl_key(KeyCode::Char('x'));
    let ctrl_home = ctrl_key(KeyCode::Home);
    let ctrl_esc = ctrl_key(KeyCode::Esc);
    let alt_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::ALT);
    for (first, second, expected) in [
        (ctrl_x, ctrl_esc, "ctrl-x ctrl-esc"),
        (ctrl_x, alt_esc, "ctrl-x alt-esc"),
        (ctrl_esc, ctrl_home, "ctrl-esc ctrl-home"),
        (alt_esc, ctrl_home, "alt-esc ctrl-home"),
    ] {
        let (mut view, mut rx) = capture_view();
        view.handle_key_event(first);
        assert!(
            !view.is_complete(),
            "the prefix of `{expected}` must be captured"
        );
        assert!(rx.try_recv().is_err());

        view.handle_key_event(second);

        let AppEvent::KeymapCaptured { key, .. } =
            rx.try_recv().expect("completed modified-Escape chord")
        else {
            panic!("expected a captured key chord");
        };
        assert_eq!(key, expected);
        assert!(view.is_complete());
    }
}

#[test]
fn replacing_one_list_binding_with_chord_preserves_other_alternatives() {
    let config = keymap_with_bindings(
        &TuiKeymap::default(),
        "list",
        "jump_top",
        &["home".to_string(), "ctrl-x home".to_string()],
    )
    .expect("list chord bindings");
    let runtime = RuntimeKeymap::from_config(&config).expect("valid list key chords");
    let params = build_keymap_replace_binding_menu_params(
        "list".to_string(),
        "jump_top".to_string(),
        &runtime,
    );

    let chord_item = params
        .items
        .iter()
        .find(|item| item.name == "home (key chord)")
        .expect("replace-one chord capture option");
    let (tx, mut rx) = unbounded_channel();
    let sender = AppEventSender::new(tx);

    chord_item.actions[0](&sender);

    let AppEvent::OpenKeymapCapture {
        context,
        action,
        intent,
        capture_mode,
    } = rx.try_recv().expect("open replace-one chord capture")
    else {
        panic!("expected a chord capture event");
    };
    assert_eq!(capture_mode, KeymapCaptureMode::Chord);
    assert_eq!(
        intent,
        KeymapEditIntent::ReplaceOne {
            old_key: "home".to_string()
        }
    );

    let outcome = keymap_with_edit(
        &config,
        &runtime,
        &context,
        &action,
        "ctrl-x ctrl-h",
        &intent,
    )
    .expect("replace one binding with a key chord");

    let expected_bindings = vec!["ctrl-x ctrl-h".to_string(), "ctrl-x home".to_string()];
    let expected_keymap = keymap_with_bindings(&config, "list", "jump_top", &expected_bindings)
        .expect("updated list key chords");
    RuntimeKeymap::from_config(&expected_keymap).expect("updated list chord keymap is valid");
    assert_eq!(
        outcome,
        KeymapEditOutcome::Updated {
            keymap_config: Box::new(expected_keymap),
            bindings: expected_bindings,
            message: "Replaced `home` with `ctrl-x ctrl-h` for `list.jump_top`.".to_string(),
        }
    );
}
