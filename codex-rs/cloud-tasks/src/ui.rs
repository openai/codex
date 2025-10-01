use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::ListState;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use std::sync::OnceLock;

use crate::app::App;
use crate::app::AttemptView;
use chrono::Local;
use chrono::Utc;
use codex_cloud_tasks_client::AttemptStatus;
use codex_cloud_tasks_client::TaskStatus;
use codex_tui::render_markdown_text;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // list
            Constraint::Length(2), // two-line footer (help + status)
        ])
        .split(area);
    if app.new_task.is_some() {
        draw_new_task_page(frame, chunks[0], app);
        draw_footer(frame, chunks[1], app);
    } else {
        draw_list(frame, chunks[0], app);
        draw_footer(frame, chunks[1], app);
    }

    if app.diff_overlay.is_some() {
        draw_diff_overlay(frame, area, app);
    }
    if app.env_modal.is_some() {
        draw_env_modal(frame, area, app);
    }
    if app.best_of_modal.is_some() {
        draw_best_of_modal(frame, area, app);
    }
    if app.apply_modal.is_some() {
        draw_apply_modal(frame, area, app);
    }
}

// ===== Overlay helpers (geometry + styling) =====
static ROUNDED: OnceLock<bool> = OnceLock::new();

fn rounded_enabled() -> bool {
    *ROUNDED.get_or_init(|| {
        std::env::var("CODEX_TUI_ROUNDED")
            .ok()
            .map(|v| v == "1")
            .unwrap_or(true)
    })
}

fn overlay_outer(area: Rect) -> Rect {
    let outer_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(outer_v)[1]
}

fn overlay_block() -> Block<'static> {
    let base = Block::default().borders(Borders::ALL);
    let base = if rounded_enabled() {
        base.border_type(BorderType::Rounded)
    } else {
        base
    };
    base.padding(Padding::new(2, 2, 1, 1))
}

fn overlay_content(area: Rect) -> Rect {
    overlay_block().inner(area)
}

pub fn draw_new_task_page(frame: &mut Frame, area: Rect, app: &mut App) {
    let title_spans = {
        let mut spans: Vec<ratatui::text::Span> = vec!["New Task".magenta().bold()];
        if let Some(id) = app
            .new_task
            .as_ref()
            .and_then(|p| p.env_id.as_ref())
            .cloned()
        {
            spans.push("  • ".into());
            // Try to map id to label
            let label = app
                .environments
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.label.clone())
                .unwrap_or(id);
            spans.push(label.dim());
        } else {
            spans.push("  • ".into());
            spans.push("Env: none (press ctrl-o to choose)".red());
        }
        if let Some(page) = app.new_task.as_ref() {
            spans.push("  • ".into());
            let attempts = page.best_of_n;
            let label = format!(
                "{} attempt{}",
                attempts,
                if attempts == 1 { "" } else { "s" }
            );
            spans.push(label.cyan());
        }
        spans
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_spans));

    frame.render_widget(Clear, area);
    frame.render_widget(block.clone(), area);
    let content = block.inner(area);

    // Expand composer height up to (terminal height - 6), with a 3-line minimum.
    let max_allowed = frame.area().height.saturating_sub(6).max(3);
    let desired = app
        .new_task
        .as_ref()
        .map(|p| p.composer.desired_height(content.width))
        .unwrap_or(3)
        .clamp(3, max_allowed);

    // Anchor the composer to the bottom-left by allocating a flexible spacer
    // above it and a fixed `desired`-height area for the composer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(desired)])
        .split(content);
    let composer_area = rows[1];

    if let Some(page) = app.new_task.as_ref() {
        page.composer.render_ref(composer_area, frame.buffer_mut());
        // Composer renders its own footer hints; no extra row here.
    }

    // Place cursor where composer wants it
    if let Some(page) = app.new_task.as_ref()
        && let Some((x, y)) = page.composer.cursor_pos(composer_area)
    {
        frame.set_cursor_position((x, y));
    }
}

fn draw_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app.tasks.iter().map(|t| render_task_item(app, t)).collect();

    // Selection reflects the actual task index (no artificial spacer item).
    let mut state = ListState::default().with_selected(Some(app.selected));
    // Dim task list when a modal/overlay is active to emphasize focus.
    let dim_bg = app.env_modal.is_some()
        || app.apply_modal.is_some()
        || app.best_of_modal.is_some()
        || app.diff_overlay.is_some();
    // Dynamic title includes current environment filter
    let suffix_span = if let Some(ref id) = app.env_filter {
        let label = app
            .environments
            .iter()
            .find(|r| &r.id == id)
            .and_then(|r| r.label.clone())
            .unwrap_or_else(|| "Selected".to_string());
        format!(" • {label}").dim()
    } else {
        " • All".dim()
    };
    // Percent scrolled based on selection position in the list (0% at top, 100% at bottom).
    let percent_span = if app.tasks.len() <= 1 {
        "  • 0%".dim()
    } else {
        let p = ((app.selected as f32) / ((app.tasks.len() - 1) as f32) * 100.0).round() as i32;
        format!("  • {}%", p.clamp(0, 100)).dim()
    };
    let title_line = {
        let base = Line::from(vec!["Cloud Tasks".into(), suffix_span, percent_span]);
        if dim_bg {
            base.style(Style::default().add_modifier(Modifier::DIM))
        } else {
            base
        }
    };
    let block = Block::default().borders(Borders::ALL).title(title_line);
    // Render the outer block first
    frame.render_widget(block.clone(), area);
    // Draw list inside with a persistent top spacer row
    let inner = block.inner(area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    let mut list = List::new(items)
        .highlight_symbol("› ")
        .highlight_style(Style::default().bold());
    if dim_bg {
        list = list.style(Style::default().add_modifier(Modifier::DIM));
    }
    frame.render_stateful_widget(list, rows[1], &mut state);

    // In-box spinner during initial/refresh loads
    if app.refresh_inflight {
        draw_centered_spinner(frame, inner, &mut app.throbber, "Loading tasks…");
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut help = vec![
        "↑/↓".dim(),
        ": Move  ".dim(),
        "r".dim(),
        ": Refresh  ".dim(),
        "Enter".dim(),
        ": Open  ".dim(),
    ];
    // Apply hint; show disabled note when overlay is open without a diff.
    if let Some(ov) = app.diff_overlay.as_ref() {
        if !ov.current_can_apply() {
            help.push("a".dim());
            help.push(": Apply (disabled)  ".dim());
        } else {
            help.push("a".dim());
            help.push(": Apply  ".dim());
        }
        if ov.attempt_count() > 1 {
            help.push("Tab".dim());
            help.push(": Next attempt  ".dim());
            help.push("[ ]".dim());
            help.push(": Cycle attempts  ".dim());
        }
    } else {
        help.push("a".dim());
        help.push(": Apply  ".dim());
    }
    help.push("o : Set Env  ".dim());
    if app.new_task.is_some() {
        help.push("Ctrl+N".dim());
        help.push(format!(": Attempts {}x  ", app.best_of_n).dim());
        help.push("(editing new task)  ".dim());
    } else {
        help.push("n : New Task  ".dim());
    }
    help.extend(vec!["q".dim(), ": Quit  ".dim()]);
    // Split footer area into two rows: help+spinner (top) and status (bottom)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Top row: help text + spinner at right
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(18)])
        .split(rows[0]);
    let para = Paragraph::new(Line::from(help));
    // Draw help text; avoid clearing the whole footer area every frame.
    frame.render_widget(para, top[0]);
    // Right side: spinner or clear the spinner area if idle to prevent stale glyphs.
    if app.refresh_inflight
        || app.details_inflight
        || app.env_loading
        || app.apply_preflight_inflight
        || app.apply_inflight
    {
        draw_inline_spinner(frame, top[1], &mut app.throbber, "Loading…");
    } else {
        frame.render_widget(Clear, top[1]);
    }

    // Bottom row: status/log text across full width (single-line; sanitize newlines)
    let mut status_line = app.status.replace('\n', " ");
    if status_line.len() > 2000 {
        // hard cap to avoid TUI noise
        status_line.truncate(2000);
        status_line.push('…');
    }
    // Clear the status row to avoid trailing characters when the message shrinks.
    frame.render_widget(Clear, rows[1]);
    let status = Paragraph::new(status_line);
    frame.render_widget(status, rows[1]);
}

fn draw_diff_overlay(frame: &mut Frame, area: Rect, app: &mut App) {
    let inner = overlay_outer(area);
    if app.diff_overlay.is_none() {
        return;
    }
    let ov_can_apply = app
        .diff_overlay
        .as_ref()
        .map(super::app::DiffOverlay::current_can_apply)
        .unwrap_or(false);
    let is_error = app
        .diff_overlay
        .as_ref()
        .and_then(|o| o.sd.wrapped_lines().first().cloned())
        .map(|s| s.trim_start().starts_with("Task failed:"))
        .unwrap_or(false)
        && !ov_can_apply;
    let title = app
        .diff_overlay
        .as_ref()
        .map(|o| o.title.clone())
        .unwrap_or_default();

    // Title block
    let title_ref = title.as_str();
    let mut title_spans: Vec<ratatui::text::Span> = if is_error {
        vec![
            "Details ".magenta(),
            "[FAILED]".red().bold(),
            " ".into(),
            title_ref.magenta(),
        ]
    } else if ov_can_apply {
        vec!["Diff: ".magenta(), title_ref.magenta()]
    } else {
        vec!["Details: ".magenta(), title_ref.magenta()]
    };
    if let Some(p) = app
        .diff_overlay
        .as_ref()
        .and_then(|o| o.sd.percent_scrolled())
    {
        title_spans.push("  • ".dim());
        title_spans.push(format!("{p}%").dim());
    }
    frame.render_widget(Clear, inner);
    frame.render_widget(
        overlay_block().title(Line::from(title_spans)).clone(),
        inner,
    );

    // Content area and optional status bar
    let content_full = overlay_content(inner);
    let mut content_area = content_full;
    if let Some(ov) = app.diff_overlay.as_mut() {
        let has_text = ov.current_attempt().is_some_and(AttemptView::has_text);
        let has_diff = ov.current_attempt().is_some_and(AttemptView::has_diff) || ov.base_can_apply;
        if has_diff || has_text {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(content_full);
            // Status bar label
            let mut spans: Vec<ratatui::text::Span> = Vec::new();
            if has_diff && has_text {
                let prompt_lbl = if matches!(ov.current_view, crate::app::DetailView::Prompt) {
                    "[Prompt]".magenta().bold()
                } else {
                    "Prompt".dim()
                };
                let diff_lbl = if matches!(ov.current_view, crate::app::DetailView::Diff) {
                    "[Diff]".magenta().bold()
                } else {
                    "Diff".dim()
                };
                spans.extend(vec![
                    prompt_lbl,
                    "  ".into(),
                    diff_lbl,
                    "  ".into(),
                    "(← → to switch view)".dim(),
                ]);
            } else if has_text {
                spans.push("Conversation".magenta().bold());
            } else {
                spans.push("Diff".magenta().bold());
            }
            if let Some(total) = ov.expected_attempts().or({
                if ov.attempts.is_empty() {
                    None
                } else {
                    Some(ov.attempts.len())
                }
            }) && total > 1
            {
                spans.extend(vec![
                    "  ".into(),
                    format!("Attempt {}/{}", ov.selected_attempt + 1, total)
                        .bold()
                        .dim(),
                    "  ".into(),
                    "(Tab/Shift-Tab or [ ] to cycle attempts)".dim(),
                ]);
            }
            frame.render_widget(Paragraph::new(Line::from(spans)), rows[0]);
            ov.sd.set_width(rows[1].width);
            ov.sd.set_viewport(rows[1].height);
            content_area = rows[1];
        } else {
            ov.sd.set_width(content_full.width);
            ov.sd.set_viewport(content_full.height);
            content_area = content_full;
        }
    }

    // Styled content render
    // Choose styling by the active view, not just presence of a diff
    let is_diff_view = app
        .diff_overlay
        .as_ref()
        .map(|o| matches!(o.current_view, crate::app::DetailView::Diff))
        .unwrap_or(false);
    let styled_lines: Vec<Line<'static>> = if is_diff_view {
        let raw = app.diff_overlay.as_ref().map(|o| o.sd.wrapped_lines());
        raw.unwrap_or(&[])
            .iter()
            .map(|l| style_diff_line(l))
            .collect()
    } else {
        app.diff_overlay
            .as_ref()
            .map(|o| style_conversation_lines(&o.sd, o.current_attempt()))
            .unwrap_or_default()
    };
    let raw_empty = app
        .diff_overlay
        .as_ref()
        .map(|o| o.sd.wrapped_lines().is_empty())
        .unwrap_or(true);
    if app.details_inflight && raw_empty {
        draw_centered_spinner(frame, content_area, &mut app.throbber, "Loading details…");
    } else {
        let scroll = app
            .diff_overlay
            .as_ref()
            .map(|o| o.sd.state.scroll)
            .unwrap_or(0);
        let content = Paragraph::new(Text::from(styled_lines)).scroll((scroll, 0));
        frame.render_widget(content, content_area);
    }
}

pub fn draw_apply_modal(frame: &mut Frame, area: Rect, app: &mut App) {
    use ratatui::widgets::Wrap;
    let inner = overlay_outer(area);
    let title = Line::from("Apply Changes?".magenta().bold());
    let block = overlay_block().title(title);
    frame.render_widget(Clear, inner);
    frame.render_widget(block.clone(), inner);
    let content = overlay_content(inner);

    if let Some(m) = &app.apply_modal {
        // Header
        let header = Paragraph::new(Line::from(
            format!("Apply '{}' ?", m.title).magenta().bold(),
        ))
        .wrap(Wrap { trim: true });
        // Footer instructions
        let footer =
            Paragraph::new(Line::from("Press Y to apply, P to preflight, N to cancel.").dim())
                .wrap(Wrap { trim: true });

        // Split into header/body/footer
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(content);

        frame.render_widget(header, rows[0]);
        // Body: spinner while preflight/apply runs; otherwise show result message and path lists
        if app.apply_preflight_inflight {
            draw_centered_spinner(frame, rows[1], &mut app.throbber, "Checking…");
        } else if app.apply_inflight {
            draw_centered_spinner(frame, rows[1], &mut app.throbber, "Applying…");
        } else if m.result_message.is_none() {
            draw_centered_spinner(frame, rows[1], &mut app.throbber, "Loading…");
        } else if let Some(msg) = &m.result_message {
            let mut body_lines: Vec<Line> = Vec::new();
            let first = match m.result_level {
                Some(crate::app::ApplyResultLevel::Success) => msg.clone().green(),
                Some(crate::app::ApplyResultLevel::Partial) => msg.clone().magenta(),
                Some(crate::app::ApplyResultLevel::Error) => msg.clone().red(),
                None => msg.clone().into(),
            };
            body_lines.push(Line::from(first));

            // On partial or error, show conflicts/skips if present
            if !matches!(m.result_level, Some(crate::app::ApplyResultLevel::Success)) {
                use ratatui::text::Span;
                if !m.conflict_paths.is_empty() {
                    body_lines.push(Line::from(""));
                    body_lines.push(
                        Line::from(format!("Conflicts ({}):", m.conflict_paths.len()))
                            .red()
                            .bold(),
                    );
                    for p in &m.conflict_paths {
                        body_lines
                            .push(Line::from(vec!["  • ".into(), Span::raw(p.clone()).dim()]));
                    }
                }
                if !m.skipped_paths.is_empty() {
                    body_lines.push(Line::from(""));
                    body_lines.push(
                        Line::from(format!("Skipped ({}):", m.skipped_paths.len()))
                            .magenta()
                            .bold(),
                    );
                    for p in &m.skipped_paths {
                        body_lines
                            .push(Line::from(vec!["  • ".into(), Span::raw(p.clone()).dim()]));
                    }
                }
            }
            let body = Paragraph::new(body_lines).wrap(Wrap { trim: true });
            frame.render_widget(body, rows[1]);
        }
        frame.render_widget(footer, rows[2]);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConversationSpeaker {
    User,
    Assistant,
}

fn style_conversation_lines(
    sd: &crate::scrollable_diff::ScrollableDiff,
    attempt: Option<&AttemptView>,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    let wrapped = sd.wrapped_lines();
    if wrapped.is_empty() {
        return Vec::new();
    }

    let indices = sd.wrapped_src_indices();
    let mut styled: Vec<Line<'static>> = Vec::new();
    let mut speaker: Option<ConversationSpeaker> = None;
    let mut in_code = false;
    let mut last_src: Option<usize> = None;
    let mut bullet_indent: Option<usize> = None;

    for (display, &src_idx) in wrapped.iter().zip(indices.iter()) {
        let raw = sd.raw_line_at(src_idx);
        let trimmed = raw.trim();
        let is_new_raw = last_src.map(|prev| prev != src_idx).unwrap_or(true);

        if trimmed.eq_ignore_ascii_case("user:") {
            speaker = Some(ConversationSpeaker::User);
            in_code = false;
            bullet_indent = None;
            styled.push(conversation_header_line(ConversationSpeaker::User, None));
            last_src = Some(src_idx);
            continue;
        }
        if trimmed.eq_ignore_ascii_case("assistant:") {
            speaker = Some(ConversationSpeaker::Assistant);
            in_code = false;
            bullet_indent = None;
            styled.push(conversation_header_line(
                ConversationSpeaker::Assistant,
                attempt,
            ));
            last_src = Some(src_idx);
            continue;
        }
        if raw.is_empty() {
            let mut spans: Vec<Span> = Vec::new();
            if let Some(role) = speaker {
                spans.push(conversation_gutter_span(role));
            } else {
                spans.push(Span::raw(String::new()));
            }
            styled.push(Line::from(spans));
            last_src = Some(src_idx);
            bullet_indent = None;
            continue;
        }

        if is_new_raw {
            let trimmed_start = raw.trim_start();
            if trimmed_start.starts_with("```") {
                in_code = !in_code;
                bullet_indent = None;
            } else if !in_code
                && (trimmed_start.starts_with("- ") || trimmed_start.starts_with("* "))
            {
                let indent = raw.chars().take_while(|c| c.is_whitespace()).count();
                bullet_indent = Some(indent);
            } else if !in_code {
                bullet_indent = None;
            }
        }

        let mut spans: Vec<Span> = Vec::new();
        if let Some(role) = speaker {
            spans.push(conversation_gutter_span(role));
        }

        spans.extend(conversation_text_spans(
            display,
            in_code,
            is_new_raw,
            bullet_indent,
        ));

        styled.push(Line::from(spans));
        last_src = Some(src_idx);
    }

    if styled.is_empty() {
        wrapped.iter().map(|l| Line::from(l.to_string())).collect()
    } else {
        styled
    }
}

fn conversation_header_line(
    speaker: ConversationSpeaker,
    attempt: Option<&AttemptView>,
) -> Line<'static> {
    use ratatui::text::Span;

    let mut spans: Vec<Span> = vec!["╭ ".dim()];
    match speaker {
        ConversationSpeaker::User => {
            spans.push("User".cyan().bold());
            spans.push(" prompt".dim());
        }
        ConversationSpeaker::Assistant => {
            spans.push("Assistant".magenta().bold());
            spans.push(" response".dim());
            if let Some(attempt) = attempt
                && let Some(status_span) = attempt_status_span(attempt.status)
            {
                spans.push("  • ".dim());
                spans.push(status_span);
            }
        }
    }
    Line::from(spans)
}

fn conversation_gutter_span(speaker: ConversationSpeaker) -> ratatui::text::Span<'static> {
    match speaker {
        ConversationSpeaker::User => "│ ".cyan().dim(),
        ConversationSpeaker::Assistant => "│ ".magenta().dim(),
    }
}

fn conversation_text_spans(
    display: &str,
    in_code: bool,
    is_new_raw: bool,
    bullet_indent: Option<usize>,
) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::text::Span;

    if in_code {
        return vec![Span::styled(
            display.to_string(),
            Style::default().fg(Color::Cyan),
        )];
    }

    let trimmed = display.trim_start();

    if let Some(indent) = bullet_indent {
        if is_new_raw {
            let rest = trimmed.get(2..).unwrap_or("").trim_start();
            let mut spans: Vec<Span> = Vec::new();
            if indent > 0 {
                spans.push(Span::raw(" ".repeat(indent)));
            }
            spans.push("• ".into());
            spans.push(Span::raw(rest.to_string()));
            return spans;
        }
        let mut continuation = String::new();
        continuation.push_str(&" ".repeat(indent + 2));
        continuation.push_str(trimmed);
        return vec![Span::raw(continuation)];
    }

    if is_new_raw
        && (trimmed.starts_with("### ") || trimmed.starts_with("## ") || trimmed.starts_with("# "))
    {
        return vec![Span::styled(
            display.to_string(),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )];
    }

    let mut rendered = render_markdown_text(display);
    if rendered.lines.is_empty() {
        return vec![Span::raw(display.to_string())];
    }
    // `render_markdown_text` can yield multiple lines when the input contains
    // explicit breaks. We only expect a single line here; join the spans of the
    // first rendered line for styling.
    rendered.lines.remove(0).spans.into_iter().collect()
}

fn attempt_status_span(status: AttemptStatus) -> Option<ratatui::text::Span<'static>> {
    match status {
        AttemptStatus::Completed => Some("Completed".green()),
        AttemptStatus::Failed => Some("Failed".red().bold()),
        AttemptStatus::InProgress => Some("In progress".magenta()),
        AttemptStatus::Pending => Some("Pending".cyan()),
        AttemptStatus::Cancelled => Some("Cancelled".dim()),
        AttemptStatus::Unknown => None,
    }
}

fn style_diff_line(raw: &str) -> Line<'static> {
    use ratatui::style::Color;
    use ratatui::style::Modifier;
    use ratatui::style::Style;
    use ratatui::text::Span;

    if raw.starts_with("@@") {
        return Line::from(vec![Span::styled(
            raw.to_string(),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )]);
    }
    if raw.starts_with("+++") || raw.starts_with("---") {
        return Line::from(vec![Span::styled(
            raw.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]);
    }
    if raw.starts_with('+') {
        return Line::from(vec![Span::styled(
            raw.to_string(),
            Style::default().fg(Color::Green),
        )]);
    }
    if raw.starts_with('-') {
        return Line::from(vec![Span::styled(
            raw.to_string(),
            Style::default().fg(Color::Red),
        )]);
    }
    Line::from(vec![Span::raw(raw.to_string())])
}

fn render_task_item(_app: &App, t: &codex_cloud_tasks_client::TaskSummary) -> ListItem<'static> {
    let status = match t.status {
        TaskStatus::Ready => "READY".green(),
        TaskStatus::Pending => "PENDING".magenta(),
        TaskStatus::Applied => "APPLIED".blue(),
        TaskStatus::Error => "ERROR".red(),
    };

    // Title line: [STATUS] Title
    let title = Line::from(vec![
        "[".into(),
        status,
        "] ".into(),
        t.title.clone().into(),
    ]);

    // Meta line: environment label and relative time (dim)
    let mut meta: Vec<ratatui::text::Span> = Vec::new();
    if let Some(lbl) = t.environment_label.as_ref().filter(|s| !s.is_empty()) {
        meta.push(lbl.clone().dim());
    }
    let when = format_relative_time(t.updated_at).dim();
    if !meta.is_empty() {
        meta.push("  ".into());
        meta.push("•".dim());
        meta.push("  ".into());
    }
    meta.push(when);
    let meta_line = Line::from(meta);

    // Subline: summary when present; otherwise show "no diff"
    let sub = if t.summary.files_changed > 0
        || t.summary.lines_added > 0
        || t.summary.lines_removed > 0
    {
        let adds = t.summary.lines_added;
        let dels = t.summary.lines_removed;
        let files = t.summary.files_changed;
        Line::from(vec![
            format!("+{adds}").green(),
            "/".into(),
            format!("−{dels}").red(),
            " ".into(),
            "•".dim(),
            " ".into(),
            format!("{files}").into(),
            " ".into(),
            "files".dim(),
        ])
    } else {
        Line::from("no diff".to_string().dim())
    };

    // Insert a blank spacer line after the summary to separate tasks
    let spacer = Line::from("");
    ListItem::new(vec![title, meta_line, sub, spacer])
}

fn format_relative_time(ts: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let mut secs = (now - ts).num_seconds();
    if secs < 0 {
        secs = 0;
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let local = ts.with_timezone(&Local);
    local.format("%b %e %H:%M").to_string()
}

fn draw_inline_spinner(
    frame: &mut Frame,
    area: Rect,
    state: &mut throbber_widgets_tui::ThrobberState,
    label: &str,
) {
    use ratatui::style::Style;
    use throbber_widgets_tui::BRAILLE_EIGHT;
    use throbber_widgets_tui::Throbber;
    use throbber_widgets_tui::WhichUse;
    let w = Throbber::default()
        .label(label)
        .style(Style::default().cyan())
        .throbber_style(Style::default().magenta().bold())
        .throbber_set(BRAILLE_EIGHT)
        .use_type(WhichUse::Spin);
    frame.render_stateful_widget(w, area, state);
}

fn draw_centered_spinner(
    frame: &mut Frame,
    area: Rect,
    state: &mut throbber_widgets_tui::ThrobberState,
    label: &str,
) {
    // Center a 1xN throbber within the given rect
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(49),
        ])
        .split(area);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(18),
            Constraint::Percentage(50),
        ])
        .split(rows[1]);
    draw_inline_spinner(frame, cols[1], state, label);
}

// Styling helpers for diff rendering live inline where used.

pub fn draw_env_modal(frame: &mut Frame, area: Rect, app: &mut App) {
    use ratatui::widgets::Wrap;

    // Use shared overlay geometry and padding.
    let inner = overlay_outer(area);

    // Title: primary only; move long hints to a subheader inside content.
    let title = Line::from(vec!["Select Environment".magenta().bold()]);
    let block = overlay_block().title(title);

    frame.render_widget(Clear, inner);
    frame.render_widget(block.clone(), inner);
    let content = overlay_content(inner);

    if app.env_loading {
        draw_centered_spinner(frame, content, &mut app.throbber, "Loading environments…");
        return;
    }

    // Layout: subheader + search + results list
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // subheader
            Constraint::Length(1), // search
            Constraint::Min(1),    // list
        ])
        .split(content);

    // Subheader with usage hints (dim cyan)
    let subheader = Paragraph::new(Line::from(
        "Type to search, Enter select, Esc cancel; r refresh"
            .cyan()
            .dim(),
    ))
    .wrap(Wrap { trim: true });
    frame.render_widget(subheader, rows[0]);

    let query = app
        .env_modal
        .as_ref()
        .map(|m| m.query.clone())
        .unwrap_or_default();
    let ql = query.to_lowercase();
    let search = Paragraph::new(format!("Search: {query}")).wrap(Wrap { trim: true });
    frame.render_widget(search, rows[1]);

    // Filter environments by query (case-insensitive substring over label/id/hints)
    let envs: Vec<&crate::app::EnvironmentRow> = app
        .environments
        .iter()
        .filter(|e| {
            if ql.is_empty() {
                return true;
            }
            let mut hay = String::new();
            if let Some(l) = &e.label {
                hay.push_str(&l.to_lowercase());
                hay.push(' ');
            }
            hay.push_str(&e.id.to_lowercase());
            if let Some(h) = &e.repo_hints {
                hay.push(' ');
                hay.push_str(&h.to_lowercase());
            }
            hay.contains(&ql)
        })
        .collect();

    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(Line::from("All Environments (Global)")));
    for env in envs.iter() {
        let primary = env.label.clone().unwrap_or_else(|| "<unnamed>".to_string());
        let mut spans: Vec<ratatui::text::Span> = vec![primary.into()];
        if env.is_pinned {
            spans.push("  ".into());
            spans.push("PINNED".magenta().bold());
        }
        spans.push("  ".into());
        spans.push(env.id.clone().dim());
        if let Some(hint) = &env.repo_hints {
            spans.push("  ".into());
            spans.push(hint.clone().dim());
        }
        items.push(ListItem::new(Line::from(spans)));
    }

    let sel_desired = app.env_modal.as_ref().map(|m| m.selected).unwrap_or(0);
    let sel = sel_desired.min(envs.len());
    let mut list_state = ListState::default().with_selected(Some(sel));
    let list = List::new(items)
        .highlight_symbol("› ")
        .highlight_style(Style::default().bold())
        .block(Block::default().borders(Borders::NONE));
    frame.render_stateful_widget(list, rows[2], &mut list_state);
}

pub fn draw_best_of_modal(frame: &mut Frame, area: Rect, app: &mut App) {
    use ratatui::widgets::Wrap;

    let inner = overlay_outer(area);
    const MAX_WIDTH: u16 = 40;
    const MIN_WIDTH: u16 = 20;
    const MAX_HEIGHT: u16 = 12;
    const MIN_HEIGHT: u16 = 6;
    let modal_width = inner.width.min(MAX_WIDTH).max(inner.width.min(MIN_WIDTH));
    let modal_height = inner
        .height
        .min(MAX_HEIGHT)
        .max(inner.height.min(MIN_HEIGHT));
    let modal_x = inner.x + (inner.width.saturating_sub(modal_width)) / 2;
    let modal_y = inner.y + (inner.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);
    let title = Line::from(vec!["Parallel Attempts".magenta().bold()]);
    let block = overlay_block().title(title);

    frame.render_widget(Clear, modal_area);
    frame.render_widget(block.clone(), modal_area);
    let content = overlay_content(modal_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(content);

    let hint = Paragraph::new(Line::from("Use ↑/↓ to choose, 1-4 jump".cyan().dim()))
        .wrap(Wrap { trim: true });
    frame.render_widget(hint, rows[0]);

    let selected = app.best_of_modal.as_ref().map(|m| m.selected).unwrap_or(0);
    let options = [1usize, 2, 3, 4];
    let mut items: Vec<ListItem> = Vec::new();
    for &attempts in &options {
        let noun = if attempts == 1 { "attempt" } else { "attempts" };
        let mut spans: Vec<ratatui::text::Span> = vec![format!("{attempts} {noun:<8}").into()];
        spans.push("  ".into());
        spans.push(format!("{attempts}x parallel").dim());
        if attempts == app.best_of_n {
            spans.push("  ".into());
            spans.push("Current".magenta().bold());
        }
        items.push(ListItem::new(Line::from(spans)));
    }
    let sel = selected.min(options.len().saturating_sub(1));
    let mut list_state = ListState::default().with_selected(Some(sel));
    let list = List::new(items)
        .highlight_symbol("› ")
        .highlight_style(Style::default().bold())
        .block(Block::default().borders(Borders::NONE));
    frame.render_stateful_widget(list, rows[1], &mut list_state);
}
