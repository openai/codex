use super::*;
use pretty_assertions::assert_eq;

#[test]
fn policy_preserves_explicit_modes_and_terminal_ownership() {
    use VscodeDetection::Other;
    use VscodeDetection::Unknown;
    use VscodeDetection::VsCode;
    for (platform_default, ssh, wsl, vscode, expected) in [
        (true, false, false, Other, [false, true, true]),
        (false, false, false, Other, [false, true, false]),
        (true, true, false, Other, [false, false, false]),
        (true, false, false, VsCode, [false, false, false]),
        (true, false, true, Unknown, [false, true, false]),
        (true, false, true, Other, [false, true, true]),
    ] {
        let env = PasteEnvironment {
            primary: false,
            platform_default,
            ssh,
            wsl,
            vscode,
        };
        assert_eq!(
            [
                RightClickPaste::Off,
                RightClickPaste::On,
                RightClickPaste::Auto
            ]
            .map(|mode| env.allows(mode)),
            expected
        );
    }
}

#[tokio::test]
async fn pending_primary_paste_survives_release_but_not_other_input() {
    let mut app = crate::app::test_support::make_test_app().await;
    for (kind, keep) in [
        (MouseEventKind::Up(MouseButton::Middle), true),
        (MouseEventKind::Moved, true),
        (MouseEventKind::Down(MouseButton::Middle), true),
        (MouseEventKind::Down(MouseButton::Left), false),
        (MouseEventKind::Up(MouseButton::Right), false),
        (MouseEventKind::ScrollDown, false),
    ] {
        app.pending_right_click_paste = Some(PendingPaste {
            thread: None,
            draft: (String::new(), 0),
            source: PasteSource::Primary,
            _request: Arc::new(()),
        });
        app.invalidate_right_click_paste(&TuiEvent::Mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }));
        assert_eq!(app.pending_right_click_paste.is_some(), keep);
    }
}
