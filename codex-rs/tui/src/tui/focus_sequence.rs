use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use tokio::time::Instant as TokioInstant;

use super::TuiEvent;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum FocusSequenceState {
    #[default]
    None,
    Esc(KeyEvent),
    EscBracket(KeyEvent, KeyEvent),
}

/// Coalesces split focus change escape sequences so they cannot masquerade as key input.
#[derive(Debug)]
pub(super) struct FocusSequenceBuffer {
    state: FocusSequenceState,
    deadline: Option<TokioInstant>,
    timeout: Duration,
}

impl FocusSequenceBuffer {
    pub(super) fn new(timeout: Duration) -> Self {
        Self {
            state: FocusSequenceState::None,
            deadline: None,
            timeout,
        }
    }

    pub(super) fn deadline(&self) -> Option<TokioInstant> {
        self.deadline
    }

    pub(super) fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
        queue: &mut VecDeque<TuiEvent>,
        terminal_focused: &Arc<AtomicBool>,
    ) -> bool {
        match &self.state {
            FocusSequenceState::None => {
                if Self::is_plain_esc(&key_event) {
                    self.state = FocusSequenceState::Esc(key_event);
                    self.start_deadline();
                    return true;
                }
            }
            FocusSequenceState::Esc(esc) => {
                if Self::is_left_bracket(&key_event) {
                    self.state = FocusSequenceState::EscBracket(*esc, key_event);
                    self.start_deadline();
                    return true;
                }
            }
            FocusSequenceState::EscBracket(_, _) => {
                if Self::is_focus_tail(&key_event) {
                    self.apply_focus_event(key_event, queue, terminal_focused);
                    return true;
                }
            }
        }

        if !matches!(self.state, FocusSequenceState::None) {
            self.flush_as_keys(queue);
            if Self::is_plain_esc(&key_event) {
                self.state = FocusSequenceState::Esc(key_event);
                self.start_deadline();
                return true;
            }
        }

        false
    }

    pub(super) fn flush_as_keys(&mut self, queue: &mut VecDeque<TuiEvent>) {
        match std::mem::take(&mut self.state) {
            FocusSequenceState::Esc(esc) => queue.push_back(TuiEvent::Key(esc)),
            FocusSequenceState::EscBracket(esc, bracket) => {
                queue.push_back(TuiEvent::Key(esc));
                queue.push_back(TuiEvent::Key(bracket));
            }
            FocusSequenceState::None => {}
        }
        self.deadline = None;
    }

    fn start_deadline(&mut self) {
        self.deadline = Some(TokioInstant::now() + self.timeout);
    }

    fn apply_focus_event(
        &mut self,
        key_event: KeyEvent,
        queue: &mut VecDeque<TuiEvent>,
        terminal_focused: &Arc<AtomicBool>,
    ) {
        let focus_gained = matches!(key_event.code, KeyCode::Char('I'));
        terminal_focused.store(focus_gained, Ordering::Relaxed);
        if focus_gained {
            crate::terminal_palette::requery_default_colors();
            queue.push_back(TuiEvent::Draw);
        }
        self.state = FocusSequenceState::None;
        self.deadline = None;
    }

    fn is_plain_esc(key_event: &KeyEvent) -> bool {
        key_event.code == KeyCode::Esc
            && key_event.modifiers.is_empty()
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
    }

    fn is_left_bracket(key_event: &KeyEvent) -> bool {
        Self::is_char(key_event, '[')
    }

    fn is_focus_tail(key_event: &KeyEvent) -> bool {
        Self::is_char(key_event, 'I') || Self::is_char(key_event, 'O')
    }

    fn is_char(key_event: &KeyEvent, expected: char) -> bool {
        matches!(key_event.code, KeyCode::Char(c) if c == expected)
            && key_event.modifiers.is_empty()
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
    }
}

#[cfg(test)]
mod tests {
    use super::FocusSequenceBuffer;
    use super::FocusSequenceState;
    use super::TuiEvent;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn focus_in_sequence_coalesces_to_draw() {
        let mut buffer = FocusSequenceBuffer::new(Duration::from_millis(30));
        let mut queue = VecDeque::new();
        let focused = Arc::new(AtomicBool::new(false));

        assert!(buffer.handle_key_event(key(KeyCode::Esc), &mut queue, &focused));
        assert!(buffer.handle_key_event(key(KeyCode::Char('[')), &mut queue, &focused));
        assert!(buffer.handle_key_event(key(KeyCode::Char('I')), &mut queue, &focused));

        assert_eq!(focused.load(Ordering::Relaxed), true);
        assert_eq!(queue.pop_front(), Some(TuiEvent::Draw));
        assert!(queue.is_empty());
        assert!(matches!(buffer.state, FocusSequenceState::None));
    }

    #[test]
    fn focus_out_sequence_is_absorbed_without_leaking_keys() {
        let mut buffer = FocusSequenceBuffer::new(Duration::from_millis(30));
        let mut queue = VecDeque::new();
        let focused = Arc::new(AtomicBool::new(true));

        assert!(buffer.handle_key_event(key(KeyCode::Esc), &mut queue, &focused));
        assert!(buffer.handle_key_event(key(KeyCode::Char('[')), &mut queue, &focused));
        assert!(buffer.handle_key_event(key(KeyCode::Char('O')), &mut queue, &focused));

        assert_eq!(focused.load(Ordering::Relaxed), false);
        assert!(queue.is_empty());
        assert!(matches!(buffer.state, FocusSequenceState::None));
    }

    #[test]
    fn mismatched_sequence_flushes_pending_escape() {
        let mut buffer = FocusSequenceBuffer::new(Duration::from_millis(30));
        let mut queue = VecDeque::new();
        let focused = Arc::new(AtomicBool::new(false));

        assert!(buffer.handle_key_event(key(KeyCode::Esc), &mut queue, &focused));
        assert!(!buffer.handle_key_event(key(KeyCode::Char('X')), &mut queue, &focused));

        assert_eq!(queue.pop_front(), Some(TuiEvent::Key(key(KeyCode::Esc))));
        assert!(queue.is_empty());
        assert!(matches!(buffer.state, FocusSequenceState::None));
    }
}
