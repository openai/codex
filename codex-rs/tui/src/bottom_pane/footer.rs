use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::text::Span;

use crate::key_hint;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CtrlCReminderState {
    pub(crate) is_task_running: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShortcutsState {
    pub(crate) use_shift_enter_hint: bool,
    pub(crate) esc_backtrack_hint: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FooterContent {
    Shortcuts(ShortcutsState),
    CtrlCReminder(CtrlCReminderState),
}

pub(crate) fn footer_spans(content: FooterContent) -> Vec<Span<'static>> {
    match content {
        FooterContent::Shortcuts(state) => shortcuts_spans(state),
        FooterContent::CtrlCReminder(state) => ctrl_c_reminder_spans(state),
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
