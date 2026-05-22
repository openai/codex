use super::helpers::make_chatwidget_manual;
use super::helpers::normalized_backend_snapshot;
use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[tokio::test]
async fn next_prompt_suggestion_renders_as_empty_composer_placeholder_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_next_prompt_suggestion(Some("run the tests".to_string()));

    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw next prompt suggestion placeholder");
    assert_chatwidget_snapshot!(
        "next_prompt_suggestion_renders_as_empty_composer_placeholder",
        normalized_backend_snapshot(terminal.backend())
    );
}
