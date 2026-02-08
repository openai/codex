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

use crate::i18n::t;
use crate::state::AppState;
use crate::state::FocusTarget;
use crate::state::Overlay;
use crate::widgets::ChatWidget;
use crate::widgets::FileSuggestionPopup;
use crate::widgets::InputWidget;
use crate::widgets::QueuedListWidget;
use crate::widgets::SkillSuggestionPopup;
use crate::widgets::StatusBar;
use crate::widgets::SubagentPanel;
use crate::widgets::ToastWidget;
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

    // Render toast notifications (always on top)
    if state.ui.has_toasts() {
        render_toasts(frame, area, state);
    }
}

/// Render toast notifications in the top-right corner.
fn render_toasts(frame: &mut Frame, area: Rect, state: &AppState) {
    let toast_widget = ToastWidget::new(&state.ui.toasts);
    let toast_area = toast_widget.calculate_area(area);
    if toast_area.width > 0 && toast_area.height > 0 {
        frame.render_widget(toast_widget, toast_area);
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

    // Check if we have subagents to show
    let has_subagents = !state.session.subagents.is_empty();

    if has_tools || has_subagents {
        // Three-column layout: chat | tools/subagents
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70), // Chat + Input
                Constraint::Percentage(30), // Tools/Subagents
            ])
            .split(area);

        render_chat_and_input(frame, horizontal[0], state);
        render_side_panel(frame, horizontal[1], state, has_tools, has_subagents);
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

    // Calculate queued list height (if any queued commands)
    let queued_list = QueuedListWidget::new(&state.session.queued_commands);
    let queued_height = queued_list.required_height();

    let chunks = if queued_height > 0 {
        // Layout: Chat | Queued List | Input
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),                // Chat
                Constraint::Length(queued_height), // Queued list
                Constraint::Length(input_height),  // Input
            ])
            .split(area)
    } else {
        // Layout: Chat | Input (no queued commands)
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),               // Chat
                Constraint::Length(input_height), // Input
            ])
            .split(area)
    };

    // Chat widget
    let streaming_content = state.ui.streaming.as_ref().map(|s| s.content.as_str());
    let streaming_thinking = state.ui.streaming.as_ref().map(|s| s.thinking.as_str());

    let chat = ChatWidget::new(&state.session.messages)
        .scroll(state.ui.scroll_offset)
        .streaming(streaming_content)
        .streaming_thinking(streaming_thinking)
        .show_thinking(state.ui.show_thinking)
        .is_thinking(state.ui.is_thinking())
        .animation_frame(state.ui.animation_frame())
        .thinking_duration(state.ui.thinking_duration());
    frame.render_widget(chat, chunks[0]);

    // Queued list widget (if any queued commands)
    // Get the input chunk index based on whether queued list is shown
    let input_chunk_index = if queued_height > 0 {
        // Render queued list
        let queued_list = QueuedListWidget::new(&state.session.queued_commands);
        frame.render_widget(queued_list, chunks[1]);
        2 // Input is at index 2
    } else {
        1 // Input is at index 1
    };

    // Input widget
    let placeholder = t!("input.placeholder").to_string();
    let input = InputWidget::new(&state.ui.input)
        .focused(state.ui.focus == FocusTarget::Input)
        .placeholder(&placeholder);
    frame.render_widget(input, chunks[input_chunk_index]);

    // File suggestion popup (if active)
    if let Some(ref suggestions) = state.ui.file_suggestions {
        let popup = FileSuggestionPopup::new(suggestions);
        let popup_area = popup.calculate_area(chunks[input_chunk_index], area.height);
        frame.render_widget(popup, popup_area);
    }

    // Skill suggestion popup (if active)
    if let Some(ref suggestions) = state.ui.skill_suggestions {
        let popup = SkillSuggestionPopup::new(suggestions);
        let popup_area = popup.calculate_area(chunks[input_chunk_index], area.height);
        frame.render_widget(popup, popup_area);
    }
}

/// Render the side panel (tools and/or subagents).
fn render_side_panel(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    has_tools: bool,
    has_subagents: bool,
) {
    if has_tools && has_subagents {
        // Split vertically between tools and subagents
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Tools
                Constraint::Percentage(50), // Subagents
            ])
            .split(area);

        render_tools(frame, chunks[0], state);
        render_subagents(frame, chunks[1], state);
    } else if has_tools {
        render_tools(frame, area, state);
    } else if has_subagents {
        render_subagents(frame, area, state);
    }
}

/// Render the tools panel.
fn render_tools(frame: &mut Frame, area: Rect, state: &AppState) {
    let panel = ToolPanel::new(&state.session.tool_executions).max_display(8);
    frame.render_widget(panel, area);
}

/// Render the subagents panel.
fn render_subagents(frame: &mut Frame, area: Rect, state: &AppState) {
    let panel = SubagentPanel::new(&state.session.subagents).max_display(5);
    frame.render_widget(panel, area);
}

/// Render the status bar.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    // Check if we're currently thinking (more precise than checking streaming state)
    let is_thinking = state.ui.is_thinking();

    let status_bar = StatusBar::new(
        &state.session.current_model,
        &state.session.thinking_level,
        state.session.plan_mode,
        &state.session.token_usage,
    )
    .is_thinking(is_thinking)
    .show_thinking_enabled(state.ui.show_thinking)
    .thinking_duration(state.ui.thinking_duration())
    .thinking_budget(
        state.session.thinking_tokens_used,
        state.session.thinking_budget_remaining(),
    )
    .plan_phase(state.session.plan_phase)
    .mcp_server_count(state.session.connected_mcp_count())
    .queue_counts(state.session.queued_count(), 0);
    frame.render_widget(status_bar, area);
}

/// Render an overlay on top of the main content.
fn render_overlay(frame: &mut Frame, area: Rect, overlay: &Overlay) {
    // Calculate centered area
    let overlay_width = (area.width * 60 / 100).min(80).max(40);
    let overlay_height = match overlay {
        Overlay::Permission(_) => 12,
        Overlay::ModelPicker(picker) => (picker.filtered_models().len() as u16 + 4).min(20),
        Overlay::CommandPalette(palette) => (palette.filtered_commands().len() as u16 + 4).min(20),
        Overlay::SessionBrowser(browser) => (browser.filtered_sessions().len() as u16 + 4).min(20),
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
        Overlay::CommandPalette(palette) => {
            render_command_palette_overlay(frame, overlay_area, palette)
        }
        Overlay::SessionBrowser(browser) => {
            render_session_browser_overlay(frame, overlay_area, browser)
        }
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
        .title(
            format!(" {} ", t!("dialog.permission_required"))
                .bold()
                .yellow(),
        )
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().yellow());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build content
    let mut lines: Vec<Line> = vec![];

    // Tool name
    lines.push(Line::from(vec![
        Span::raw(format!("{} ", t!("dialog.tool"))).bold(),
        Span::raw(&perm.request.tool_name).cyan(),
    ]));
    lines.push(Line::from(""));

    // Description
    lines.push(Line::from(Span::raw(&perm.request.description)));
    lines.push(Line::from(""));

    // Options
    let options = [
        t!("dialog.approve").to_string(),
        t!("dialog.deny").to_string(),
        t!("dialog.approve_all").to_string(),
    ];
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
        format!(" {} ", t!("dialog.select_model"))
    } else {
        format!(
            " {} ",
            t!("dialog.select_model_filter", filter = &picker.filter)
        )
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
            Span::raw(t!("dialog.no_models_match").to_string())
                .dim()
                .italic(),
        ));
    }

    // Add hints at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(
        Span::raw(t!("dialog.model_picker_hints").to_string()).dim(),
    ));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render the help overlay.
fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(format!(" {} ", t!("dialog.keyboard_shortcuts")).bold())
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = vec![
        Line::from(vec![
            "Tab".bold().into(),
            Span::raw(format!("         {}", t!("help.tab"))),
        ]),
        Line::from(vec![
            "Ctrl+T".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_t"))),
        ]),
        Line::from(vec![
            "Ctrl+Shift+T".bold().into(),
            Span::raw(format!(" {}", t!("help.ctrl_shift_t"))),
        ]),
        Line::from(vec![
            "Ctrl+M".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_m"))),
        ]),
        Line::from(vec![
            "Ctrl+V".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_v"))),
        ]),
        Line::from(vec![
            "Ctrl+E".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_e"))),
        ]),
        Line::from(vec![
            "Ctrl+L".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_l"))),
        ]),
        Line::from(vec![
            "Ctrl+C".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_c"))),
        ]),
        Line::from(vec![
            "Ctrl+Q".bold().into(),
            Span::raw(format!("      {}", t!("help.ctrl_q"))),
        ]),
        Line::from(""),
        Line::from(vec![
            "Enter".bold().into(),
            Span::raw(format!("       {}", t!("help.enter"))),
        ]),
        Line::from(vec![
            "Shift+Enter".bold().into(),
            Span::raw(format!(" {}", t!("help.shift_enter"))),
        ]),
        Line::from(vec![
            "Esc".bold().into(),
            Span::raw(format!("         {}", t!("help.esc"))),
        ]),
        Line::from(vec![
            "? / F1".bold().into(),
            Span::raw(format!("      {}", t!("help.question_f1"))),
        ]),
        Line::from(""),
        Line::from(vec![
            "↑/↓".bold().into(),
            Span::raw(format!("         {}", t!("help.up_down"))),
        ]),
        Line::from(vec![
            "Alt+↑/↓".bold().into(),
            Span::raw(format!("     {}", t!("help.alt_up_down"))),
        ]),
        Line::from(vec![
            "PgUp/PgDn".bold().into(),
            Span::raw(format!("   {}", t!("help.pgup_pgdn"))),
        ]),
        Line::from(vec![
            "Ctrl+←/→".bold().into(),
            Span::raw(format!("    {}", t!("help.ctrl_left_right"))),
        ]),
        Line::from(vec![
            "Ctrl+Bksp".bold().into(),
            Span::raw(format!("   {}", t!("help.ctrl_bksp"))),
        ]),
        Line::from(""),
        Line::from(Span::raw(t!("dialog.press_esc_close").to_string()).dim()),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the command palette overlay.
fn render_command_palette_overlay(
    frame: &mut Frame,
    area: Rect,
    palette: &crate::state::CommandPaletteOverlay,
) {
    let title = if palette.query.is_empty() {
        format!(" {} ", t!("dialog.command_palette"))
    } else {
        format!(
            " {} ",
            t!("dialog.command_palette_filter", filter = &palette.query)
        )
    };

    let block = Block::default()
        .title(title.bold())
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().cyan());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build command list
    let commands = palette.filtered_commands();
    let mut lines: Vec<Line> = vec![];

    for (i, cmd) in commands.iter().enumerate() {
        let is_selected = palette.selected == i as i32;
        let shortcut_text = cmd
            .shortcut
            .as_ref()
            .map(|s| format!(" ({s})"))
            .unwrap_or_default();

        let line = if is_selected {
            Line::from(vec![
                Span::raw("▸ ").bold().cyan(),
                Span::raw(&cmd.name).bold().cyan(),
                Span::raw(shortcut_text).dim(),
            ])
        } else {
            Line::from(vec![
                Span::raw("  "),
                Span::raw(&cmd.name),
                Span::raw(shortcut_text).dim(),
            ])
        };
        lines.push(line);

        // Add description for selected item
        if is_selected {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::raw(&cmd.description).dim().italic(),
            ]));
        }
    }

    if commands.is_empty() {
        lines.push(Line::from(
            Span::raw(t!("dialog.no_commands_match").to_string())
                .dim()
                .italic(),
        ));
    }

    // Add hints at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(
        Span::raw(t!("dialog.command_palette_hints").to_string()).dim(),
    ));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render the session browser overlay.
fn render_session_browser_overlay(
    frame: &mut Frame,
    area: Rect,
    browser: &crate::state::SessionBrowserOverlay,
) {
    let title = if browser.filter.is_empty() {
        format!(" {} ", t!("dialog.session_browser"))
    } else {
        format!(
            " {} ",
            t!("dialog.session_browser_filter", filter = &browser.filter)
        )
    };

    let block = Block::default()
        .title(title.bold())
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().cyan());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build session list
    let sessions = browser.filtered_sessions();
    let mut lines: Vec<Line> = vec![];

    if sessions.is_empty() {
        lines.push(Line::from(
            Span::raw(t!("dialog.no_saved_sessions").to_string())
                .dim()
                .italic(),
        ));
    } else {
        for (i, session) in sessions.iter().enumerate() {
            let is_selected = browser.selected == i as i32;
            let msg_count = t!(
                "dialog.session_message_count",
                count = session.message_count
            )
            .to_string();
            let line = if is_selected {
                Line::from(vec![
                    Span::raw("▸ ").bold().cyan(),
                    Span::raw(&session.title).bold().cyan(),
                    Span::raw(format!(" {msg_count}")).dim(),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::raw(&session.title),
                    Span::raw(format!(" {msg_count}")).dim(),
                ])
            };
            lines.push(line);
        }
    }

    // Add hints at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(
        Span::raw(t!("dialog.session_browser_hints").to_string()).dim(),
    ));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render an error overlay.
fn render_error_overlay(frame: &mut Frame, area: Rect, message: &str) {
    let block = Block::default()
        .title(format!(" {} ", t!("dialog.error")).bold().red())
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().red());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = vec![
        Line::from(Span::raw(message)),
        Line::from(""),
        Line::from(Span::raw(t!("dialog.press_esc_enter_dismiss").to_string()).dim()),
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
                    proposed_prefix_pattern: None,
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
