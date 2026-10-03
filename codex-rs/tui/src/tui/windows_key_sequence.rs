//! Decode Windows Terminal's explicit CSI-u Shift+Enter mapping from console key events.
//!
//! Crossterm's Win32 backend delivers `sendInput` as individual keys. Only the complete
//! ESC-prefixed sequence is recognized; plain text and paste events are never scanned.
//! A fixed deadline bounds waiting for input; already-ready keys may complete a candidate.
//! Failed prefixes are replayed unchanged.
//! State belongs to the event source so dropping it on terminal handoff drops partial input.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use tokio::time::Sleep;
use tokio_stream::Stream;

use super::event_stream::EventResult;

const SEQUENCE: [KeyCode; 7] = [
    KeyCode::Esc,
    KeyCode::Char('['),
    KeyCode::Char('1'),
    KeyCode::Char('3'),
    KeyCode::Char(';'),
    KeyCode::Char('2'),
    KeyCode::Char('u'),
];
const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(/*millis*/ 50);

pub(super) struct WindowsKeySequence<S> {
    inner: S,
    // At most one press and one release per sequence character.
    pending: Vec<KeyEvent>,
    next: usize,
    deadline: Option<Pin<Box<Sleep>>>,
    ready: VecDeque<EventResult>,
    final_release: Option<KeyEvent>,
}

impl<S> WindowsKeySequence<S> {
    pub(super) fn new(inner: S) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            next: 0,
            deadline: None,
            ready: VecDeque::new(),
            final_release: None,
        }
    }

    fn flush(&mut self) {
        self.ready
            .extend(self.pending.drain(..).map(|key| Ok(Event::Key(key))));
        self.next = 0;
        self.deadline = None;
    }

    fn accept(&mut self, event: Event) {
        if let Some(release) = self.final_release.take()
            && event == Event::Key(release)
        {
            return;
        }
        if let Event::Key(key) = event {
            if let Some(previous) = self.pending.last()
                && previous.kind == KeyEventKind::Press
                && key
                    == (KeyEvent {
                        kind: KeyEventKind::Release,
                        ..*previous
                    })
            {
                self.pending.push(key);
                return;
            }
            if key.code == SEQUENCE[self.next]
                && key.modifiers.is_empty()
                && key.kind == KeyEventKind::Press
            {
                if self.pending.is_empty() {
                    self.deadline = Some(Box::pin(tokio::time::sleep(SEQUENCE_TIMEOUT)));
                }
                self.pending.push(key);
                self.next += 1;
                if self.next == SEQUENCE.len() {
                    self.pending.clear();
                    self.next = 0;
                    self.deadline = None;
                    self.final_release = Some(KeyEvent {
                        kind: KeyEventKind::Release,
                        ..key
                    });
                    self.ready.push_back(Ok(Event::Key(KeyEvent::new(
                        KeyCode::Enter,
                        KeyModifiers::SHIFT,
                    ))));
                }
                return;
            }
        }
        self.flush();
        // A mismatching Escape may start the next sequence after replaying the failed one.
        if matches!(event, Event::Key(key) if key == KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        {
            self.accept(event);
        } else {
            self.ready.push_back(Ok(event));
        }
    }
}

impl<S: Stream<Item = EventResult> + Unpin> Stream for WindowsKeySequence<S> {
    type Item = EventResult;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(event) = this.ready.pop_front() {
                return Poll::Ready(Some(event));
            }
            // Each ready key advances the finite candidate or produces output, so this
            // cannot drain indefinitely. Prefer queued input over an elapsed deadline.
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => this.accept(event),
                Poll::Ready(Some(Err(error))) => {
                    this.flush();
                    this.ready.push_back(Err(error));
                }
                Poll::Ready(None) => {
                    this.flush();
                    return Poll::Ready(this.ready.pop_front());
                }
                Poll::Pending => {
                    if let Some(deadline) = &mut this.deadline
                        && deadline.as_mut().poll(cx).is_ready()
                    {
                        this.flush();
                        continue;
                    }
                    return Poll::Pending;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "windows_key_sequence_tests.rs"]
mod tests;
