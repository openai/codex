//! Short, best-effort terminal response probes for TUI startup and resume.
//!
//! Crossterm's public helpers wait up to two seconds for terminal responses. That is too long for
//! TUI startup and resume, where unsupported terminals should simply fall back to conservative
//! defaults.
//! This module sends the same kinds of optional terminal queries with a caller-provided deadline,
//! prefers duplicated stdio handles, falls back to the controlling terminal path when stdio is
//! unavailable, and reports `None` when a response is unavailable.
//!
//! Probes run only while the crossterm event stream is absent or paused, so they do not share
//! crossterm's internal skipped-event queue. The startup probe returns plain input read alongside
//! terminal responses so callers can preserve it without replaying terminal control sequences.

use std::time::Duration;

use crossterm::event::KeyEvent;

/// Default wall-clock budget for each startup probe group.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_millis(100);

/// Default terminal foreground and background colors reported by OSC 10 and OSC 11.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct DefaultColors {
    /// Default foreground color as an 8-bit RGB tuple.
    pub(crate) fg: (u8, u8, u8),
    /// Default background color as an 8-bit RGB tuple.
    pub(crate) bg: (u8, u8, u8),
}

/// User input read while the Unix startup probe owns the terminal.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum StartupInput {
    Plain(Vec<u8>),
    Paste(Vec<u8>),
    Key(KeyEvent),
    UnknownAction,
}

#[cfg(unix)]
#[cfg_attr(test, allow(dead_code))]
#[path = "terminal_probe/unix.rs"]
mod imp;
#[cfg(windows)]
mod imp {
    use super::DefaultColors;
    use super::parse_default_colors;
    use std::io;
    use std::io::ErrorKind;
    use std::time::Duration;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
    use windows_sys::Win32::Storage::FileSystem::ReadFile;
    use windows_sys::Win32::Storage::FileSystem::WriteFile;
    use windows_sys::Win32::System::Console::CONSOLE_SCREEN_BUFFER_INFOEX;
    use windows_sys::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_INPUT;
    use windows_sys::Win32::System::Console::GetConsoleMode;
    use windows_sys::Win32::System::Console::GetConsoleScreenBufferInfoEx;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
    use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
    use windows_sys::Win32::System::Console::SetConsoleMode;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    /// Queries OSC 10 and OSC 11 default colors under one shared deadline.
    ///
    /// The Windows path uses raw console handles because crossterm's public color query helper is
    /// currently Unix-only. Failures and missing responses are reported as `Ok(None)` by callers so
    /// terminals without OSC 10/11 support keep the existing conservative palette fallback.
    pub(crate) fn default_colors(timeout: Duration) -> io::Result<Option<DefaultColors>> {
        let Ok(output) = std_handle(STD_OUTPUT_HANDLE) else {
            return Ok(None);
        };

        if let Ok(input) = std_handle(STD_INPUT_HANDLE)
            && let Ok(Some(colors)) = query_osc_default_colors(input, output, timeout)
        {
            return Ok(Some(colors));
        }

        console_default_colors()
    }

    /// Reads the configured console palette without consuming terminal input.
    pub(crate) fn console_default_colors() -> io::Result<Option<DefaultColors>> {
        let Ok(output) = std_handle(STD_OUTPUT_HANDLE) else {
            return Ok(None);
        };
        Ok(query_console_default_colors(output).ok().flatten())
    }

    fn query_osc_default_colors(
        input: HANDLE,
        output: HANDLE,
        timeout: Duration,
    ) -> io::Result<Option<DefaultColors>> {
        let _vt_input = VirtualTerminalInputMode::enable(input)?;
        write_all(output, b"\x1B]10;?\x1B\\\x1B]11;?\x1B\\")?;
        read_until(input, timeout, parse_default_colors)
    }

    fn query_console_default_colors(output: HANDLE) -> io::Result<Option<DefaultColors>> {
        let mut info = unsafe { std::mem::zeroed::<CONSOLE_SCREEN_BUFFER_INFOEX>() };
        info.cbSize = std::mem::size_of::<CONSOLE_SCREEN_BUFFER_INFOEX>() as u32;
        if unsafe { GetConsoleScreenBufferInfoEx(output, &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Some(decode_console_default_colors(
            info.wAttributes,
            &info.ColorTable,
        )))
    }

    fn decode_console_default_colors(attributes: u16, color_table: &[u32; 16]) -> DefaultColors {
        let fg_index = (attributes & 0x0f) as usize;
        let bg_index = ((attributes >> 4) & 0x0f) as usize;
        // COMMON_LVB_REVERSE_VIDEO changes how cells render, but this probe is discovering the
        // configured default colors for palette blending. Keep the attribute fg/bg indices as-is.
        DefaultColors {
            fg: decode_color_ref(color_table[fg_index]),
            bg: decode_color_ref(color_table[bg_index]),
        }
    }

    fn decode_color_ref(color_ref: u32) -> (u8, u8, u8) {
        (
            (color_ref & 0xff) as u8,
            ((color_ref >> 8) & 0xff) as u8,
            ((color_ref >> 16) & 0xff) as u8,
        )
    }

    fn std_handle(kind: u32) -> io::Result<HANDLE> {
        let handle = unsafe { GetStdHandle(kind) };
        if handle == 0 || handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(handle)
    }

    struct VirtualTerminalInputMode {
        handle: HANDLE,
        original_mode: u32,
    }

    impl VirtualTerminalInputMode {
        fn enable(handle: HANDLE) -> io::Result<Self> {
            let mut original_mode = 0;
            if unsafe { GetConsoleMode(handle, &mut original_mode) } == 0 {
                return Err(io::Error::last_os_error());
            }

            let requested_mode = original_mode | ENABLE_VIRTUAL_TERMINAL_INPUT;
            if unsafe { SetConsoleMode(handle, requested_mode) } == 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                handle,
                original_mode,
            })
        }
    }

    impl Drop for VirtualTerminalInputMode {
        fn drop(&mut self) {
            unsafe {
                SetConsoleMode(self.handle, self.original_mode);
            }
        }
    }

    fn write_all(handle: HANDLE, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let mut written = 0;
            let ok = unsafe {
                WriteFile(
                    handle,
                    bytes.as_ptr().cast(),
                    bytes.len().min(u32::MAX as usize) as u32,
                    &mut written,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            if written == 0 {
                return Err(io::Error::from(ErrorKind::WriteZero));
            }
            bytes = &bytes[written as usize..];
        }
        Ok(())
    }

    fn read_until<T>(
        handle: HANDLE,
        timeout: Duration,
        mut parse: impl FnMut(&[u8]) -> Option<T>,
    ) -> io::Result<Option<T>> {
        let deadline = Instant::now() + timeout;
        let mut buffer = Vec::new();
        loop {
            if let Some(value) = parse(&buffer) {
                return Ok(Some(value));
            }

            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let timeout_ms = deadline
                .saturating_duration_since(now)
                .as_millis()
                .min(u32::MAX as u128) as u32;
            match unsafe { WaitForSingleObject(handle, timeout_ms) } {
                WAIT_OBJECT_0 => read_once(handle, &mut buffer)?,
                WAIT_TIMEOUT => return Ok(None),
                _ => return Err(io::Error::last_os_error()),
            }
        }
    }

    fn read_once(handle: HANDLE, buffer: &mut Vec<u8>) -> io::Result<()> {
        let mut chunk = [0_u8; 256];
        let mut read = 0;
        let ok = unsafe {
            ReadFile(
                handle,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        buffer.extend_from_slice(&chunk[..read as usize]);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use pretty_assertions::assert_eq;
        use windows_sys::Win32::System::Console::COMMON_LVB_REVERSE_VIDEO;

        fn color_table() -> [u32; 16] {
            [
                0x00000000, 0x00000080, 0x00008000, 0x00008080, 0x00800000, 0x00800080, 0x00808000,
                0x00c0c0c0, 0x00808080, 0x000000ff, 0x0000ff00, 0x0000ffff, 0x00ff0000, 0x00ff00ff,
                0x00ffff00, 0x00ffffff,
            ]
        }

        #[test]
        fn decodes_console_color_attribute_indices() {
            assert_eq!(
                decode_console_default_colors(/*attributes*/ 0x21, &color_table()),
                DefaultColors {
                    fg: (128, 0, 0),
                    bg: (0, 128, 0),
                }
            );
        }

        #[test]
        fn decodes_console_color_intensity_indices() {
            assert_eq!(
                decode_console_default_colors(/*attributes*/ 0xe9, &color_table()),
                DefaultColors {
                    fg: (255, 0, 0),
                    bg: (0, 255, 255),
                }
            );
        }

        #[test]
        fn decodes_console_color_ref_byte_order() {
            let mut colors = color_table();
            colors[3] = 0x00112233;
            colors[4] = 0x00aabbcc;

            assert_eq!(
                decode_console_default_colors(/*attributes*/ 0x43, &colors),
                DefaultColors {
                    fg: (0x33, 0x22, 0x11),
                    bg: (0xcc, 0xbb, 0xaa),
                }
            );
        }

        #[test]
        fn ignores_reverse_video_when_decoding_default_colors() {
            assert_eq!(
                decode_console_default_colors(
                    /*attributes*/ COMMON_LVB_REVERSE_VIDEO | 0x21,
                    &color_table(),
                ),
                DefaultColors {
                    fg: (128, 0, 0),
                    bg: (0, 128, 0),
                }
            );
        }
    }
}

fn parse_osc_color(buffer: &[u8], slot: u8) -> Option<(u8, u8, u8)> {
    let prefix = format!("\x1B]{slot};");
    for start in buffer
        .windows(prefix.len())
        .enumerate()
        .filter_map(|(index, window)| (window == prefix.as_bytes()).then_some(index))
    {
        if is_inside_bracketed_paste(buffer, start) {
            continue;
        }
        let payload_start = start + prefix.len();
        let rest = &buffer[payload_start..];
        let Some((payload_end, _terminator_len)) = osc_payload_end(rest) else {
            continue;
        };
        let Ok(payload) = std::str::from_utf8(&rest[..payload_end]) else {
            continue;
        };
        if let Some(color) = parse_osc_rgb(payload) {
            return Some(color);
        }
    }
    None
}

fn parse_default_colors(buffer: &[u8]) -> Option<DefaultColors> {
    let fg = parse_osc_color(buffer, /*slot*/ 10)?;
    let bg = parse_osc_color(buffer, /*slot*/ 11)?;
    Some(DefaultColors { fg, bg })
}

fn osc_payload_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let mut idx = 0;
    while idx < buffer.len() {
        match buffer[idx] {
            0x07 => return Some((idx, 1)),
            0x1B if buffer.get(idx + 1) == Some(&b'\\') => return Some((idx, 2)),
            _ => idx += 1,
        }
    }
    None
}

fn parse_osc_rgb(payload: &str) -> Option<(u8, u8, u8)> {
    let (prefix, values) = payload.trim().split_once(':')?;
    if !prefix.eq_ignore_ascii_case("rgb") && !prefix.eq_ignore_ascii_case("rgba") {
        return None;
    }

    let mut parts = values.split('/');
    let r = parse_osc_component(parts.next()?)?;
    let g = parse_osc_component(parts.next()?)?;
    let b = parse_osc_component(parts.next()?)?;
    if prefix.eq_ignore_ascii_case("rgba") {
        parse_osc_component(parts.next()?)?;
    }
    parts.next().is_none().then_some((r, g, b))
}

fn parse_osc_component(component: &str) -> Option<u8> {
    match component.len() {
        2 => u8::from_str_radix(component, 16).ok(),
        4 => u16::from_str_radix(component, 16)
            .ok()
            .map(|value| (value / 257) as u8),
        _ => None,
    }
}

fn is_inside_bracketed_paste(buffer: &[u8], offset: usize) -> bool {
    const PASTE_START: &[u8] = b"\x1b[200~";
    const PASTE_END: &[u8] = b"\x1b[201~";

    let mut index = 0;
    let mut in_paste = false;
    while index < offset {
        let remaining = &buffer[index..];
        if !in_paste && remaining.starts_with(PASTE_START) {
            in_paste = true;
            index += PASTE_START.len();
        } else if in_paste && remaining.starts_with(PASTE_END) {
            in_paste = false;
            index += PASTE_END.len();
        } else {
            index += 1;
        }
    }
    in_paste
}

#[cfg(any(unix, windows))]
pub(crate) use imp::*;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_osc_colors_with_bel_and_st() {
        assert_eq!(
            parse_osc_color(b"\x1B]10;rgb:ffff/8000/0000\x07", /*slot*/ 10),
            Some((255, 127, 0))
        );
        assert_eq!(
            parse_osc_color(b"\x1B]11;rgba:00/80/ff/ff\x1B\\", /*slot*/ 11),
            Some((0, 128, 255))
        );
    }

    #[test]
    fn parses_two_and_four_digit_color_components() {
        assert_eq!(parse_osc_rgb("rgb:00/80/ff"), Some((0, 128, 255)));
        assert_eq!(
            parse_osc_rgb("rgba:ffff/8000/0000/ffff"),
            Some((255, 127, 0))
        );
    }

    #[test]
    fn parses_default_colors_from_one_buffer() {
        assert_eq!(
            parse_default_colors(b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B]11;rgb:1111/1111/1111\x07"),
            Some(DefaultColors {
                fg: (238, 238, 238),
                bg: (17, 17, 17)
            })
        );
        assert_eq!(
            parse_default_colors(b"\x1B]11;rgb:1111/1111/1111\x07\x1B]10;rgb:eeee/eeee/eeee\x1B\\"),
            Some(DefaultColors {
                fg: (238, 238, 238),
                bg: (17, 17, 17)
            })
        );
        assert_eq!(
            parse_default_colors(b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\"),
            None
        );
    }

    #[test]
    fn ignores_malformed_or_partial_default_color_responses() {
        assert_eq!(
            parse_default_colors(b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B]11;rgb:nope\x07"),
            None
        );
        assert_eq!(
            parse_default_colors(b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B]11;rgb:11/11/11/11\x07"),
            None
        );
        assert_eq!(
            parse_default_colors(b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B]11;rgb:1111/1111/1111"),
            None
        );
    }

    #[test]
    fn parses_default_colors_with_unrelated_bytes() {
        assert_eq!(
            parse_default_colors(
                b"typed\x1B]10;rgb:eeee/eeee/eeee\x1B\\noise\x1B]11;rgb:1111/1111/1111\x07"
            ),
            Some(DefaultColors {
                fg: (238, 238, 238),
                bg: (17, 17, 17),
            })
        );
    }

    #[test]
    fn default_colors_ignore_responses_inside_bracketed_paste() {
        let mut buffer =
            b"\x1b[200~\x1B]10;rgb:aaaa/aaaa/aaaa\x1B\\\x1B]11;rgb:bbbb/bbbb/bbbb\x1B\\\x1b[201~"
                .to_vec();
        assert_eq!(parse_default_colors(&buffer), None);

        buffer
            .extend_from_slice(b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B]11;rgb:1111/1111/1111\x1B\\");
        assert_eq!(
            parse_default_colors(&buffer),
            Some(DefaultColors {
                fg: (238, 238, 238),
                bg: (17, 17, 17),
            })
        );
    }
}
