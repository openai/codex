use codex_core::protocol::TokenUsageInfo;
use codex_protocol::num_format::format_si_suffix;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;

use crate::key_hint;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FooterProps<'a> {
    pub(crate) ctrl_c_quit_hint: bool,
    pub(crate) is_task_running: bool,
    pub(crate) esc_backtrack_hint: bool,
    pub(crate) use_shift_enter_hint: bool,
    pub(crate) token_usage_info: Option<&'a TokenUsageInfo>,
}

#[derive(Clone, Copy, Debug)]
struct CtrlCReminderState {
    pub(crate) is_task_running: bool,
}

#[derive(Clone, Copy, Debug)]
struct ShortcutsState {
    pub(crate) use_shift_enter_hint: bool,
    pub(crate) esc_backtrack_hint: bool,
}

#[derive(Clone, Copy, Debug)]
enum FooterContent {
    Shortcuts(ShortcutsState),
    CtrlCReminder(CtrlCReminderState),
}

pub(crate) fn render_footer(area: Rect, buf: &mut Buffer, props: FooterProps<'_>) {
    let content = if props.ctrl_c_quit_hint {
        FooterContent::CtrlCReminder(CtrlCReminderState {
            is_task_running: props.is_task_running,
        })
    } else {
        FooterContent::Shortcuts(ShortcutsState {
            use_shift_enter_hint: props.use_shift_enter_hint,
            esc_backtrack_hint: props.esc_backtrack_hint,
        })
    };

    let mut spans = footer_spans(content);
    if let Some(token_usage_info) = props.token_usage_info {
        append_token_usage_spans(&mut spans, token_usage_info);
    }

    let spans = spans
        .into_iter()
        .map(|span| span.patch_style(Style::default().dim()))
        .collect::<Vec<_>>();
    Line::from(spans).render_ref(area, buf);
}

fn footer_spans(content: FooterContent) -> Vec<Span<'static>> {
    match content {
        FooterContent::Shortcuts(state) => shortcuts_spans(state),
        FooterContent::CtrlCReminder(state) => ctrl_c_reminder_spans(state),
    }
}

fn append_token_usage_spans(spans: &mut Vec<Span<'static>>, token_usage_info: &TokenUsageInfo) {
    let token_usage = &token_usage_info.total_token_usage;
    spans.push("   ".into());
    spans.push(
        Span::from(format!(
            "{} tokens used",
            format_si_suffix(token_usage.blended_total())
        ))
        .style(Style::default().add_modifier(Modifier::DIM)),
    );

    let last_token_usage = &token_usage_info.last_token_usage;
    if let Some(context_window) = token_usage_info.model_context_window {
        let percent_remaining: u8 = if context_window > 0 {
            last_token_usage.percent_of_context_window_remaining(context_window)
        } else {
            100
        };

        let context_style = if percent_remaining < 20 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        spans.push("   ".into());
        spans.push(Span::styled(
            format!("{percent_remaining}% context left"),
            context_style,
        ));
    }
}

fn shortcuts_spans(state: ShortcutsState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for descriptor in SHORTCUTS {
        if let Some(segment) = descriptor.footer_segment(state) {
            if !segment.prefix.is_empty() {
                spans.push(segment.prefix.into());
            }
            spans.push(segment.binding.span());
            spans.push(segment.label.into());
        }
    }
    spans
}

fn ctrl_c_reminder_spans(state: CtrlCReminderState) -> Vec<Span<'static>> {
    let followup = if state.is_task_running {
        " to interrupt"
    } else {
        " to quit"
    };
    vec![
        " ".into(),
        key_hint::ctrl('C'),
        " again".into(),
        followup.into(),
    ]
}

#[derive(Clone, Copy, Debug)]
struct FooterSegment {
    prefix: &'static str,
    binding: ShortcutBinding,
    label: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum ShortcutId {
    Send,
    InsertNewline,
    ShowTranscript,
    Quit,
    EditPrevious,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShortcutBinding {
    code: KeyCode,
    modifiers: KeyModifiers,
    display: ShortcutDisplay,
    condition: DisplayCondition,
}

impl ShortcutBinding {
    fn span(&self) -> Span<'static> {
        self.display.into_span()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShortcutDisplay {
    Plain(&'static str),
    Ctrl(char),
    Shift(char),
}

impl ShortcutDisplay {
    fn into_span(self) -> Span<'static> {
        match self {
            ShortcutDisplay::Plain(text) => key_hint::plain(text),
            ShortcutDisplay::Ctrl(ch) => key_hint::ctrl(ch),
            ShortcutDisplay::Shift(ch) => key_hint::shift(ch),
        }
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
    footer_label: &'static str,
    footer_prefix: &'static str,
}

impl ShortcutDescriptor {
    fn binding_for(&self, state: ShortcutsState) -> Option<ShortcutBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.condition.matches(state))
            .copied()
    }

    fn should_show(&self, state: ShortcutsState) -> bool {
        match self.id {
            ShortcutId::EditPrevious => state.esc_backtrack_hint,
            _ => true,
        }
    }

    fn footer_segment(&self, state: ShortcutsState) -> Option<FooterSegment> {
        if !self.should_show(state) {
            return None;
        }
        let binding = self.binding_for(state)?;
        Some(FooterSegment {
            prefix: self.footer_prefix,
            binding,
            label: self.footer_label,
        })
    }
}

const SHORTCUTS: &[ShortcutDescriptor] = &[
    ShortcutDescriptor {
        id: ShortcutId::Send,
        bindings: &[ShortcutBinding {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            display: ShortcutDisplay::Plain("⏎"),
            condition: DisplayCondition::Always,
        }],
        footer_label: " send   ",
        footer_prefix: "",
    },
    ShortcutDescriptor {
        id: ShortcutId::InsertNewline,
        bindings: &[
            ShortcutBinding {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::SHIFT,
                display: ShortcutDisplay::Shift('⏎'),
                condition: DisplayCondition::WhenShiftEnterHint,
            },
            ShortcutBinding {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                display: ShortcutDisplay::Ctrl('J'),
                condition: DisplayCondition::WhenNotShiftEnterHint,
            },
        ],
        footer_label: " newline   ",
        footer_prefix: "",
    },
    ShortcutDescriptor {
        id: ShortcutId::ShowTranscript,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::CONTROL,
            display: ShortcutDisplay::Ctrl('T'),
            condition: DisplayCondition::Always,
        }],
        footer_label: " transcript   ",
        footer_prefix: "",
    },
    ShortcutDescriptor {
        id: ShortcutId::Quit,
        bindings: &[ShortcutBinding {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            display: ShortcutDisplay::Ctrl('C'),
            condition: DisplayCondition::Always,
        }],
        footer_label: " quit",
        footer_prefix: "",
    },
    ShortcutDescriptor {
        id: ShortcutId::EditPrevious,
        bindings: &[ShortcutBinding {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            display: ShortcutDisplay::Plain("Esc"),
            condition: DisplayCondition::Always,
        }],
        footer_label: " edit prev",
        footer_prefix: "   ",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::protocol::TokenUsage;
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn snapshot_footer(name: &str, props: FooterProps<'_>) {
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();
        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, f.area().width, 1);
                render_footer(area, f.buffer_mut(), props);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    fn token_usage(total_tokens: u64, last_tokens: u64, context_window: u64) -> TokenUsageInfo {
        let usage = TokenUsage {
            input_tokens: total_tokens,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens,
        };
        let last = TokenUsage {
            input_tokens: last_tokens,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens: last_tokens,
        };
        TokenUsageInfo {
            total_token_usage: usage,
            last_token_usage: last,
            model_context_window: Some(context_window),
        }
    }

    #[test]
    fn footer_snapshots() {
        snapshot_footer(
            "footer_shortcuts_default",
            FooterProps {
                ctrl_c_quit_hint: false,
                is_task_running: false,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                token_usage_info: None,
            },
        );

        snapshot_footer(
            "footer_shortcuts_shift_and_esc",
            FooterProps {
                ctrl_c_quit_hint: false,
                is_task_running: false,
                esc_backtrack_hint: true,
                use_shift_enter_hint: true,
                token_usage_info: Some(&token_usage(4_200, 900, 8_000)),
            },
        );

        snapshot_footer(
            "footer_ctrl_c_quit_idle",
            FooterProps {
                ctrl_c_quit_hint: true,
                is_task_running: false,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                token_usage_info: None,
            },
        );

        snapshot_footer(
            "footer_ctrl_c_quit_running",
            FooterProps {
                ctrl_c_quit_hint: true,
                is_task_running: true,
                esc_backtrack_hint: false,
                use_shift_enter_hint: false,
                token_usage_info: None,
            },
        );
    }
}
