//! Link and divider hover own the mouse shape only while Codex captures pointer input.
//! Remember motion across repaints, but discard coordinates across focus and geometry changes.
//! Cleanup is independent of Tui so panic, suspend, and editor handoffs also restore the shape.
//! Kitty releases its override; Ghostty retains its existing text-cursor fallback.

use std::io::Write;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

use codex_terminal_detection::TerminalName;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Position;

use super::Tui;
use super::TuiEvent;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PointerShape {
    #[default]
    Default = 1,
    Link,
    Resize,
}

#[derive(Default)]
pub(crate) struct LinkHover {
    pub(crate) mouse: Option<MouseEvent>,
    pub(crate) shape: PointerShape,
}

impl LinkHover {
    pub(crate) fn position(&self) -> Option<Position> {
        self.mouse.map(|mouse| (mouse.column, mouse.row).into())
    }

    pub(crate) fn observe(&mut self, event: &TuiEvent) {
        match event {
            TuiEvent::Mouse(mouse) => {
                self.mouse = (!matches!(
                    mouse.kind,
                    MouseEventKind::Down(_) | MouseEventKind::Drag(_)
                ))
                .then_some(*mouse);
            }
            TuiEvent::Key(key) => {
                if let Some(mouse) = &mut self.mouse {
                    mouse.modifiers = key.modifiers;
                }
            }
            TuiEvent::FocusLost | TuiEvent::Resize(_) | TuiEvent::Resume => self.mouse = None,
            _ => {}
        }
    }
}

// Zero means untouched/restored. A failed write leaves the previous shape unknown.
// Keep cleanup armed until flush succeeds.
#[derive(Default)]
pub(super) struct LinkPointer(AtomicU8);

impl LinkPointer {
    pub(super) const fn new() -> Self {
        Self(AtomicU8::new(/*v*/ 0))
    }

    pub(super) fn update(
        &self,
        writer: &mut impl Write,
        shape: PointerShape,
    ) -> std::io::Result<()> {
        let previous = self.0.load(Ordering::Relaxed);
        let next = shape as u8;
        if previous == next || (previous == 0 && shape == PointerShape::Default) {
            return Ok(());
        }
        self.0.store(next, Ordering::Relaxed);
        let result = writer
            .write_all(match shape {
                PointerShape::Default => b"\x1b]22;default\x1b\\",
                PointerShape::Link => b"\x1b]22;pointer\x1b\\",
                PointerShape::Resize => b"\x1b]22;ew-resize\x1b\\",
            })
            .and_then(|()| writer.flush());
        if result.is_err() {
            self.0.store(u8::MAX, Ordering::Relaxed);
        }
        result
    }

    pub(super) fn restore(
        &self,
        writer: &mut impl Write,
        terminal: TerminalName,
    ) -> std::io::Result<()> {
        if self.0.load(Ordering::Relaxed) != 0 {
            // In Kitty, a named shape persists across mouse capture and screen changes.
            // An empty OSC 22 releases that override back to the terminal's default policy.
            writer.write_all(if terminal == TerminalName::Kitty {
                b"\x1b]22;\x1b\\"
            } else {
                b"\x1b]22;text\x1b\\"
            })?;
            writer.flush()?;
            self.0.store(/*val*/ 0, Ordering::Relaxed);
        }
        Ok(())
    }
}

impl Tui {
    pub(crate) fn set_pointer_shape(&mut self, shape: PointerShape) -> std::io::Result<()> {
        // OSC 22 names differ between terminals. Restrict CSS names to known implementations;
        // multiplexers may consume OSC 22 and do not promise pointer-shape passthrough.
        let info = codex_terminal_detection::terminal_info();
        if info.multiplexer.is_none()
            && matches!(info.name, TerminalName::Ghostty | TerminalName::Kitty)
        {
            super::alternate_screen::ALTERNATE_SCREEN
                .set_pointer_shape(self.terminal.backend_mut(), shape)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "link_pointer_tests.rs"]
mod tests;
