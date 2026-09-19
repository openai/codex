//! Picker hints follow runtime bindings while preserving custom and required-choice policy.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn popup_hints_follow_configured_bindings() {
    for (name, params) in [
        ("default", SelectionViewParams::default()),
        ("picker", SelectionViewParams::picker()),
    ] {
        let (tx, _rx) = unbounded_channel();
        let mut pane = test_pane(AppEventSender::new(tx));
        let mut keymap = RuntimeKeymap::defaults();
        keymap.list.accept = vec![crate::key_hint::plain(KeyCode::F(/*n*/ 3))];
        keymap.list.cancel = vec![crate::key_hint::plain(KeyCode::F(/*n*/ 2))];
        pane.set_keymap_bindings(&keymap);
        pane.show_selection_view(SelectionViewParams {
            title: Some("Choose an action".into()),
            items: vec![SelectionItem {
                name: "Continue".into(),
                ..Default::default()
            }],
            ..params
        });
        let width = 60;
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            pane.desired_height(width),
        );
        let rendered = render_snapshot(&pane, area);
        assert_snapshot!(format!("popup_hint_{name}"), rendered);
        assert!(rendered.contains("f3 select · f2 back"), "{rendered}");
        pane.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 2)));
        assert!(pane.view_stack.is_empty());
    }
}

#[test]
fn custom_and_noncancelable_popup_hints_keep_their_policy() {
    let (tx, _rx) = unbounded_channel();
    let pane = test_pane(AppEventSender::new(tx));
    for (allow_cancel, supplied, expected) in [
        (
            true,
            Some(Line::from("custom controls")),
            Some(Line::from("custom controls")),
        ),
        (true, Some(Line::default()), Some(Line::default())),
        (false, None, None),
        (false, Some(popup_consts::standard_popup_hint_line()), None),
        (
            false,
            Some(Line::from("custom controls")),
            Some(Line::from("custom controls")),
        ),
    ] {
        let mut params = SelectionViewParams {
            allow_cancel,
            footer_hint: supplied,
            ..Default::default()
        };
        pane.apply_standard_popup_hint(&mut params);
        assert_eq!(params.footer_hint, expected);
    }
}
