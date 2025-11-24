//! Windows specific definitions
#![allow(clippy::try_err)] // suggested fix does not work (cannot infer...)

use std::fs::OpenOptions;
use std::io;
use std::mem;
use std::os::windows::io::IntoRawHandle;
use std::ptr;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;

use log::{debug, warn};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use windows_sys::Win32::Foundation::{self as foundation, BOOL, FALSE, HANDLE, TRUE};
use windows_sys::Win32::System::Console as console;
use windows_sys::Win32::System::Threading as threading;
use windows_sys::Win32::UI::Input::KeyboardAndMouse;

use super::{width, Event, RawMode, RawReader, Renderer, Term};
use crate::config::{Behavior, BellStyle, ColorMode, Config};
use crate::highlight::Highlighter;
use crate::keys::{KeyCode as K, KeyEvent, Modifiers as M};
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::{error, Cmd, Result};

fn get_std_handle(fd: console::STD_HANDLE) -> Result<HANDLE> {
    let handle = unsafe { console::GetStdHandle(fd) };
    check_handle(handle)
}

fn check_handle(handle: HANDLE) -> Result<HANDLE> {
    if handle == foundation::INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())?;
    } else if handle == 0 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "no stdio handle available for this process",
        ))?;
    }
    Ok(handle)
}

fn check(rc: BOOL) -> io::Result<()> {
    if rc == FALSE {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn get_win_size(handle: HANDLE) -> (usize, usize) {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { console::GetConsoleScreenBufferInfo(handle, &mut info) } {
        FALSE => (80, 24),
        _ => (
            info.dwSize.X as usize,
            (1 + info.srWindow.Bottom - info.srWindow.Top) as usize,
        ), // (info.srWindow.Right - info.srWindow.Left + 1)
    }
}

fn get_console_mode(handle: HANDLE) -> Result<console::CONSOLE_MODE> {
    let mut original_mode = 0;
    check(unsafe { console::GetConsoleMode(handle, &mut original_mode) })?;
    Ok(original_mode)
}

type ConsoleBuffer = ();
#[cfg(not(test))]
pub type Buffer = ConsoleBuffer;

type ConsoleKeyMap = ();
#[cfg(not(test))]
pub type KeyMap = ConsoleKeyMap;

#[cfg(not(test))]
pub type Mode = ConsoleMode;

#[must_use = "You must restore default mode (disable_raw_mode)"]
#[derive(Clone, Debug)]
pub struct ConsoleMode {
    original_conin_mode: console::CONSOLE_MODE,
    conin: HANDLE,
    original_conout_mode: Option<console::CONSOLE_MODE>,
    conout: HANDLE,
    raw_mode: Arc<AtomicBool>,
}

impl RawMode for ConsoleMode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        check(unsafe { console::SetConsoleMode(self.conin, self.original_conin_mode) })?;
        if let Some(original_stdstream_mode) = self.original_conout_mode {
            check(unsafe { console::SetConsoleMode(self.conout, original_stdstream_mode) })?;
        }
        self.raw_mode.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// Console input reader
pub struct ConsoleRawReader {
    conin: HANDLE,
    // external print reader
    pipe_reader: Option<Rc<AsyncPipe>>,
}

impl ConsoleRawReader {
    fn create(conin: HANDLE, pipe_reader: Option<Rc<AsyncPipe>>) -> ConsoleRawReader {
        ConsoleRawReader { conin, pipe_reader }
    }

    fn select(&mut self) -> Result<Event> {
        use foundation::WAIT_OBJECT_0;
        use threading::{WaitForMultipleObjects, INFINITE};

        let pipe_reader = self.pipe_reader.as_ref().unwrap();
        let handles = [self.conin, pipe_reader.event.0];
        let n = handles.len().try_into().unwrap();
        loop {
            let rc = unsafe { WaitForMultipleObjects(n, handles.as_ptr(), FALSE, INFINITE) };
            if rc == WAIT_OBJECT_0 {
                let mut count = 0;
                check(unsafe { console::GetNumberOfConsoleInputEvents(self.conin, &mut count) })?;
                match read_input(self.conin, count)? {
                    KeyEvent(K::UnknownEscSeq, M::NONE) => continue, // no relevant
                    key => return Ok(Event::KeyPress(key)),
                };
            } else if rc == WAIT_OBJECT_0 + 1 {
                debug!(target: "rustyline", "ExternalPrinter::receive");
                check(unsafe { threading::ResetEvent(pipe_reader.event.0) })?;
                match pipe_reader.receiver.recv() {
                    Ok(msg) => return Ok(Event::ExternalPrint(msg)),
                    Err(e) => Err(io::Error::new(io::ErrorKind::InvalidInput, e))?,
                }
            } else {
                Err(io::Error::last_os_error())?
            }
        }
    }
}

impl RawReader for ConsoleRawReader {
    type Buffer = ConsoleBuffer;

    fn wait_for_input(&mut self, single_esc_abort: bool) -> Result<Event> {
        match self.pipe_reader {
            Some(_) => self.select(),
            None => self.next_key(single_esc_abort).map(Event::KeyPress),
        }
    }

    fn next_key(&mut self, _: bool) -> Result<KeyEvent> {
        read_input(self.conin, u32::MAX)
    }

    fn read_pasted_text(&mut self) -> Result<String> {
        Ok(clipboard_win::get_clipboard_string()?)
    }

    fn find_binding(&self, _: &KeyEvent) -> Option<Cmd> {
        None
    }

    fn unbuffer(self) -> Option<ConsoleBuffer> {
        None
    }
}

fn read_input(handle: HANDLE, max_count: u32) -> Result<KeyEvent> {
    use console::{
        LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED, SHIFT_PRESSED,
    };
    use std::char::decode_utf16;

    let mut rec: console::INPUT_RECORD = unsafe { mem::zeroed() };
    let mut count = 0;
    let mut total = 0;
    let mut surrogate = 0;
    loop {
        if total >= max_count {
            return Ok(KeyEvent(K::UnknownEscSeq, M::NONE));
        }
        // TODO GetNumberOfConsoleInputEvents
        check(unsafe { console::ReadConsoleInputW(handle, &mut rec, 1, &mut count) })?;
        total += count;

        if u32::from(rec.EventType) == console::WINDOW_BUFFER_SIZE_EVENT {
            debug!(target: "rustyline", "SIGWINCH");
            return Err(error::ReadlineError::WindowResized);
        } else if u32::from(rec.EventType) != console::KEY_EVENT {
            continue;
        }
        let key_event = unsafe { rec.Event.KeyEvent };
        // writeln!(io::stderr(), "key_event: {:?}", key_event).unwrap();
        if key_event.bKeyDown == 0 && key_event.wVirtualKeyCode != KeyboardAndMouse::VK_MENU {
            continue;
        }
        // key_event.wRepeatCount seems to be always set to 1 (maybe because we only
        // read one character at a time)

        let alt_gr = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED)
            == (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED);
        let mut mods = M::NONE;
        if !alt_gr && key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0 {
            mods |= M::CTRL;
        }
        if !alt_gr && key_event.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0 {
            mods |= M::ALT;
        }
        if key_event.dwControlKeyState & SHIFT_PRESSED != 0 {
            mods |= M::SHIFT;
        }

        let utf16 = unsafe { key_event.uChar.UnicodeChar };
        let key_code = match key_event.wVirtualKeyCode {
            KeyboardAndMouse::VK_LEFT => K::Left,
            KeyboardAndMouse::VK_RIGHT => K::Right,
            KeyboardAndMouse::VK_UP => K::Up,
            KeyboardAndMouse::VK_DOWN => K::Down,
            KeyboardAndMouse::VK_DELETE => K::Delete,
            KeyboardAndMouse::VK_HOME => K::Home,
            KeyboardAndMouse::VK_END => K::End,
            KeyboardAndMouse::VK_PRIOR => K::PageUp,
            KeyboardAndMouse::VK_NEXT => K::PageDown,
            KeyboardAndMouse::VK_INSERT => K::Insert,
            KeyboardAndMouse::VK_F1 => K::F(1),
            KeyboardAndMouse::VK_F2 => K::F(2),
            KeyboardAndMouse::VK_F3 => K::F(3),
            KeyboardAndMouse::VK_F4 => K::F(4),
            KeyboardAndMouse::VK_F5 => K::F(5),
            KeyboardAndMouse::VK_F6 => K::F(6),
            KeyboardAndMouse::VK_F7 => K::F(7),
            KeyboardAndMouse::VK_F8 => K::F(8),
            KeyboardAndMouse::VK_F9 => K::F(9),
            KeyboardAndMouse::VK_F10 => K::F(10),
            KeyboardAndMouse::VK_F11 => K::F(11),
            KeyboardAndMouse::VK_F12 => K::F(12),
            KeyboardAndMouse::VK_BACK => K::Backspace, // vs Ctrl-h
            KeyboardAndMouse::VK_RETURN => K::Enter,   // vs Ctrl-m
            KeyboardAndMouse::VK_ESCAPE => K::Esc,
            KeyboardAndMouse::VK_TAB => {
                if mods.contains(M::SHIFT) {
                    mods.remove(M::SHIFT);
                    K::BackTab
                } else {
                    K::Tab // vs Ctrl-i
                }
            }
            _ => {
                if utf16 == 0 {
                    continue;
                }
                K::UnknownEscSeq
            }
        };

        let key = if key_code != K::UnknownEscSeq {
            KeyEvent(key_code, mods)
        } else if utf16 == 27 {
            KeyEvent(K::Esc, mods) // FIXME dead code ?
        } else {
            if (0xD800..0xDC00).contains(&utf16) {
                surrogate = utf16;
                continue;
            }
            let orc = if surrogate == 0 {
                decode_utf16(Some(utf16)).next()
            } else {
                decode_utf16([surrogate, utf16].iter().copied()).next()
            };
            let rc = if let Some(rc) = orc {
                rc
            } else {
                return Err(error::ReadlineError::Eof);
            };
            let c = rc?;
            KeyEvent::new(c, mods)
        };
        debug!(target: "rustyline", "wVirtualKeyCode: {:#x}, utf16: {:#x}, dwControlKeyState: {:#x} => key: {:?}", key_event.wVirtualKeyCode, utf16, key_event.dwControlKeyState,key);
        return Ok(key);
    }
}

pub struct ConsoleRenderer {
    conout: HANDLE,
    cols: usize, // Number of columns in terminal
    buffer: String,
    utf16: Vec<u16>,
    colors_enabled: bool,
    bell_style: BellStyle,
}

impl ConsoleRenderer {
    fn new(conout: HANDLE, colors_enabled: bool, bell_style: BellStyle) -> ConsoleRenderer {
        // Multi line editing is enabled by ENABLE_WRAP_AT_EOL_OUTPUT mode
        let (cols, _) = get_win_size(conout);
        ConsoleRenderer {
            conout,
            cols,
            buffer: String::with_capacity(1024),
            utf16: Vec::with_capacity(1024),
            colors_enabled,
            bell_style,
        }
    }

    fn get_console_screen_buffer_info(&self) -> Result<console::CONSOLE_SCREEN_BUFFER_INFO> {
        let mut info = unsafe { mem::zeroed() };
        check(unsafe { console::GetConsoleScreenBufferInfo(self.conout, &mut info) })?;
        Ok(info)
    }

    fn set_console_cursor_position(
        &mut self,
        mut pos: console::COORD,
        size: console::COORD,
    ) -> Result<console::COORD> {
        use std::cmp::{max, min};
        // https://docs.microsoft.com/en-us/windows/console/setconsolecursorposition
        // > The coordinates must be within the boundaries of the console screen buffer.
        // pos.X = max(0, min(size.X - 1, pos.X));
        pos.Y = max(0, min(size.Y - 1, pos.Y));
        check(unsafe { console::SetConsoleCursorPosition(self.conout, pos) })?;
        Ok(pos)
    }

    fn clear(&mut self, length: u32, pos: console::COORD, attr: u16) -> Result<()> {
        let mut _count = 0;
        check(unsafe {
            console::FillConsoleOutputCharacterA(self.conout, b' ', length, pos, &mut _count)
        })?;
        Ok(check(unsafe {
            console::FillConsoleOutputAttribute(self.conout, attr, length, pos, &mut _count)
        })?)
    }

    fn set_cursor_visibility(&mut self, visible: bool) -> Result<Option<ConsoleCursorGuard>> {
        set_cursor_visibility(self.conout, visible)
    }

    // You can't have both ENABLE_WRAP_AT_EOL_OUTPUT and
    // ENABLE_VIRTUAL_TERMINAL_PROCESSING. So we need to wrap manually.
    fn wrap_at_eol(&mut self, s: &str, mut col: usize) -> usize {
        let mut esc_seq = 0;
        for c in s.graphemes(true) {
            if c == "\n" {
                col = 0;
            } else {
                let cw = width(c, &mut esc_seq);
                col += cw;
                if col > self.cols {
                    self.buffer.push('\n');
                    col = cw;
                }
            }
            self.buffer.push_str(c);
        }
        if col == self.cols {
            self.buffer.push('\n');
            col = 0;
        }
        col
    }

    // position at the start of the prompt, clear to end of previous input
    fn clear_old_rows(
        &mut self,
        info: &console::CONSOLE_SCREEN_BUFFER_INFO,
        layout: &Layout,
    ) -> Result<()> {
        let current_row = layout.cursor.row;
        let old_rows = layout.end.row;
        let mut coord = info.dwCursorPosition;
        coord.X = 0;
        coord.Y -= current_row as i16;
        let coord = self.set_console_cursor_position(coord, info.dwSize)?;
        self.clear(
            (info.dwSize.X * (old_rows as i16 + 1)) as u32,
            coord,
            info.wAttributes,
        )
    }
}

pub struct ConsoleCursorGuard(HANDLE);

impl Drop for ConsoleCursorGuard {
    fn drop(&mut self) {
        let _ = set_cursor_visibility(self.0, true);
    }
}

fn set_cursor_visibility(handle: HANDLE, visible: bool) -> Result<Option<ConsoleCursorGuard>> {
    let mut info = unsafe { mem::zeroed() };
    check(unsafe { console::GetConsoleCursorInfo(handle, &mut info) })?;
    let b = if visible { TRUE } else { FALSE };
    if info.bVisible == b {
        return Ok(None);
    }
    info.bVisible = b;
    check(unsafe { console::SetConsoleCursorInfo(handle, &info) })?;
    Ok(if visible {
        None
    } else {
        Some(ConsoleCursorGuard(handle))
    })
}

impl Renderer for ConsoleRenderer {
    type Reader = ConsoleRawReader;

    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        let info = self.get_console_screen_buffer_info()?;
        let mut cursor = info.dwCursorPosition;
        if new.row > old.row {
            cursor.Y += (new.row - old.row) as i16;
        } else {
            cursor.Y -= (old.row - new.row) as i16;
        }
        if new.col > old.col {
            cursor.X += (new.col - old.col) as i16;
        } else {
            cursor.X -= (old.col - new.col) as i16;
        }
        self.set_console_cursor_position(cursor, info.dwSize)
            .map(|_| ())
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        let default_prompt = new_layout.default_prompt;
        let cursor = new_layout.cursor;
        let end_pos = new_layout.end;

        self.buffer.clear();
        let mut col = 0;
        if let Some(highlighter) = highlighter {
            // TODO handle ansi escape code (SetConsoleTextAttribute)
            // append the prompt
            col = self.wrap_at_eol(&highlighter.highlight_prompt(prompt, default_prompt), col);
            // append the input line
            col = self.wrap_at_eol(&highlighter.highlight(line, line.pos()), col);
        } else if self.colors_enabled {
            // append the prompt
            col = self.wrap_at_eol(prompt, col);
            // append the input line
            col = self.wrap_at_eol(line, col);
        } else {
            // append the prompt
            self.buffer.push_str(prompt);
            // append the input line
            self.buffer.push_str(line);
        }
        // append hint
        if let Some(hint) = hint {
            if let Some(highlighter) = highlighter {
                self.wrap_at_eol(&highlighter.highlight_hint(hint), col);
            } else if self.colors_enabled {
                self.wrap_at_eol(hint, col);
            } else {
                self.buffer.push_str(hint);
            }
        }
        let info = self.get_console_screen_buffer_info()?;
        // just to avoid flickering
        let mut guard = self.set_cursor_visibility(false)?;
        // position at the start of the prompt, clear to end of previous input
        self.clear_old_rows(&info, old_layout)?;
        // display prompt, input line and hint
        write_to_console(self.conout, self.buffer.as_str(), &mut self.utf16)?;

        // position the cursor
        let info = self.get_console_screen_buffer_info()?;
        let mut coord = info.dwCursorPosition;
        coord.X = cursor.col as i16;
        coord.Y -= (end_pos.row - cursor.row) as i16;
        self.set_console_cursor_position(coord, info.dwSize)?;
        guard.take();
        Ok(())
    }

    fn write_and_flush(&mut self, buf: &str) -> Result<()> {
        write_to_console(self.conout, buf, &mut self.utf16)
    }

    /// Characters with 2 column width are correctly handled (not split).
    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        for c in s.graphemes(true) {
            if c == "\n" {
                pos.col = 0;
                pos.row += 1;
            } else {
                let cw = c.width();
                pos.col += cw;
                if pos.col > self.cols {
                    pos.row += 1;
                    pos.col = cw;
                }
            }
        }
        if pos.col == self.cols {
            pos.col = 0;
            pos.row += 1;
        }
        pos
    }

    fn beep(&mut self) -> Result<()> {
        match self.bell_style {
            BellStyle::Audible => write_all(self.conout, &[7; 1]),
            _ => Ok(()),
        }
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()> {
        let info = self.get_console_screen_buffer_info()?;
        let coord = console::COORD { X: 0, Y: 0 };
        check(unsafe { console::SetConsoleCursorPosition(self.conout, coord) })?;
        let n = info.dwSize.X as u32 * info.dwSize.Y as u32;
        self.clear(n, coord, info.wAttributes)
    }

    fn clear_rows(&mut self, layout: &Layout) -> Result<()> {
        let info = self.get_console_screen_buffer_info()?;
        self.clear_old_rows(&info, layout)
    }

    /// Try to get the number of columns in the current terminal,
    /// or assume 80 if it fails.
    fn update_size(&mut self) {
        let (cols, _) = get_win_size(self.conout);
        self.cols = cols;
    }

    fn get_columns(&self) -> usize {
        self.cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size(self.conout);
        rows
    }

    fn colors_enabled(&self) -> bool {
        self.colors_enabled
    }

    fn move_cursor_at_leftmost(&mut self, _: &mut ConsoleRawReader) -> Result<()> {
        let info = self.get_console_screen_buffer_info()?;
        let mut cursor = info.dwCursorPosition;
        if cursor.X == 0 {
            return Ok(());
        }
        debug!(target: "rustyline", "initial cursor location: {:?}, {:?}", cursor.X, cursor.Y);
        cursor.X = 0;
        cursor.Y += 1;
        let res = self.set_console_cursor_position(cursor, info.dwSize);
        if let Err(error::ReadlineError::Io(ref e)) = res {
            if e.raw_os_error() == Some(foundation::ERROR_INVALID_PARAMETER as i32) {
                warn!(target: "rustyline", "invalid cursor position: ({:?}, {:?}) in ({:?}, {:?})", cursor.X, cursor.Y, info.dwSize.X, info.dwSize.Y);
                write_all(self.conout, &[10; 1])?;
                return Ok(());
            }
        }
        res.map(|_| ())
    }
}

fn write_to_console(handle: HANDLE, s: &str, utf16: &mut Vec<u16>) -> Result<()> {
    utf16.clear();
    utf16.extend(s.encode_utf16());
    write_all(handle, utf16.as_slice())
}

// See write_valid_utf8_to_console
// /src/rust/library/std/src/sys/windows/stdio.rs:171
fn write_all(handle: HANDLE, mut data: &[u16]) -> Result<()> {
    use std::io::{Error, ErrorKind};
    while !data.is_empty() {
        let slice = if data.len() < 8192 {
            data
        } else if (0xD800..0xDC00).contains(&data[8191]) {
            &data[..8191]
        } else {
            &data[..8192]
        };
        let mut written = 0;
        check(unsafe {
            console::WriteConsoleW(
                handle,
                slice.as_ptr().cast::<std::ffi::c_void>(),
                slice.len() as u32,
                &mut written,
                ptr::null_mut(),
            )
        })?;
        if written == 0 {
            Err(Error::new(ErrorKind::WriteZero, "WriteConsoleW"))?;
        }
        data = &data[(written as usize)..];
    }
    Ok(())
}

#[cfg(not(test))]
pub type Terminal = Console;

#[derive(Clone, Debug)]
pub struct Console {
    conin_isatty: bool,
    conin: HANDLE,
    conout_isatty: bool,
    conout: HANDLE,
    close_on_drop: bool,
    pub(crate) color_mode: ColorMode,
    ansi_colors_supported: bool,
    bell_style: BellStyle,
    raw_mode: Arc<AtomicBool>,
    // external print reader
    pipe_reader: Option<Rc<AsyncPipe>>,
    // external print writer
    pipe_writer: Option<SyncSender<String>>,
}

impl Console {
    fn colors_enabled(&self) -> bool {
        // TODO ANSI Colors & Windows <10
        match self.color_mode {
            ColorMode::Enabled => self.conout_isatty && self.ansi_colors_supported,
            ColorMode::Forced => true,
            ColorMode::Disabled => false,
        }
    }
}

impl Term for Console {
    type Buffer = ConsoleBuffer;
    type CursorGuard = ConsoleCursorGuard;
    type ExternalPrinter = ExternalPrinter;
    type KeyMap = ConsoleKeyMap;
    type Mode = ConsoleMode;
    type Reader = ConsoleRawReader;
    type Writer = ConsoleRenderer;

    fn new(
        color_mode: ColorMode,
        behavior: Behavior,
        _tab_stop: usize,
        bell_style: BellStyle,
        _enable_bracketed_paste: bool,
        _enable_signals: bool,
    ) -> Result<Console> {
        let (conin, conout, close_on_drop) = if behavior == Behavior::PreferTerm {
            if let (Ok(conin), Ok(conout)) = (
                OpenOptions::new().read(true).write(true).open("CONIN$"),
                OpenOptions::new().read(true).write(true).open("CONOUT$"),
            ) {
                (
                    Ok(conin.into_raw_handle() as HANDLE),
                    Ok(conout.into_raw_handle() as HANDLE),
                    true,
                )
            } else {
                (
                    get_std_handle(console::STD_INPUT_HANDLE),
                    get_std_handle(console::STD_OUTPUT_HANDLE),
                    false,
                )
            }
        } else {
            (
                get_std_handle(console::STD_INPUT_HANDLE),
                get_std_handle(console::STD_OUTPUT_HANDLE),
                false,
            )
        };
        let conin_isatty = match conin {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };

        let conout_isatty = match conout {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };

        Ok(Console {
            conin_isatty,
            conin: conin.unwrap_or(0),
            conout_isatty,
            conout: conout.unwrap_or(0),
            close_on_drop,
            color_mode,
            ansi_colors_supported: false,
            bell_style,
            raw_mode: Arc::new(AtomicBool::new(false)),
            pipe_reader: None,
            pipe_writer: None,
        })
    }

    /// Checking for an unsupported TERM in windows is a no-op
    fn is_unsupported(&self) -> bool {
        false
    }

    fn is_input_tty(&self) -> bool {
        self.conin_isatty
    }

    fn is_output_tty(&self) -> bool {
        self.conout_isatty
    }

    // pub fn install_sigwinch_handler(&mut self) {
    // See ReadConsoleInputW && WINDOW_BUFFER_SIZE_EVENT
    // }

    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&mut self) -> Result<(ConsoleMode, ConsoleKeyMap)> {
        if !self.conin_isatty {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "no stdio handle available for this process",
            ))?;
        }
        let original_conin_mode = get_console_mode(self.conin)?;
        // Disable these modes
        let mut raw = original_conin_mode
            & !(console::ENABLE_LINE_INPUT
                | console::ENABLE_ECHO_INPUT
                | console::ENABLE_PROCESSED_INPUT);
        // Enable these modes
        raw |= console::ENABLE_EXTENDED_FLAGS;
        raw |= console::ENABLE_INSERT_MODE;
        raw |= console::ENABLE_QUICK_EDIT_MODE;
        raw |= console::ENABLE_WINDOW_INPUT;
        check(unsafe { console::SetConsoleMode(self.conin, raw) })?;

        let original_conout_mode = if self.conout_isatty {
            let original_conout_mode = get_console_mode(self.conout)?;

            let mut mode = original_conout_mode;
            if mode & console::ENABLE_WRAP_AT_EOL_OUTPUT == 0 {
                mode |= console::ENABLE_WRAP_AT_EOL_OUTPUT;
                debug!(target: "rustyline", "activate ENABLE_WRAP_AT_EOL_OUTPUT");
                unsafe {
                    assert_ne!(console::SetConsoleMode(self.conout, mode), 0);
                }
            }
            // To enable ANSI colors (Windows 10 only):
            // https://docs.microsoft.com/en-us/windows/console/setconsolemode
            self.ansi_colors_supported = mode & console::ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0;
            if self.ansi_colors_supported {
                if self.color_mode == ColorMode::Disabled {
                    mode &= !console::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                    debug!(target: "rustyline", "deactivate ENABLE_VIRTUAL_TERMINAL_PROCESSING");
                    unsafe {
                        assert_ne!(console::SetConsoleMode(self.conout, mode), 0);
                    }
                } else {
                    debug!(target: "rustyline", "ANSI colors already enabled");
                }
            } else if self.color_mode != ColorMode::Disabled {
                mode |= console::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                self.ansi_colors_supported =
                    unsafe { console::SetConsoleMode(self.conout, mode) != 0 };
                debug!(target: "rustyline", "ansi_colors_supported: {}", self.ansi_colors_supported);
            }
            Some(original_conout_mode)
        } else {
            None
        };

        self.raw_mode.store(true, Ordering::SeqCst);
        // when all ExternalPrinter are dropped there is no need to use `pipe_reader`
        if Arc::strong_count(&self.raw_mode) == 1 {
            self.pipe_writer = None;
            self.pipe_reader = None;
        }

        Ok((
            ConsoleMode {
                original_conin_mode,
                conin: self.conin,
                original_conout_mode,
                conout: self.conout,
                raw_mode: self.raw_mode.clone(),
            },
            (),
        ))
    }

    fn create_reader(
        &self,
        _: Option<ConsoleBuffer>,
        _: &Config,
        _: ConsoleKeyMap,
    ) -> ConsoleRawReader {
        ConsoleRawReader::create(self.conin, self.pipe_reader.clone())
    }

    fn create_writer(&self) -> ConsoleRenderer {
        ConsoleRenderer::new(self.conout, self.colors_enabled(), self.bell_style)
    }

    fn writeln(&self) -> Result<()> {
        write_all(self.conout, &[10; 1])
    }

    fn create_external_printer(&mut self) -> Result<ExternalPrinter> {
        if let Some(ref sender) = self.pipe_writer {
            return Ok(ExternalPrinter {
                event: self.pipe_reader.as_ref().unwrap().event.0,
                sender: sender.clone(),
                raw_mode: self.raw_mode.clone(),
                conout: self.conout,
            });
        }
        if !self.is_input_tty() || !self.is_output_tty() {
            Err(io::Error::from(io::ErrorKind::Other))?; // FIXME
        }
        let event = unsafe { threading::CreateEventW(ptr::null_mut(), TRUE, FALSE, ptr::null()) };
        if event == 0 {
            Err(io::Error::last_os_error())?;
        }
        let (sender, receiver) = sync_channel(1);

        let reader = Rc::new(AsyncPipe {
            event: Handle(event),
            receiver,
        });
        self.pipe_reader.replace(reader);
        self.pipe_writer.replace(sender.clone());
        Ok(ExternalPrinter {
            event,
            sender,
            raw_mode: self.raw_mode.clone(),
            conout: self.conout,
        })
    }

    fn set_cursor_visibility(&mut self, visible: bool) -> Result<Option<ConsoleCursorGuard>> {
        if self.conout_isatty {
            set_cursor_visibility(self.conout, visible)
        } else {
            Ok(None)
        }
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        if self.close_on_drop {
            unsafe { foundation::CloseHandle(self.conin) };
            unsafe { foundation::CloseHandle(self.conout) };
        }
    }
}

unsafe impl Send for Console {}
unsafe impl Sync for Console {}

#[derive(Debug)]
struct AsyncPipe {
    event: Handle,
    receiver: Receiver<String>,
}

#[derive(Debug)]
pub struct ExternalPrinter {
    event: HANDLE,
    sender: SyncSender<String>,
    raw_mode: Arc<AtomicBool>,
    conout: HANDLE,
}

unsafe impl Send for ExternalPrinter {}
unsafe impl Sync for ExternalPrinter {}

impl super::ExternalPrinter for ExternalPrinter {
    fn print(&mut self, msg: String) -> Result<()> {
        // write directly to stdout/stderr while not in raw mode
        if !self.raw_mode.load(Ordering::SeqCst) {
            let mut utf16 = Vec::new();
            write_to_console(self.conout, msg.as_str(), &mut utf16)
        } else {
            self.sender
                .send(msg)
                .map_err(|_| io::Error::from(io::ErrorKind::Other))?; // FIXME
            Ok(check(unsafe { threading::SetEvent(self.event) })?)
        }
    }
}

#[derive(Debug)]
struct Handle(HANDLE);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { foundation::CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod test {
    use super::Console;

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Console>();
    }

    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Console>();
    }
}
