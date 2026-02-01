//! Rendering functions for the TUI.
//!
//! This module provides the main render function that draws the UI
//! based on the current application state.

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::state::AppState;
use crate::state::FocusTarget;
use crate::state::Overlay;
use crate::widgets::ChatWidget;
use crate::widgets::InputWidget;
use crate::widgets::StatusBar;
use crate::widgets::ToolPanel;

/// Render the UI to the terminal frame.
///
/// This function is the main entry point for rendering. It layouts
/// the screen and draws all widgets based on the current state.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Main layout: status bar at bottom, rest is chat + input
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Chat + Input + Tools
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Upper area layout
    render_main_area(frame, main_chunks[0], state);

    // Status bar
    render_status_bar(frame, main_chunks[1], state);

    // Render overlay if present
    if let Some(ref overlay) = state.ui.overlay {
        render_overlay(frame, area, overlay);
    }
}

/// Render the main content area (chat, tools, input).
fn render_main_area(frame: &mut Frame, area: Rect, state: &AppState) {
    // Check if we have running tools to show
    let has_tools = !state.session.tool_executions.is_empty()
        && state
            .session
            .tool_executions
            .iter()
            .any(|t| t.status == crate::state::ToolStatus::Running);

    if has_tools {
        // Three-column layout: chat | tools
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70), // Chat + Input
                Constraint::Percentage(30), // Tools
            ])
            .split(area);

        render_chat_and_input(frame, horizontal[0], state);
        render_tools(frame, horizontal[1], state);
    } else {
        // Just chat + input
        render_chat_and_input(frame, area, state);
    }
}

/// Render the chat area and input box.
fn render_chat_and_input(frame: &mut Frame, area: Rect, state: &AppState) {
    // Calculate input height based on content
    let input_lines = state.ui.input.text().lines().count().max(1);
    let input_height = (input_lines as u16 + 2).min(10); // +2 for borders, max 10

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),               // Chat
            Constraint::Length(input_height), // Input
        ])
        .split(area);

    // Chat widget
    let streaming_content = state.ui.streaming.as_ref().map(|s| s.content.as_str());

    let chat = ChatWidget::new(&state.session.messages)
        .scroll(state.ui.scroll_offset)
        .streaming(streaming_content);
    frame.render_widget(chat, chunks[0]);

    // Input widget
    let input = InputWidget::new(&state.ui.input)
        .focused(state.ui.focus == FocusTarget::Input)
        .placeholder("Type a message... (Enter to send, Shift+Enter for newline)");
    frame.render_widget(input, chunks[1]);
}

/// Render the tools panel.
fn render_tools(frame: &mut Frame, area: Rect, state: &AppState) {
    let panel = ToolPanel::new(&state.session.tool_executions).max_display(8);
    frame.render_widget(panel, area);
}

/// Render the status bar.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status_bar = StatusBar::new(
        &state.session.current_model,
        &state.session.thinking_level,
        state.session.plan_mode,
        &state.session.token_usage,
    );
    frame.render_widget(status_bar, area);
}

/// Render an overlay on top of the main content.
fn render_overlay(frame: &mut Frame, area: Rect, overlay: &Overlay) {
    // Calculate centered area
    let overlay_width = (area.width * 60 / 100).min(80).max(40);
    let overlay_height = match overlay {
        Overlay::Permission(_) => 12,
        Overlay::ModelPicker(picker) => (picker.filtered_models().len() as u16 + 4).min(20),
        Overlay::Help => 20,
        Overlay::Error(_) => 8,
    };

    let x = (area.width.saturating_sub(overlay_width)) / 2;
    let y = (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    // Clear the area behind the overlay
    frame.render_widget(Clear, overlay_area);

    match overlay {
        Overlay::Permission(perm) => render_permission_overlay(frame, overlay_area, perm),
        Overlay::ModelPicker(picker) => render_model_picker_overlay(frame, overlay_area, picker),
        Overlay::Help => render_help_overlay(frame, overlay_area),
        Overlay::Error(message) => render_error_overlay(frame, overlay_area, message),
    }
}

/// Render the permission approval overlay.
fn render_permission_overlay(
    frame: &mut Frame,
    area: Rect,
    perm: &crate::state::PermissionOverlay,
) {
    let block = Block::default()
        .title(" Permission Required ".bold().yellow())
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().yellow());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build content
    let mut lines: Vec<Line> = vec![];

    // Tool name
    lines.push(Line::from(vec![
        "Tool: ".bold().into(),
        Span::raw(&perm.request.tool_name).cyan(),
    ]));
    lines.push(Line::from(""));

    // Description
    lines.push(Line::from(Span::raw(&perm.request.description)));
    lines.push(Line::from(""));

    // Options
    let options = ["[Y] Approve", "[N] Deny", "[A] Approve All"];
    for (i, opt) in options.iter().enumerate() {
        let is_selected = perm.selected == i as i32;
        let line = if is_selected {
            Line::from(Span::raw(format!("▸ {opt}")).bold().cyan())
        } else {
            Line::from(Span::raw(format!("  {opt}")).dim())
        };
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render the model picker overlay.
fn render_model_picker_overlay(
    frame: &mut Frame,
    area: Rect,
    picker: &crate::state::ModelPickerOverlay,
) {
    let title = if picker.filter.is_empty() {
        " Select Model ".to_string()
    } else {
        format!(" Select Model [{}] ", picker.filter)
    };

    let block = Block::default()
        .title(title.bold())
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().cyan());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build model list
    let models = picker.filtered_models();
    let mut lines: Vec<Line> = vec![];

    for (i, model) in models.iter().enumerate() {
        let is_selected = picker.selected == i as i32;
        let line = if is_selected {
            Line::from(Span::raw(format!("▸ {model}")).bold().cyan())
        } else {
            Line::from(Span::raw(format!("  {model}")))
        };
        lines.push(line);
    }

    if models.is_empty() {
        lines.push(Line::from(
            Span::raw("No models match filter").dim().italic(),
        ));
    }

    // Add hints at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(
        Span::raw("↑/↓: Navigate  Enter: Select  Esc: Cancel  Type to filter").dim(),
    ));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render the help overlay.
fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Keyboard Shortcuts ".bold())
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = vec![
        Line::from(vec!["Tab".bold().into(), "      Toggle plan mode".into()]),
        Line::from(vec![
            "Ctrl+T".bold().into(),
            "   Cycle thinking level".into(),
        ]),
        Line::from(vec!["Ctrl+M".bold().into(), "   Switch model".into()]),
        Line::from(vec!["Ctrl+C".bold().into(), "   Interrupt".into()]),
        Line::from(vec!["Ctrl+L".bold().into(), "   Clear screen".into()]),
        Line::from(vec!["Ctrl+Q".bold().into(), "   Quit".into()]),
        Line::from(""),
        Line::from(vec!["Enter".bold().into(), "    Submit message".into()]),
        Line::from(vec!["Shift+Enter".bold().into(), " New line".into()]),
        Line::from(vec![
            "Esc".bold().into(),
            "      Cancel/Close overlay".into(),
        ]),
        Line::from(""),
        Line::from(vec!["↑/↓".bold().into(), "      History navigation".into()]),
        Line::from(vec!["Alt+↑/↓".bold().into(), "  Scroll chat".into()]),
        Line::from(vec!["PgUp/PgDn".bold().into(), " Page scroll".into()]),
        Line::from(""),
        Line::from(Span::raw("Press Esc to close").dim()),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render an error overlay.
fn render_error_overlay(frame: &mut Frame, area: Rect, message: &str) {
    let block = Block::default()
        .title(" Error ".bold().red())
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().red());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = vec![
        Line::from(Span::raw(message)),
        Line::from(""),
        Line::from(Span::raw("Press Esc or Enter to dismiss").dim()),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn create_test_terminal() -> Terminal<TestBackend> {
        let backend = TestBackend::new(80, 24);
        Terminal::new(backend).expect("Failed to create test terminal")
    }

    #[test]
    fn test_render_empty_state() {
        let mut terminal = create_test_terminal();
        let state = AppState::new();

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");

        // Just verify it doesn't panic
    }

    #[test]
    fn test_render_with_messages() {
        let mut terminal = create_test_terminal();
        let mut state = AppState::new();

        state
            .session
            .add_message(crate::state::ChatMessage::user("1", "Hello"));
        state
            .session
            .add_message(crate::state::ChatMessage::assistant("2", "Hi there!"));

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");
    }

    #[test]
    fn test_render_with_streaming() {
        let mut terminal = create_test_terminal();
        let mut state = AppState::new();

        state.ui.start_streaming("turn-1".to_string());
        state.ui.append_streaming("Streaming content...");

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");
    }

    #[test]
    fn test_render_with_tools() {
        let mut terminal = create_test_terminal();
        let mut state = AppState::new();

        state
            .session
            .start_tool("call-1".to_string(), "bash".to_string());

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");
    }

    #[test]
    fn test_render_with_permission_overlay() {
        let mut terminal = create_test_terminal();
        let mut state = AppState::new();

        state
            .ui
            .set_overlay(Overlay::Permission(crate::state::PermissionOverlay::new(
                cocode_protocol::ApprovalRequest {
                    request_id: "req-1".to_string(),
                    tool_name: "bash".to_string(),
                    description: "Run command: ls -la".to_string(),
                    risks: vec![],
                    allow_remember: true,
                },
            )));

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");
    }

    #[test]
    fn test_render_with_model_picker() {
        let mut terminal = create_test_terminal();
        let mut state = AppState::new();

        state
            .ui
            .set_overlay(Overlay::ModelPicker(crate::state::ModelPickerOverlay::new(
                vec!["claude-sonnet-4".to_string(), "claude-opus-4".to_string()],
            )));

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");
    }

    #[test]
    fn test_render_with_error_overlay() {
        let mut terminal = create_test_terminal();
        let mut state = AppState::new();

        state
            .ui
            .set_overlay(Overlay::Error("Something went wrong".to_string()));

        terminal
            .draw(|frame| {
                render(frame, &state);
            })
            .expect("Failed to render");
    }
}
