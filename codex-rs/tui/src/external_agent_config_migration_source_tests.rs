use super::*;
use crate::test_backend::VT100Backend;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::Terminal;

fn new_screen() -> ExternalAgentConfigSourceScreen {
    ExternalAgentConfigSourceScreen::new(
        FrameRequester::test_dummy(),
        &ExternalAgentConfigMigrationSource::ALL,
    )
}

#[test]
fn external_agent_config_source_prompt_snapshot() {
    let screen = new_screen();
    let mut terminal =
        Terminal::new(VT100Backend::new(/*width*/ 80, /*height*/ 12)).expect("terminal");
    terminal
        .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
        .expect("render source prompt");
    insta::assert_snapshot!("external_agent_config_source_prompt", terminal.backend());
}

#[test]
fn external_agent_config_source_prompt_selects_highlighted_source() {
    let mut screen = new_screen();
    screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        screen.selection(),
        Some(ExternalAgentConfigMigrationSource::Cur)
    );
}

#[test]
fn external_agent_config_source_prompt_can_cancel() {
    let mut screen = new_screen();
    screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(screen.selection(), None);
}
