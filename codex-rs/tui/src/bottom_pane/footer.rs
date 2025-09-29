use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FooterProps {
    pub(crate) mode: FooterMode,
    pub(crate) esc_backtrack_hint: bool,
    pub(crate) use_shift_enter_hint: bool,
    pub(crate) is_task_running: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FooterMode {
    CtrlCReminder,
    ShortcutPrompt,
    ShortcutOverlay,
    EscHint,
}

pub(crate) fn toggle_shortcut_mode(current: FooterMode, ctrl_c_hint: bool) -> FooterMode {
    if ctrl_c_hint {
        return current;
    }
    match current {
        FooterMode::ShortcutOverlay | FooterMode::CtrlCReminder => FooterMode::ShortcutPrompt,
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
        FooterMode::EscHint | FooterMode::ShortcutOverlay => FooterMode::ShortcutPrompt,
        other => other,
    }
}

pub(crate) fn prompt_mode() -> FooterMode {
    FooterMode::ShortcutPrompt
}

pub(crate) fn footer_height(props: FooterProps) -> u16 {
    footer_lines(props).len() as u16
}

pub(crate) fn render_footer(area: Rect, buf: &mut Buffer, props: FooterProps) {
    let lines = footer_lines(props);
    for (idx, line) in lines.into_iter().enumerate() {
        let y = area.y + idx as u16;
        if y >= area.y + area.height {
            break;
        }
        let row = Rect::new(area.x, y, area.width, 1);
        line.render_ref(row, buf);
    }
}

fn footer_lines(props: FooterProps) -> Vec<Line<'static>> {
    match props.mode {
        FooterMode::CtrlCReminder => {
            vec![ctrl_c_reminder_line(CtrlCReminderState {
                is_task_running: props.is_task_running,
            })]
        }
        FooterMode::ShortcutPrompt => vec![Line::from(vec!["? for shortcuts".dim()])],
        FooterMode::ShortcutOverlay => shortcut_overlay_lines(ShortcutsState {
            use_shift_enter_hint: props.use_shift_enter_hint,
            esc_backtrack_hint: props.esc_backtrack_hint,
            is_task_running: props.is_task_running,
        }),
        FooterMode::EscHint => {
            vec![esc_hint_line(ShortcutsState {
                use_shift_enter_hint: props.use_shift_enter_hint,
                esc_backtrack_hint: props.esc_backtrack_hint,
                is_task_running: props.is_task_running,
            })]
        }
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
    is_task_running: bool,
}

fn ctrl_c_reminder_line(state: CtrlCReminderState) -> Line<'static> {
    let action = if state.is_task_running {
        "interrupt"
    } else {
        "quit"
    };
    Line::from(vec![
        Span::from(format!("  ctrl + c again to {action}")).dim(),
    ])
}

fn esc_hint_line(state: ShortcutsState) -> Line<'static> {
    let text = if state.esc_backtrack_hint {
        "  esc again to edit previous message"
    } else {
        "  esc esc to edit previous message"
    };
    Line::from(vec![Span::from(text).dim()])
}

fn shortcut_overlay_lines(state: ShortcutsState) -> Vec<Line<'static>> {
    let mut rendered = Vec::new();
    for descriptor in SHORTCUTS {
        if let Some(text) = descriptor.overlay_entry(state) {
            rendered.push(text);
        }
    }
    build_columns(rendered)
}

fn build_columns(entries: Vec<String>) -> Vec<Line<'static>> {
    if entries.is_empty() {
        return Vec::new();
    }

    const COLUMNS: usize = 3;
    const MAX_PADDED_WIDTHS: [usize; COLUMNS - 1] = [24, 28];
    const MIN_PADDED_WIDTHS: [usize; COLUMNS - 1] = [22, 0];

    let rows = entries.len().div_ceil(COLUMNS);
    let mut column_widths = [0usize; COLUMNS];

    for (idx, entry) in entries.iter().enumerate() {
        let column = idx % COLUMNS;
        column_widths[column] = column_widths[column].max(entry.len());
    }

    let mut lines = Vec::new();
    for row in 0..rows {
        let mut line = String::from("  ");
        for col in 0..COLUMNS {
            let idx = row * COLUMNS + col;
            if idx >= entries.len() {
                continue;
            }
            let entry = &entries[idx];
            if col < COLUMNS - 1 {
                let max_width = MAX_PADDED_WIDTHS[col];
                let mut target_width = column_widths[col];
                target_width = target_width.max(MIN_PADDED_WIDTHS[col]).min(max_width);
                let pad_width = target_width + 2;
                line.push_str(&format!("{entry:<pad_width$}"));
            } else {
                if col != 0 {
                    line.push_str("  ");
                }
                line.push_str(entry);
            }
        }
        lines.push(Line::from(vec![Span::from(line).dim()]));
    }

    lines
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShortcutId {
    Commands,
    InsertNewline,
    ChangeMode,
    FilePaths,
    PasteImage,
    EditPrevious,
    Quit,
    ShowTranscript,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShortcutBinding {
    code: KeyCode,
    modifiers: KeyModifiers,
    overlay_text: &'static str,
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
}

impl DisplayCondition {
    fn matches(self, state: ShortcutsState) -> bool {
        match self {
            DisplayCondition::Always => true,
            DisplayCondition::WhenShiftEnterHint => state.use_shift_enter_hint,
            DisplayCondition::WhenNotShiftEnterHint => !state.use_shift_enter_hint,
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

    fn overlay_entry(&self, state: ShortcutsState) -> Option<String> {
        let binding = self.binding_for(state)?;
        let label = match self.id {
            ShortcutId::Quit => {
                if state.is_task_running {
                    " to interrupt"
                } else {
                    self.label
                }
            }
            ShortcutId::EditPrevious => {
                if state.esc_backtrack_hint {
                    " again to edit previous message"
                } else {
                    " esc to edit previous message"
                }
            }
            _ => self.label,
        };
        let text = match self.id {
            ShortcutId::Quit if state.is_task_running => {
                format!("{}{} to interrupt", self.prefix, binding.overlay_text)
            }
            _ => format!("{}{}{}", self.prefix, binding.overlay_text, label),
        };
        Some(text)
    }
}

const SHORTCUTS: &[ShortcutDescriptor] = &[
    ShortcutDescriptor {
        id: ShortcutId::Commands,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('/'),
            modifiers: KeyModifiers::NONE,
            overlay_text: "/",
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " for commands",
    },
    ShortcutDescriptor {
        id: ShortcutId::InsertNewline,
        bindings: &[
            ShortcutBinding {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::SHIFT,
                overlay_text: "shift + enter",
                condition: DisplayCondition::WhenShiftEnterHint,
            },
            ShortcutBinding {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                overlay_text: "ctrl + j",
                condition: DisplayCondition::WhenNotShiftEnterHint,
            },
        ],
        prefix: "",
        label: " for newline",
    },
    ShortcutDescriptor {
        id: ShortcutId::ChangeMode,
        bindings: &[ShortcutBinding {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::SHIFT,
            overlay_text: "shift + tab",
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " to change mode",
    },
    ShortcutDescriptor {
        id: ShortcutId::FilePaths,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('@'),
            modifiers: KeyModifiers::NONE,
            overlay_text: "@",
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " for file paths",
    },
    ShortcutDescriptor {
        id: ShortcutId::PasteImage,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('v'),
            modifiers: KeyModifiers::CONTROL,
            overlay_text: "ctrl + v",
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " to paste images",
    },
    ShortcutDescriptor {
        id: ShortcutId::EditPrevious,
        bindings: &[ShortcutBinding {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            overlay_text: "esc",
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: "",
    },
    ShortcutDescriptor {
        id: ShortcutId::Quit,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            overlay_text: "ctrl + c",
            condition: DisplayCondition::Always,
        }],
        prefix: "",
        label: " to exit",
    },
    ShortcutDescriptor {
        id: ShortcutId::ShowTranscript,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::CONTROL,
            overlay_text: "ctrl + t",
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
                mode: FooterMode::ShortcutPrompt,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
            },
        );

        snapshot_footer(
            "footer_shortcuts_shift_and_esc",
            FooterProps {
                mode: FooterMode::ShortcutOverlay,
                esc_backtrack_hint: true,
                use_shift_enter_hint: true,
                is_task_running: false,
            },
        );

        snapshot_footer(
            "footer_ctrl_c_quit_idle",
            FooterProps {
                mode: FooterMode::CtrlCReminder,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
            },
        );

        snapshot_footer(
            "footer_ctrl_c_quit_running",
            FooterProps {
                mode: FooterMode::CtrlCReminder,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: true,
            },
        );

        snapshot_footer(
            "footer_esc_hint_idle",
            FooterProps {
                mode: FooterMode::EscHint,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                is_task_running: false,
            },
        );

        snapshot_footer(
            "footer_esc_hint_primed",
            FooterProps {
                mode: FooterMode::EscHint,
                esc_backtrack_hint: true,
                use_shift_enter_hint: false,
                is_task_running: false,
            },
        );
    }
}
