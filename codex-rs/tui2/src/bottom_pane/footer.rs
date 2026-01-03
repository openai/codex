#[cfg(target_os = "linux")]
use crate::clipboard_paste::is_probably_wsl;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::render::line_utils::prefix_lines;
use crate::status::format_tokens_compact;
use crate::ui_consts::FOOTER_INDENT_COLS;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FooterProps {
    pub(crate) mode: FooterMode,
    pub(crate) esc_backtrack_hint: bool,
    pub(crate) use_shift_enter_hint: bool,
    pub(crate) is_task_running: bool,
    pub(crate) context_window_percent: Option<i64>,
    pub(crate) context_window_used_tokens: Option<i64>,
    pub(crate) transcript_scrolled: bool,
    pub(crate) transcript_selection_active: bool,
    pub(crate) transcript_scroll_position: Option<(usize, usize)>,
    pub(crate) transcript_copy_selection_key: KeyBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FooterMode {
    CtrlCReminder,
    ShortcutSummary,
    ShortcutOverlay,
    EscHint,
    ContextOnly,
}

pub(crate) fn toggle_shortcut_mode(current: FooterMode, ctrl_c_hint: bool) -> FooterMode {
    if ctrl_c_hint && matches!(current, FooterMode::CtrlCReminder) {
        return current;
    }

    match current {
        FooterMode::ShortcutOverlay | FooterMode::CtrlCReminder => FooterMode::ShortcutSummary,
        _ => FooterMode::ShortcutOverlay,
    }
}

pub(crate) fn esc_hint_mode(current: FooterMode, is_task_running: bool) -> FooterMode {
    if is_task_running {
        current
    } else {
        FooterMode::EscHint
    }
}

pub(crate) fn reset_mode_after_activity(current: FooterMode) -> FooterMode {
    match current {
        FooterMode::EscHint
        | FooterMode::ShortcutOverlay
        | FooterMode::CtrlCReminder
        | FooterMode::ContextOnly => FooterMode::ShortcutSummary,
        other => other,
    }
}

pub(crate) fn footer_height(props: FooterProps) -> u16 {
    footer_lines(props).len() as u16
}

pub(crate) fn render_footer(area: Rect, buf: &mut Buffer, props: FooterProps) {
    Paragraph::new(prefix_lines(
        footer_lines(props),
        " ".repeat(FOOTER_INDENT_COLS).into(),
        " ".repeat(FOOTER_INDENT_COLS).into(),
    ))
    .render(area, buf);
}

fn footer_lines(props: FooterProps) -> Vec<Line<'static>> {
    // Show the context indicator on the left, appended after the primary hint
    // (e.g., "? for shortcuts"). Keep it visible even when typing (i.e., when
    // the shortcut hint is hidden). Hide it only for the multi-line
    // ShortcutOverlay.
    match props.mode {
        FooterMode::CtrlCReminder => vec![ctrl_c_reminder_line(CtrlCReminderState {
            is_task_running: props.is_task_running,
        })],
        FooterMode::ShortcutSummary => {
            let mut line = context_window_line(
                props.context_window_percent,
                props.context_window_used_tokens,
            );
            line.push_span(" · ".dim());
            line.extend(vec![
                key_hint::plain(KeyCode::Char('?')).into(),
                " for shortcuts".dim(),
            ]);
            if props.transcript_scrolled {
                line.push_span(" · ".dim());
                line.push_span(key_hint::plain(KeyCode::PageUp));
                line.push_span("/");
                line.push_span(key_hint::plain(KeyCode::PageDown));
                line.push_span(" scroll".dim());
                line.push_span(" · ".dim());
                line.push_span(key_hint::plain(KeyCode::Home));
                line.push_span("/");
                line.push_span(key_hint::plain(KeyCode::End));
                line.push_span(" jump".dim());
                if let Some((current, total)) = props.transcript_scroll_position {
                    line.push_span(" · ".dim());
                    line.push_span(Span::from(format!("{current}/{total}")).dim());
                }
            }
            if props.transcript_selection_active {
                line.push_span(" · ".dim());
                line.push_span(props.transcript_copy_selection_key);
                line.push_span(" copy selection".dim());
            }
            vec![line]
        }
        FooterMode::ShortcutOverlay => {
            #[cfg(target_os = "linux")]
            let is_wsl = is_probably_wsl();
            #[cfg(not(target_os = "linux"))]
            let is_wsl = false;

            let state = ShortcutsState {
                use_shift_enter_hint: props.use_shift_enter_hint,
                esc_backtrack_hint: props.esc_backtrack_hint,
                is_wsl,
            };
            shortcut_overlay_lines(state)
        }
        FooterMode::EscHint => vec![esc_hint_line(props.esc_backtrack_hint)],
        FooterMode::ContextOnly => vec![context_window_line(
            props.context_window_percent,
            props.context_window_used_tokens,
        )],
    }
}

#[derive(Clone, Copy, Debug)]
struct CtrlCReminderState {
    is_task_running: bool,
}

#[derive(Clone, Copy, Debug)]
struct ShortcutsState {
    use_shift_enter_hint: bool,
    esc_backtrack_hint: bool,
    is_wsl: bool,
}

fn ctrl_c_reminder_line(state: CtrlCReminderState) -> Line<'static> {
    let action = if state.is_task_running {
        "interrupt"
    } else {
        "quit"
    };
    Line::from(vec![
        key_hint::ctrl(KeyCode::Char('c')).into(),
        format!(" again to {action}").into(),
    ])
    .dim()
}

fn esc_hint_line(esc_backtrack_hint: bool) -> Line<'static> {
    let esc = key_hint::plain(KeyCode::Esc);
    if esc_backtrack_hint {
        Line::from(vec![esc.into(), " again to edit previous message".into()]).dim()
    } else {
        Line::from(vec![
            esc.into(),
            " ".into(),
            esc.into(),
            " to edit previous message".into(),
        ])
        .dim()
    }
}

fn shortcut_overlay_lines(state: ShortcutsState) -> Vec<Line<'static>> {
    let mut commands = Line::from("");
    let mut newline = Line::from("");
    let mut file_paths = Line::from("");
    let mut paste_image = Line::from("");
    let mut edit_previous = Line::from("");
    let mut quit = Line::from("");
    let mut show_transcript = Line::from("");

    for descriptor in SHORTCUTS {
        if let Some(text) = descriptor.overlay_entry(state) {
            match descriptor.id {
                ShortcutId::Commands => commands = text,
                ShortcutId::InsertNewline => newline = text,
                ShortcutId::FilePaths => file_paths = text,
                ShortcutId::PasteImage => paste_image = text,
                ShortcutId::EditPrevious => edit_previous = text,
                ShortcutId::Quit => quit = text,
                ShortcutId::ShowTranscript => show_transcript = text,
            }
        }
    }

    let ordered = vec![
        commands,
        newline,
        file_paths,
        paste_image,
        edit_previous,
        quit,
        Line::from(""),
        show_transcript,
    ];

    build_columns(ordered)
}

fn build_columns(entries: Vec<Line<'static>>) -> Vec<Line<'static>> {
    if entries.is_empty() {
        return Vec::new();
    }

    const COLUMNS: usize = 2;
    const COLUMN_PADDING: [usize; COLUMNS] = [4, 4];
    const COLUMN_GAP: usize = 4;

    let rows = entries.len().div_ceil(COLUMNS);
    let target_len = rows * COLUMNS;
    let mut entries = entries;
    if entries.len() < target_len {
        entries.extend(std::iter::repeat_n(
            Line::from(""),
            target_len - entries.len(),
        ));
    }

    let mut column_widths = [0usize; COLUMNS];

    for (idx, entry) in entries.iter().enumerate() {
        let column = idx % COLUMNS;
        column_widths[column] = column_widths[column].max(entry.width());
    }

    for (idx, width) in column_widths.iter_mut().enumerate() {
        *width += COLUMN_PADDING[idx];
    }

    entries
        .chunks(COLUMNS)
        .map(|chunk| {
            let mut line = Line::from("");
            for (col, entry) in chunk.iter().enumerate() {
                line.extend(entry.spans.clone());
                if col < COLUMNS - 1 {
                    let target_width = column_widths[col];
                    let padding = target_width.saturating_sub(entry.width()) + COLUMN_GAP;
                    line.push_span(Span::from(" ".repeat(padding)));
                }
            }
            line.dim()
        })
        .collect()
}

fn context_window_line(percent: Option<i64>, used_tokens: Option<i64>) -> Line<'static> {
    if let Some(percent) = percent {
        let percent = percent.clamp(0, 100);
        return Line::from(vec![Span::from(format!("{percent}% context left")).dim()]);
    }

    if let Some(tokens) = used_tokens {
        let used_fmt = format_tokens_compact(tokens);
        return Line::from(vec![Span::from(format!("{used_fmt} used")).dim()]);
    }

    Line::from(vec![Span::from("100% context left").dim()])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShortcutId {
    Commands,
    InsertNewline,
    FilePaths,
    PasteImage,
    EditPrevious,
    Quit,
    ShowTranscript,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShortcutBinding {
    key: KeyBinding,
    condition: DisplayCondition,
}

impl ShortcutBinding {
    fn matches(&self, state: ShortcutsState) -> bool {
        self.condition.matches(state)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayCondition {
    Always,
    WhenShiftEnterHint,
    WhenNotShiftEnterHint,
    WhenUnderWSL,
}

impl DisplayCondition {
    fn matches(self, state: ShortcutsState) -> bool {
        match self {
            DisplayCondition::Always => true,
            DisplayCondition::WhenShiftEnterHint => state.use_shift_enter_hint,
            DisplayCondition::WhenNotShiftEnterHint => !state.use_shift_enter_hint,
            DisplayCondition::WhenUnderWSL => state.is_wsl,
        }
    }
}

struct ShortcutDescriptor {
    id: ShortcutId,
    bindings: &'static [ShortcutBinding],
    prefix: &'static str,
    label: &'static str,
}

impl ShortcutDescriptor {
    fn binding_for(&self, state: ShortcutsState) -> Option<&'static ShortcutBinding> {
        self.bindings.iter().find(|binding| binding.matches(state))
    }

    fn overlay_entry(&self, state: ShortcutsState) -> Option<Line<'static>> {
        let binding = self.binding_for(state)?;
        let mut line = Line::from(vec![self.prefix.into(), binding.key.into()]);
        match self.id {
            ShortcutId::EditPrevious => {
                if state.esc_backtrack_hint {
                    line.push_span(" again to edit previous message");
                } else {
                    line.extend(vec![
                        " ".into(),
                        key_hint::plain(KeyCode::Esc).into(),
                        " to edit previous message".into(),
                    ]);
                }
            }
            _ => line.push_span(self.label),
        };
        Some(line)
    }
}

const SHORTCUTS: &[ShortcutDescriptor] = &[
    ShortcutDescriptor {
        id: ShortcutId::Commands,
        bindings: &[ShortcutBinding {
            key: key_hint::plain(KeyCode::Char('/')),
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " for commands",
    },
    ShortcutDescriptor {
        id: ShortcutId::InsertNewline,
        bindings: &[
            ShortcutBinding {
                key: key_hint::shift(KeyCode::Enter),
                condition: DisplayCondition::WhenShiftEnterHint,
            },
            ShortcutBinding {
                key: key_hint::ctrl(KeyCode::Char('j')),
                condition: DisplayCondition::WhenNotShiftEnterHint,
            },
        ],
        prefix: "",
        label: " for newline",
    },
    ShortcutDescriptor {
        id: ShortcutId::FilePaths,
        bindings: &[ShortcutBinding {
            key: key_hint::plain(KeyCode::Char('@')),
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " for file paths",
    },
    ShortcutDescriptor {
        id: ShortcutId::PasteImage,
        // Show Ctrl+Alt+V when running under WSL (terminals often intercept plain
        // Ctrl+V); otherwise fall back to Ctrl+V.
        bindings: &[
            ShortcutBinding {
                key: key_hint::ctrl_alt(KeyCode::Char('v')),
                condition: DisplayCondition::WhenUnderWSL,
            },
            ShortcutBinding {
                key: key_hint::ctrl(KeyCode::Char('v')),
                condition: DisplayCondition::Always,
            },
        ],
        prefix: "",
        label: " to paste images",
    },
    ShortcutDescriptor {
        id: ShortcutId::EditPrevious,
        bindings: &[ShortcutBinding {
            key: key_hint::plain(KeyCode::Esc),
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: "",
    },
    ShortcutDescriptor {
        id: ShortcutId::Quit,
        bindings: &[ShortcutBinding {
            key: key_hint::ctrl(KeyCode::Char('c')),
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " to exit",
    },
    ShortcutDescriptor {
        id: ShortcutId::ShowTranscript,
        bindings: &[ShortcutBinding {
            key: key_hint::ctrl(KeyCode::Char('t')),
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " to view transcript",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn snapshot_footer(name: &str, props: FooterProps) {
        let height = footer_height(props).max(1);
        let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, f.area().width, height);
                render_footer(area, f.buffer_mut(), props);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    #[test]
    fn footer_snapshots() {
        snapshot_footer(
            "footer_shortcuts_default",
            FooterProps {
                mode: FooterMode::ShortcutSummary,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_shortcuts_transcript_scrolled_and_selection",
            FooterProps {
                mode: FooterMode::ShortcutSummary,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: true,
                transcript_selection_active: true,
                transcript_scroll_position: Some((3, 42)),
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_shortcuts_shift_and_esc",
            FooterProps {
                mode: FooterMode::ShortcutOverlay,
                esc_backtrack_hint: true,
                use_shift_enter_hint: true,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_ctrl_c_quit_idle",
            FooterProps {
                mode: FooterMode::CtrlCReminder,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_ctrl_c_quit_running",
            FooterProps {
                mode: FooterMode::CtrlCReminder,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: true,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_esc_hint_idle",
            FooterProps {
                mode: FooterMode::EscHint,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_esc_hint_primed",
            FooterProps {
                mode: FooterMode::EscHint,
                esc_backtrack_hint: true,
                use_shift_enter_hint: false,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_shortcuts_context_running",
            FooterProps {
                mode: FooterMode::ShortcutSummary,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: true,
                context_window_percent: Some(72),
                context_window_used_tokens: None,
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );

        snapshot_footer(
            "footer_context_tokens_used",
            FooterProps {
                mode: FooterMode::ShortcutSummary,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
                context_window_percent: None,
                context_window_used_tokens: Some(123_456),
                transcript_scrolled: false,
                transcript_selection_active: false,
                transcript_scroll_position: None,
                transcript_copy_selection_key: key_hint::ctrl_shift(KeyCode::Char('c')),
            },
        );
    }
}
