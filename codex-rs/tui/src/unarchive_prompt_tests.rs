use super::*;
use crate::app_server_session::ResumeModelSettings;
use crate::test_backend::VT100Backend;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::widgets::FrameExt as _;

fn prompt(action: SessionStartAction) -> UnarchivePrompt {
    UnarchivePrompt::new(
        ThreadId::from_string("019e72f4-e09a-70f2-b2c2-a153a57b8cc0").unwrap(),
        action,
    )
}

fn render(screen: &UnarchivePrompt) -> String {
    let mut terminal = Terminal::new(VT100Backend::new(/*width*/ 80, /*height*/ 10)).unwrap();
    terminal
        .draw(|frame| frame.render_widget_ref(screen, frame.area()))
        .unwrap();
    terminal
        .backend()
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[test]
fn resume_prompt_snapshot() {
    let screen = prompt(SessionStartAction::Resume(
        ResumeModelSettings::RestoreFromThread,
    ));
    insta::assert_snapshot!(render(&screen), @"
    This conversation is archived
    019e72f4-e09a-70f2-b2c2-a153a57b8cc0

    › 1. Unarchive and resume
      2. Cancel

    Press enter to continue or esc to cancel
    ");
}

#[test]
fn fork_prompt_cancel_snapshot() {
    let mut screen = prompt(SessionStartAction::Fork);
    assert_eq!(
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        None
    );
    insta::assert_snapshot!(render(&screen), @"
    This conversation is archived
    019e72f4-e09a-70f2-b2c2-a153a57b8cc0

      1. Unarchive and fork
    › 2. Cancel

    Press enter to continue or esc to cancel
    ");
    assert_eq!(
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(UnarchiveChoice::Cancel)
    );
}

#[test]
fn confirmation_requires_an_accept_key() {
    let mut screen = prompt(SessionStartAction::Fork);
    for key in [
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    ] {
        assert_eq!(screen.handle_key(key), Some(UnarchiveChoice::Cancel));
    }
    assert_eq!(
        screen.handle_key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Release
        )),
        None
    );
    assert_eq!(
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(UnarchiveChoice::Unarchive)
    );
}

fn render_inline(width: u16) -> String {
    let screen = prompt(SessionStartAction::Fork);
    let height = screen.content().desired_height(width);
    let mut terminal = Terminal::new(VT100Backend::new(width, height + 2)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new("$ codex fork saved-session"),
                Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1),
            );
            frame.render_widget_ref(&screen, Rect::new(/*x*/ 0, /*y*/ 2, width, height));
        })
        .unwrap();
    terminal
        .backend()
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

#[test]
fn inline_prompt_preserves_shell_context_snapshot() {
    insta::assert_snapshot!("inline_prompt", render_inline(/*width*/ 80));
}

#[test]
fn narrow_inline_prompt_wraps_snapshot() {
    insta::assert_snapshot!("narrow_inline_prompt", render_inline(/*width*/ 28));
}

#[tokio::test]
async fn inline_prompt_only_claims_content_height() -> Result<()> {
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_alt_screen_enabled(/*enabled*/ false);
    let original = Rect::new(
        /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 0,
    );
    tui.terminal.set_viewport_area(original);
    let screen = prompt(SessionStartAction::Fork);
    {
        let mut guard = PromptScreenGuard::enter(&mut tui)?;
        guard.draw(&screen)?;
        assert!(!guard.tui.is_alt_screen_active());
        assert_eq!(
            guard.tui.terminal.viewport_area,
            Rect {
                height: screen.content().desired_height(/*width*/ 80),
                ..original
            }
        );
    }
    assert_eq!(tui.terminal.viewport_area, original);
    Ok(())
}

#[tokio::test]
async fn alternate_prompt_restores_original_viewport() -> Result<()> {
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let original = Rect::new(
        /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 0,
    );
    tui.terminal.set_viewport_area(original);
    {
        let mut guard = PromptScreenGuard::enter(&mut tui)?;
        guard.draw(&prompt(SessionStartAction::Fork))?;
        assert!(guard.tui.is_alt_screen_active());
        assert_eq!(
            guard.tui.terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 24
            )
        );
    }
    assert!(!tui.is_alt_screen_active());
    assert_eq!(tui.terminal.viewport_area, original);
    Ok(())
}
