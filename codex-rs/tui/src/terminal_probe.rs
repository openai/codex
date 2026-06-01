//! Short, best-effort terminal response probes for TUI startup.
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

#[cfg(unix)]
#[cfg_attr(test, allow(dead_code))]
mod imp {
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

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
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
                parse_default_colors(
                    b"\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B]11;rgb:1111/1111/1111\x07"
                ),
                Some(DefaultColors {
                    fg: (238, 238, 238),
                    bg: (17, 17, 17)
                })
            );
            assert_eq!(
                parse_default_colors(
                    b"\x1B]11;rgb:1111/1111/1111\x07\x1B]10;rgb:eeee/eeee/eeee\x1B\\"
                ),
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

#[cfg(unix)]
pub(crate) use imp::*;
