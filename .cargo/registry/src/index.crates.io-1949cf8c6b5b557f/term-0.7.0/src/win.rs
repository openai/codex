// Copyright 2013-2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Windows console handling

// FIXME (#13400): this is only a tiny fraction of the Windows console api

use crate::color;
use crate::Attr;
use crate::Error;
use crate::Result;
use crate::Terminal;
use std::io;
use std::io::prelude::*;
use std::ops::Deref;
use std::ptr;

use winapi::shared::minwindef::{DWORD, WORD};
use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
use winapi::um::fileapi::{CreateFileA, OPEN_EXISTING};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::wincon::FillConsoleOutputAttribute;
use winapi::um::wincon::{
    FillConsoleOutputCharacterW, GetConsoleScreenBufferInfo, CONSOLE_SCREEN_BUFFER_INFO, COORD,
};
use winapi::um::wincon::{SetConsoleCursorPosition, SetConsoleTextAttribute};
use winapi::um::wincon::{BACKGROUND_INTENSITY, ENABLE_VIRTUAL_TERMINAL_PROCESSING};
use winapi::um::winnt::{FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE, HANDLE};

/// Console info which can be used by a Terminal implementation
/// which uses the Win32 Console API.
pub struct WinConsoleInfo {
    def_foreground: color::Color,
    def_background: color::Color,
    foreground: color::Color,
    background: color::Color,
    reverse: bool,
    secure: bool,
    standout: bool,
}

/// A Terminal implementation which uses the Win32 Console API.
pub struct WinConsole<T> {
    buf: T,
    info: WinConsoleInfo,
}

fn color_to_bits(color: color::Color) -> u16 {
    // magic numbers from mingw-w64's wincon.h

    let bits = match color % 8 {
        color::BLACK => 0,
        color::BLUE => 0x1,
        color::GREEN => 0x2,
        color::RED => 0x4,
        color::YELLOW => 0x2 | 0x4,
        color::MAGENTA => 0x1 | 0x4,
        color::CYAN => 0x1 | 0x2,
        color::WHITE => 0x1 | 0x2 | 0x4,
        _ => unreachable!(),
    };

    if color >= 8 {
        bits | 0x8
    } else {
        bits
    }
}

fn bits_to_color(bits: u16) -> color::Color {
    let color = match bits & 0x7 {
        0 => color::BLACK,
        0x1 => color::BLUE,
        0x2 => color::GREEN,
        0x4 => color::RED,
        0x6 => color::YELLOW,
        0x5 => color::MAGENTA,
        0x3 => color::CYAN,
        0x7 => color::WHITE,
        _ => unreachable!(),
    };

    color | (bits as u32 & 0x8) // copy the hi-intensity bit
}

struct HandleWrapper {
    inner: HANDLE,
}

impl HandleWrapper {
    fn new(h: HANDLE) -> HandleWrapper {
        HandleWrapper { inner: h }
    }
}

impl Drop for HandleWrapper {
    fn drop(&mut self) {
        if self.inner != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.inner);
            }
        }
    }
}

impl Deref for HandleWrapper {
    type Target = HANDLE;
    fn deref(&self) -> &HANDLE {
        &self.inner
    }
}

/// Just get a handle to the current console buffer whatever it is
fn conout() -> io::Result<HandleWrapper> {
    let name = b"CONOUT$\0";
    let handle = unsafe {
        CreateFileA(
            name.as_ptr() as *const i8,
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(HandleWrapper::new(handle))
    }
}

unsafe fn set_flag(handle: HANDLE, flag: DWORD) -> io::Result<()> {
    let mut curr_mode: DWORD = 0;
    if GetConsoleMode(handle, &mut curr_mode) == 0 {
        return Err(io::Error::last_os_error());
    }

    if SetConsoleMode(handle, curr_mode | flag) == 0 {
        return Err(io::Error::last_os_error());
    }
    return Ok(());
}

/// Check if console supports ansi codes (should succeed on Windows 10)
pub fn supports_ansi() -> bool {
    conout()
        .and_then(|handle| unsafe { set_flag(*handle, ENABLE_VIRTUAL_TERMINAL_PROCESSING) })
        .is_ok()
}

// This test will only pass if it is running in an actual console, probably
#[test]
fn test_conout() {
    assert!(conout().is_ok())
}

#[rustversion::before(1.36)]
unsafe fn get_console_screen_buffer_info(handle: HANDLE) -> io::Result<CONSOLE_SCREEN_BUFFER_INFO> {
    let mut buffer_info = ::std::mem::uninitialized();
    if GetConsoleScreenBufferInfo(handle, &mut buffer_info) == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(buffer_info)
    }
}
#[rustversion::since(1.36)]
unsafe fn get_console_screen_buffer_info(handle: HANDLE) -> io::Result<CONSOLE_SCREEN_BUFFER_INFO> {
    let mut buffer_info = ::std::mem::MaybeUninit::uninit();
    if GetConsoleScreenBufferInfo(handle, buffer_info.as_mut_ptr()) == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(buffer_info.assume_init())
    }
}

// This test will only pass if it is running in an actual console, probably
#[test]
fn test_get_console_screen_buffer_info() {
    let handle = conout().unwrap();
    unsafe {
        let buffer_info = get_console_screen_buffer_info(*handle);
        assert!(buffer_info.is_ok());
    }
}

impl WinConsoleInfo {
    /// Returns `Err` whenever console info cannot be retrieved for some
    /// reason.
    pub fn from_env() -> io::Result<WinConsoleInfo> {
        let fg;
        let bg;
        let handle = conout()?;
        unsafe {
            let buffer_info = get_console_screen_buffer_info(*handle)?;
            fg = bits_to_color(buffer_info.wAttributes);
            bg = bits_to_color(buffer_info.wAttributes >> 4);
        }
        Ok(WinConsoleInfo {
            def_foreground: fg,
            def_background: bg,
            foreground: fg,
            background: bg,
            reverse: false,
            secure: false,
            standout: false,
        })
    }
}

impl<T: Write + Send> WinConsole<T> {
    fn apply(&mut self) -> io::Result<()> {
        let out = conout()?;
        let _unused = self.buf.flush();

        let (mut fg, bg) = if self.info.reverse {
            (self.info.background, self.info.foreground)
        } else {
            (self.info.foreground, self.info.background)
        };

        if self.info.secure {
            fg = bg;
        }

        let mut accum: WORD = 0;

        accum |= color_to_bits(fg);
        accum |= color_to_bits(bg) << 4;

        if self.info.standout {
            accum |= BACKGROUND_INTENSITY;
        } else {
            accum &= BACKGROUND_INTENSITY ^ 0xFF;
        }

        unsafe {
            SetConsoleTextAttribute(*out, accum);
        }
        Ok(())
    }

    /// Create a new WinConsole with the given WinConsoleInfo and out
    pub fn new_with_consoleinfo(out: T, info: WinConsoleInfo) -> WinConsole<T> {
        WinConsole { buf: out, info }
    }

    /// Returns `Err` whenever the terminal cannot be created for some
    /// reason.
    pub fn new(out: T) -> io::Result<WinConsole<T>> {
        let info = WinConsoleInfo::from_env()?;
        Ok(Self::new_with_consoleinfo(out, info))
    }
}

impl<T: Write> Write for WinConsole<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf.flush()
    }
}

impl<T: Write + Send> Terminal for WinConsole<T> {
    type Output = T;

    fn fg(&mut self, color: color::Color) -> Result<()> {
        self.info.foreground = color;
        self.apply()?;

        Ok(())
    }

    fn bg(&mut self, color: color::Color) -> Result<()> {
        self.info.background = color;
        self.apply()?;

        Ok(())
    }

    fn attr(&mut self, attr: Attr) -> Result<()> {
        match attr {
            Attr::ForegroundColor(f) => {
                self.info.foreground = f;
                self.apply()?;
                Ok(())
            }
            Attr::BackgroundColor(b) => {
                self.info.background = b;
                self.apply()?;
                Ok(())
            }
            Attr::Reverse => {
                self.info.reverse = true;
                self.apply()?;
                Ok(())
            }
            Attr::Secure => {
                self.info.secure = true;
                self.apply()?;
                Ok(())
            }
            Attr::Standout(v) => {
                self.info.standout = v;
                self.apply()?;
                Ok(())
            }
            _ => Err(Error::NotSupported),
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        match attr {
            Attr::ForegroundColor(_)
            | Attr::BackgroundColor(_)
            | Attr::Standout(_)
            | Attr::Reverse
            | Attr::Secure => true,
            _ => false,
        }
    }

    fn reset(&mut self) -> Result<()> {
        self.info.foreground = self.info.def_foreground;
        self.info.background = self.info.def_background;
        self.info.reverse = false;
        self.info.secure = false;
        self.info.standout = false;
        self.apply()?;

        Ok(())
    }

    fn supports_reset(&self) -> bool {
        true
    }

    fn supports_color(&self) -> bool {
        true
    }

    fn cursor_up(&mut self) -> Result<()> {
        let _unused = self.buf.flush();
        let handle = conout()?;
        unsafe {
            let buffer_info = get_console_screen_buffer_info(*handle)?;
            let (x, y) = (
                buffer_info.dwCursorPosition.X,
                buffer_info.dwCursorPosition.Y,
            );
            if y == 0 {
                // Even though this might want to be a CursorPositionInvalid, on Unix there
                // is no checking to see if the cursor is already on the first line.
                // I'm not sure what the ideal behavior is, but I think it'd be silly to have
                // cursor_up fail in this case.
                Ok(())
            } else {
                let pos = COORD { X: x, Y: y - 1 };
                if SetConsoleCursorPosition(*handle, pos) != 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error().into())
                }
            }
        }
    }

    fn delete_line(&mut self) -> Result<()> {
        let _unused = self.buf.flush();
        let handle = conout()?;
        unsafe {
            let buffer_info = get_console_screen_buffer_info(*handle)?;
            let pos = buffer_info.dwCursorPosition;
            let size = buffer_info.dwSize;
            let num = (size.X - pos.X) as DWORD;
            let mut written = 0;
            // 0x0020u16 is ' ' (space) in UTF-16 (same as ascii)
            if FillConsoleOutputCharacterW(*handle, 0x0020, num, pos, &mut written) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            if FillConsoleOutputAttribute(*handle, 0, num, pos, &mut written) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            // Similar reasoning for not failing as in cursor_up -- it doesn't even make
            // sense to
            // me that these APIs could have written 0, unless the terminal is width zero.
            Ok(())
        }
    }

    fn carriage_return(&mut self) -> Result<()> {
        let _unused = self.buf.flush();
        let handle = conout()?;
        unsafe {
            let buffer_info = get_console_screen_buffer_info(*handle)?;
            let COORD { X: x, Y: y } = buffer_info.dwCursorPosition;
            if x == 0 {
                Err(Error::CursorDestinationInvalid)
            } else {
                let pos = COORD { X: 0, Y: y };
                if SetConsoleCursorPosition(*handle, pos) != 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error().into())
                }
            }
        }
    }

    fn get_ref<'a>(&'a self) -> &'a T {
        &self.buf
    }

    fn get_mut<'a>(&'a mut self) -> &'a mut T {
        &mut self.buf
    }

    fn into_inner(self) -> T
    where
        Self: Sized,
    {
        self.buf
    }
}
