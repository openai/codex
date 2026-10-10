use super::*;
use crate::transcript_mode::TranscriptMode;
use pretty_assertions::assert_eq;

#[test]
fn policy_preserves_explicit_modes_and_terminal_ownership() {
    use VscodeDetection::Other;
    use VscodeDetection::Unknown;
    use VscodeDetection::VsCode;
    const SSH_UNAVAILABLE: &str = "Unavailable over SSH. Use your terminal's paste command.";
    const VSCODE_UNAVAILABLE: &str = "VS Code handles pasting. Use its terminal paste command.";
    const WSL_AUTO_UNAVAILABLE: &str =
        "Auto cannot identify this WSL terminal. Use On or your terminal's paste command.";
    const INLINE_UNAVAILABLE: &str =
        "Unavailable in inline mode. Use your terminal's paste command.";
    let on = if cfg!(target_os = "android") {
        Err("Unavailable on Android")
    } else {
        Ok(true)
    };
    for (platform_default, ssh, wsl, vscode, on_status, auto_status) in [
        (true, false, false, Other, on, Ok(true)),
        (false, false, false, Other, on, Ok(false)),
        (
            true,
            true,
            false,
            Other,
            Err(SSH_UNAVAILABLE),
            Err(SSH_UNAVAILABLE),
        ),
        (
            true,
            false,
            false,
            VsCode,
            Err(VSCODE_UNAVAILABLE),
            Err(VSCODE_UNAVAILABLE),
        ),
        (true, false, true, Unknown, on, Err(WSL_AUTO_UNAVAILABLE)),
        (true, false, true, Other, on, Ok(true)),
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
            .map(|mode| env.status(mode, TranscriptMode::Owned)),
            [Ok(false), on_status, auto_status]
        );
    }

    let env = PasteEnvironment {
        primary: false,
        platform_default: true,
        ssh: false,
        wsl: false,
        vscode: Other,
    };
    assert_eq!(
        [RightClickPaste::Off, RightClickPaste::On]
            .map(|mode| env.status(mode, TranscriptMode::Terminal)),
        [Ok(false), Err(INLINE_UNAVAILABLE)]
    );
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
