//! Tests specific definitions
use std::slice::Iter;
use std::vec::IntoIter;

use super::{Event, ExternalPrinter, RawMode, RawReader, Renderer, Term};
use crate::config::{Behavior, BellStyle, ColorMode, Config};
use crate::error::ReadlineError;
use crate::highlight::Highlighter;
use crate::keys::KeyEvent;
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::{Cmd, Result};

pub type Buffer = ();
pub type KeyMap = ();
pub type Mode = ();

impl RawMode for Mode {
    fn disable_raw_mode(&self) -> Result<()> {
        Ok(())
    }
}

impl<'a> RawReader for Iter<'a, KeyEvent> {
    type Buffer = Buffer;

    fn wait_for_input(&mut self, single_esc_abort: bool) -> Result<Event> {
        self.next_key(single_esc_abort).map(Event::KeyPress)
    }

    fn next_key(&mut self, _: bool) -> Result<KeyEvent> {
        match self.next() {
            Some(key) => Ok(*key),
            None => Err(ReadlineError::Eof),
        }
    }

    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char> {
        unimplemented!();
    }

    fn read_pasted_text(&mut self) -> Result<String> {
        unimplemented!()
    }

    fn find_binding(&self, _: &KeyEvent) -> Option<Cmd> {
        None
    }

    fn unbuffer(self) -> Option<Buffer> {
        None
    }
}

impl RawReader for IntoIter<KeyEvent> {
    type Buffer = Buffer;

    fn wait_for_input(&mut self, single_esc_abort: bool) -> Result<Event> {
        self.next_key(single_esc_abort).map(Event::KeyPress)
    }

    fn next_key(&mut self, _: bool) -> Result<KeyEvent> {
        match self.next() {
            Some(key) => Ok(key),
            None => Err(ReadlineError::Eof),
        }
    }

    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char> {
        use crate::keys::{KeyCode as K, KeyEvent as E, Modifiers as M};
        match self.next() {
            Some(E(K::Char(c), M::NONE)) => Ok(c),
            None => Err(ReadlineError::Eof),
            _ => unimplemented!(),
        }
    }

    fn read_pasted_text(&mut self) -> Result<String> {
        unimplemented!()
    }

    fn find_binding(&self, _: &KeyEvent) -> Option<Cmd> {
        None
    }

    fn unbuffer(self) -> Option<Buffer> {
        None
    }
}

#[derive(Default)]
pub struct Sink {}

impl Renderer for Sink {
    type Reader = IntoIter<KeyEvent>;

    fn move_cursor(&mut self, _: Position, _: Position) -> Result<()> {
        Ok(())
    }

    fn refresh_line(
        &mut self,
        _prompt: &str,
        _line: &LineBuffer,
        _hint: Option<&str>,
        _old_layout: &Layout,
        _new_layout: &Layout,
        _highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        Ok(())
    }

    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        pos.col += s.len();
        pos
    }

    fn write_and_flush(&mut self, _: &str) -> Result<()> {
        Ok(())
    }

    fn beep(&mut self) -> Result<()> {
        Ok(())
    }

    fn clear_screen(&mut self) -> Result<()> {
        Ok(())
    }

    fn clear_rows(&mut self, _: &Layout) -> Result<()> {
        Ok(())
    }

    fn update_size(&mut self) {}

    fn get_columns(&self) -> usize {
        80
    }

    fn get_rows(&self) -> usize {
        24
    }

    fn colors_enabled(&self) -> bool {
        false
    }

    fn move_cursor_at_leftmost(&mut self, _: &mut IntoIter<KeyEvent>) -> Result<()> {
        Ok(())
    }
}

pub struct DummyExternalPrinter {}

impl ExternalPrinter for DummyExternalPrinter {
    fn print(&mut self, _msg: String) -> Result<()> {
        Ok(())
    }
}

pub type Terminal = DummyTerminal;

#[derive(Clone, Debug)]
pub struct DummyTerminal {
    pub keys: Vec<KeyEvent>,
    pub cursor: usize, // cursor position before last command
    pub color_mode: ColorMode,
    pub bell_style: BellStyle,
}

impl Term for DummyTerminal {
    type Buffer = Buffer;
    type CursorGuard = ();
    type ExternalPrinter = DummyExternalPrinter;
    type KeyMap = KeyMap;
    type Mode = Mode;
    type Reader = IntoIter<KeyEvent>;
    type Writer = Sink;

    fn new(
        color_mode: ColorMode,
        _behavior: Behavior,
        _tab_stop: usize,
        bell_style: BellStyle,
        _enable_bracketed_paste: bool,
        _enable_signals: bool,
    ) -> Result<DummyTerminal> {
        Ok(DummyTerminal {
            keys: Vec::new(),
            cursor: 0,
            color_mode,
            bell_style,
        })
    }

    // Init checks:

    #[cfg(not(target_arch = "wasm32"))]
    fn is_unsupported(&self) -> bool {
        false
    }

    #[cfg(target_arch = "wasm32")]
    fn is_unsupported(&self) -> bool {
        true
    }

    fn is_input_tty(&self) -> bool {
        true
    }

    fn is_output_tty(&self) -> bool {
        false
    }

    // Interactive loop:

    fn enable_raw_mode(&mut self) -> Result<(Mode, KeyMap)> {
        Ok(((), ()))
    }

    fn create_reader(&self, _: Option<Buffer>, _: &Config, _: KeyMap) -> Self::Reader {
        self.keys.clone().into_iter()
    }

    fn create_writer(&self) -> Sink {
        Sink::default()
    }

    fn create_external_printer(&mut self) -> Result<DummyExternalPrinter> {
        Ok(DummyExternalPrinter {})
    }

    fn set_cursor_visibility(&mut self, _: bool) -> Result<Option<()>> {
        Ok(None)
    }

    fn writeln(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    Ok(())
}
