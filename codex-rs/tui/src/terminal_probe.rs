//! Short, best-effort terminal response probes for TUI startup and resume.
//!
//! Crossterm's public helpers wait up to two seconds for terminal responses. That is too long for
//! TUI startup, where unsupported terminals should simply fall back to conservative defaults.
//! This module sends the same kinds of optional terminal queries with a caller-provided deadline,
//! prefers duplicated stdio handles, falls back to the controlling terminal path when stdio is
//! unavailable, and reports `None` when a response is unavailable.
//!
//! The probes run before the crossterm event stream is created, so they do not share crossterm's
//! internal skipped-event queue. Bytes read while looking for probe responses are consumed from the
//! terminal; keeping the timeout short is part of the contract that makes this acceptable for
//! startup. A future input-preservation layer would need to replay unrelated bytes through the same
//! parser that normal TUI input uses.

use std::time::Duration;

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

#[cfg(unix)]
#[cfg_attr(test, allow(dead_code))]
mod imp {
    use super::DefaultColors;
    use super::parse_default_colors;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::time::Duration;
    use std::time::Instant;

    use crossterm::event::KeyboardEnhancementFlags;
    use ratatui::layout::Position;

    /// Results from the TUI's one-shot startup terminal probe.
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub(crate) struct StartupProbe {
        pub(crate) cursor_position: Option<Position>,
        pub(crate) default_colors: Option<DefaultColors>,
        pub(crate) keyboard_enhancement_supported: Option<bool>,
    }

    /// Whether the startup probe should query keyboard enhancement support.
    #[derive(Clone, Copy, Eq, PartialEq)]
    pub(crate) enum StartupKeyboardEnhancementProbe {
        Query,
        Skip,
    }

    /// Temporary terminal handle used while a startup probe owns terminal input.
    ///
    /// The preferred path is duplicated stdin/stdout, because terminal replies are delivered to the
    /// same input stream crossterm reads from. Some embedded or redirected environments expose a
    /// controlling terminal without terminal stdio; in that case the handle falls back to
    /// `/dev/tty`. Only the reader is switched to nonblocking mode, and its original file status
    /// flags are restored when the handle is dropped.
    struct Tty {
        reader: File,
        writer: File,
        original_flags: libc::c_int,
    }

    impl Tty {
        /// Opens an isolated reader and writer for startup probes.
        ///
        /// The reader and writer must be separate file descriptions so switching the reader into
        /// nonblocking mode does not also make writes fail with `WouldBlock` under terminal
        /// backpressure. Falling back to `/dev/tty` keeps embedded or redirected environments
        /// usable when they still expose a controlling terminal.
        fn open() -> io::Result<Self> {
            let stdio_reader = dup_file(libc::STDIN_FILENO);
            let stdio_writer = dup_file(libc::STDOUT_FILENO);
            match (stdio_reader, stdio_writer) {
                (Ok(reader), Ok(writer)) => Self::new(reader, writer),
                (reader, writer) => {
                    let stdio_err = match (reader.err(), writer.err()) {
                        (Some(reader_err), Some(writer_err)) => {
                            format!("reader: {reader_err}; writer: {writer_err}")
                        }
                        (Some(reader_err), None) => format!("reader: {reader_err}"),
                        (None, Some(writer_err)) => format!("writer: {writer_err}"),
                        (None, None) => "unknown stdio duplicate error".to_string(),
                    };
                    let reader =
                        OpenOptions::new()
                            .read(true)
                            .open("/dev/tty")
                            .map_err(|fallback_err| {
                                io::Error::new(
                                    fallback_err.kind(),
                                    format!(
                                        "failed to duplicate stdio ({stdio_err}) or open /dev/tty reader ({fallback_err})"
                                    ),
                                )
                            })?;
                    let writer = OpenOptions::new().write(true).open("/dev/tty").map_err(
                        |fallback_err| {
                            io::Error::new(
                                fallback_err.kind(),
                                format!(
                                    "failed to duplicate stdio ({stdio_err}) or open /dev/tty writer ({fallback_err})"
                                ),
                            )
                        },
                    )?;
                    Self::new(reader, writer)
                }
            }
        }

        fn new(reader: File, writer: File) -> io::Result<Self> {
            let fd = reader.as_raw_fd();
            let original_flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if original_flags == -1 {
                return Err(io::Error::last_os_error());
            }
            if unsafe { libc::fcntl(fd, libc::F_SETFL, original_flags | libc::O_NONBLOCK) } == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                reader,
                writer,
                original_flags,
            })
        }

        fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.writer.write_all(bytes)?;
            self.writer.flush()
        }

        fn read_available(&mut self, buffer: &mut Vec<u8>) -> io::Result<()> {
            let mut chunk = [0_u8; 256];
            loop {
                let count = unsafe {
                    libc::read(
                        self.reader.as_raw_fd(),
                        chunk.as_mut_ptr().cast::<libc::c_void>(),
                        chunk.len(),
                    )
                };
                if count > 0 {
                    buffer.extend_from_slice(&chunk[..count as usize]);
                    continue;
                }
                if count == 0 {
                    return Ok(());
                }
                let err = io::Error::last_os_error();
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) {
                    return Ok(());
                }
                return Err(err);
            }
        }

        fn poll_readable(&self, timeout: Duration) -> io::Result<bool> {
            let mut fd = libc::pollfd {
                fd: self.reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let deadline = Instant::now() + timeout;
            loop {
                let now = Instant::now();
                if now >= deadline {
                    return Ok(false);
                }
                let timeout_ms = deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .min(libc::c_int::MAX as u128) as libc::c_int;
                let result = unsafe {
                    libc::poll(&mut fd, /*nfds*/ 1, timeout_ms)
                };
                if result > 0 {
                    return Ok((fd.revents & libc::POLLIN) != 0);
                }
                if result == 0 {
                    return Ok(false);
                }
                let err = io::Error::last_os_error();
                if err.kind() != io::ErrorKind::Interrupted {
                    return Err(err);
                }
            }
        }
    }

    impl Drop for Tty {
        fn drop(&mut self) {
            let _ =
                unsafe { libc::fcntl(self.reader.as_raw_fd(), libc::F_SETFL, self.original_flags) };
        }
    }

    /// Duplicates a process stdio descriptor so probe cleanup owns only the duplicate.
    fn dup_file(fd: libc::c_int) -> io::Result<File> {
        let duplicated = unsafe { libc::dup(fd) };
        if duplicated == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(duplicated) })
    }

    /// Queries OSC 10 and OSC 11 default colors under one shared deadline.
    ///
    /// Foreground and background are only useful as a pair for palette calculations, so a missing
    /// response from either slot returns `Ok(None)`. Both queries are sent before reading so a
    /// terminal that supports palette replies gets the full bounded window to return both values,
    /// while unsupported terminals still pay one bounded wait instead of one wait per slot.
    pub(crate) fn default_colors(timeout: Duration) -> io::Result<Option<DefaultColors>> {
        let mut tty = Tty::open()?;
        tty.write_all(b"\x1B]10;?\x1B\\\x1B]11;?\x1B\\")?;
        let Some(colors) = read_until(&mut tty, timeout, parse_default_colors)? else {
            return Ok(None);
        };
        Ok(Some(colors))
    }

    /// Queries the terminal cursor position while normal input polling is paused.
    ///
    /// Resume can emit a focus report immediately before the cursor-position response. Reusing
    /// the startup parser lets the probe find the response without leaking either sequence into
    /// the composer.
    pub(crate) fn cursor_position(timeout: Duration) -> io::Result<Option<Position>> {
        let mut tty = Tty::open()?;
        tty.write_all(b"\x1B[6n")?;
        read_until(&mut tty, timeout, parse_cursor_position)
    }

    /// Runs the optional terminal queries needed during TUI startup under one shared deadline.
    ///
    /// Keeping these queries batched avoids paying one timeout per unsupported capability before
    /// the first frame can render.
    pub(crate) fn startup(
        timeout: Duration,
        keyboard_probe: StartupKeyboardEnhancementProbe,
    ) -> io::Result<StartupProbe> {
        let mut tty = Tty::open()?;
        match keyboard_probe {
            StartupKeyboardEnhancementProbe::Query => {
                tty.write_all(b"\x1B[6n\x1B]10;?\x1B\\\x1B]11;?\x1B\\\x1B[?u\x1B[c")?;
            }
            StartupKeyboardEnhancementProbe::Skip => {
                tty.write_all(b"\x1B[6n\x1B]10;?\x1B\\\x1B]11;?\x1B\\")?;
            }
        }
        read_startup_probe(&mut tty, timeout, keyboard_probe)
    }

    /// Reads available terminal bytes until `parse` recognizes a probe response or time expires.
    ///
    /// The accumulated buffer may include unrelated terminal input. This helper intentionally does
    /// not try to replay those bytes, so it must stay limited to short startup probes that run
    /// before normal crossterm input polling begins.
    fn read_until<T>(
        tty: &mut Tty,
        timeout: Duration,
        mut parse: impl FnMut(&[u8]) -> Option<T>,
    ) -> io::Result<Option<T>> {
        let deadline = Instant::now() + timeout;
        let mut buffer = Vec::new();
        loop {
            tty.read_available(&mut buffer)?;
            if let Some(value) = parse(&buffer) {
                return Ok(Some(value));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            if !tty.poll_readable(deadline.saturating_duration_since(now))? {
                return Ok(None);
            }
        }
    }

    fn read_startup_probe(
        tty: &mut Tty,
        timeout: Duration,
        keyboard_probe: StartupKeyboardEnhancementProbe,
    ) -> io::Result<StartupProbe> {
        let deadline = Instant::now() + timeout;
        let mut buffer = Vec::new();
        let mut probe = StartupProbe {
            cursor_position: None,
            default_colors: None,
            keyboard_enhancement_supported: None,
        };
        let mut saw_supported_keyboard = false;
        loop {
            tty.read_available(&mut buffer)?;
            update_startup_probe(
                &mut probe,
                &mut saw_supported_keyboard,
                &buffer,
                keyboard_probe,
            );
            if startup_probe_complete(&probe, keyboard_probe) {
                return Ok(probe);
            }
            let now = Instant::now();
            if now >= deadline {
                finish_startup_probe(&mut probe, keyboard_probe, saw_supported_keyboard);
                return Ok(probe);
            }
            if !tty.poll_readable(deadline.saturating_duration_since(now))? {
                finish_startup_probe(&mut probe, keyboard_probe, saw_supported_keyboard);
                return Ok(probe);
            }
        }
    }

    fn update_startup_probe(
        probe: &mut StartupProbe,
        saw_supported_keyboard: &mut bool,
        buffer: &[u8],
        keyboard_probe: StartupKeyboardEnhancementProbe,
    ) {
        if probe.cursor_position.is_none() {
            probe.cursor_position = parse_cursor_position(buffer);
        }
        if probe.default_colors.is_none() {
            probe.default_colors = parse_default_colors(buffer);
        }
        if keyboard_probe == StartupKeyboardEnhancementProbe::Skip
            || probe.keyboard_enhancement_supported.is_some()
        {
            return;
        }
        match parse_keyboard_enhancement_support(buffer) {
            KeyboardProbeState::SupportedAndFallback => {
                probe.keyboard_enhancement_supported = Some(true);
            }
            KeyboardProbeState::Supported => {
                *saw_supported_keyboard = true;
            }
            KeyboardProbeState::UnsupportedFallback => {
                probe.keyboard_enhancement_supported = Some(false);
            }
            KeyboardProbeState::Pending => {}
        }
    }

    fn startup_probe_complete(
        probe: &StartupProbe,
        keyboard_probe: StartupKeyboardEnhancementProbe,
    ) -> bool {
        probe.cursor_position.is_some()
            && probe.default_colors.is_some()
            && (keyboard_probe == StartupKeyboardEnhancementProbe::Skip
                || probe.keyboard_enhancement_supported.is_some())
    }

    fn finish_startup_probe(
        probe: &mut StartupProbe,
        keyboard_probe: StartupKeyboardEnhancementProbe,
        saw_supported_keyboard: bool,
    ) {
        if keyboard_probe == StartupKeyboardEnhancementProbe::Query
            && probe.keyboard_enhancement_supported.is_none()
        {
            probe.keyboard_enhancement_supported = saw_supported_keyboard.then_some(true);
        }
    }

    fn parse_cursor_position(buffer: &[u8]) -> Option<Position> {
        for start in find_all_subslices(buffer, b"\x1B[") {
            let rest = &buffer[start + 2..];
            let Some(end) = rest.iter().position(|b| *b == b'R') else {
                continue;
            };
            let Ok(payload) = std::str::from_utf8(&rest[..end]) else {
                continue;
            };
            let Some((row, col)) = payload.split_once(';') else {
                continue;
            };
            let Ok(row) = row.parse::<u16>() else {
                continue;
            };
            let Ok(col) = col.parse::<u16>() else {
                continue;
            };
            let row = row.saturating_sub(1);
            let col = col.saturating_sub(1);
            return Some(Position { x: col, y: row });
        }
        None
    }

    /// Parser state for the keyboard enhancement probe.
    ///
    /// `UnsupportedFallback` records that a primary-device-attributes response arrived without
    /// keyboard flags. Startup treats that as unsupported immediately, matching crossterm's
    /// previous behavior and avoiding a fixed delay in terminals without the keyboard protocol.
    /// `Supported` records that keyboard flags arrived, but the caller should still drain the PDA
    /// fallback response if it arrives before the deadline so those bytes do not leak into the
    /// normal event stream.
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    enum KeyboardProbeState {
        Pending,
        UnsupportedFallback,
        Supported,
        SupportedAndFallback,
    }

    fn parse_keyboard_enhancement_support(buffer: &[u8]) -> KeyboardProbeState {
        match (
            find_keyboard_flags(buffer).is_some(),
            find_primary_device_attributes(buffer).is_some(),
        ) {
            (true, true) => KeyboardProbeState::SupportedAndFallback,
            (true, false) => KeyboardProbeState::Supported,
            (false, true) => KeyboardProbeState::UnsupportedFallback,
            (false, false) => KeyboardProbeState::Pending,
        }
    }

    fn find_keyboard_flags(buffer: &[u8]) -> Option<KeyboardEnhancementFlags> {
        for start in find_all_subslices(buffer, b"\x1B[?") {
            let rest = &buffer[start + 3..];
            let Some(end) = rest.iter().position(|b| *b == b'u') else {
                continue;
            };
            if end == 0 {
                continue;
            }
            let Ok(bits_text) = std::str::from_utf8(&rest[..end]) else {
                continue;
            };
            let Ok(bits) = bits_text.parse::<u8>() else {
                continue;
            };
            let mut flags = KeyboardEnhancementFlags::empty();
            if bits & 1 != 0 {
                flags |= KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
            }
            if bits & 2 != 0 {
                flags |= KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
            }
            if bits & 4 != 0 {
                flags |= KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;
            }
            if bits & 8 != 0 {
                flags |= KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
            }
            return Some(flags);
        }
        None
    }

    fn find_primary_device_attributes(buffer: &[u8]) -> Option<()> {
        for start in find_all_subslices(buffer, b"\x1B[?") {
            let rest = &buffer[start + 3..];
            let Some(end) = rest.iter().position(|b| *b == b'c') else {
                continue;
            };
            if end > 0 && rest[..end].iter().all(|b| b.is_ascii_digit() || *b == b';') {
                return Some(());
            }
        }
        None
    }

    fn find_all_subslices<'a>(
        haystack: &'a [u8],
        needle: &'a [u8],
    ) -> impl Iterator<Item = usize> + 'a {
        haystack
            .windows(needle.len())
            .enumerate()
            .filter_map(move |(idx, window)| (window == needle).then_some(idx))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn parses_cursor_position_as_zero_based() {
            assert_eq!(
                parse_cursor_position(b"\x1B[20;10R"),
                Some(Position { x: 9, y: 19 })
            );
            assert_eq!(
                parse_cursor_position(b"\x1B[I\x1B[20;10R"),
                Some(Position { x: 9, y: 19 })
            );
        }

        #[test]
        fn parses_keyboard_enhancement_flags_and_pda_fallback() {
            assert_eq!(
                parse_keyboard_enhancement_support(b"\x1B[?7u"),
                KeyboardProbeState::Supported
            );
            assert_eq!(
                parse_keyboard_enhancement_support(b"\x1B[?64;1;2c"),
                KeyboardProbeState::UnsupportedFallback
            );
            assert_eq!(
                parse_keyboard_enhancement_support(b"\x1B[?64;1;2c\x1B[?7u"),
                KeyboardProbeState::SupportedAndFallback
            );
            assert_eq!(
                parse_keyboard_enhancement_support(b"\x1B[?7u\x1B[?64;1;2c"),
                KeyboardProbeState::SupportedAndFallback
            );
            assert_eq!(
                parse_keyboard_enhancement_support(b""),
                KeyboardProbeState::Pending
            );
        }

        #[test]
        fn startup_probe_parses_batched_terminal_responses() {
            let mut probe = StartupProbe {
                cursor_position: None,
                default_colors: None,
                keyboard_enhancement_supported: None,
            };
            let mut saw_supported_keyboard = false;
            update_startup_probe(
                &mut probe,
                &mut saw_supported_keyboard,
                b"\x1B[20;10R\x1B]11;rgb:1111/1111/1111\x07\x1B[?64;1;2c\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B[?7u",
                StartupKeyboardEnhancementProbe::Query,
            );

            assert_eq!(
                probe,
                StartupProbe {
                    cursor_position: Some(Position { x: 9, y: 19 }),
                    default_colors: Some(DefaultColors {
                        fg: (238, 238, 238),
                        bg: (17, 17, 17),
                    }),
                    keyboard_enhancement_supported: Some(true),
                }
            );
            assert!(startup_probe_complete(
                &probe,
                StartupKeyboardEnhancementProbe::Query
            ));
        }
    }
}

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
    let start = find_subslice(buffer, prefix.as_bytes())?;
    let payload_start = start + prefix.len();
    let rest = &buffer[payload_start..];
    let (payload_end, _terminator_len) = osc_payload_end(rest)?;
    let payload = std::str::from_utf8(&rest[..payload_end]).ok()?;
    parse_osc_rgb(payload)
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

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
}
