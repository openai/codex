use super::DefaultColors;
use super::StartupInput;
use super::is_inside_bracketed_paste;
use super::parse_default_colors;
use std::ffi::CStr;
use std::ffi::OsStr;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::os::fd::AsRawFd;
#[cfg(test)]
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::KeyboardEnhancementFlags;
use ratatui::layout::Position;

#[path = "startup_input.rs"]
mod startup_input;
use startup_input::IncompleteInputPhase;
use startup_input::parse_startup_input;
use startup_input::settle_incomplete_input;

/// Maximum UTF-8 byte length of a user input accepted by the composer.
const MAX_STARTUP_INPUT_BYTES: usize = MAX_USER_INPUT_TEXT_CHARS * 4;
/// Leave bounded room for the probe replies and framing bytes around a maximum-sized input.
const MAX_PROBE_BUFFER_BYTES: usize = MAX_STARTUP_INPUT_BYTES + 32 * 1024;
const PASTE_COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);

/// Results from the TUI's one-shot startup terminal probe.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StartupProbe {
    pub(crate) cursor_position: Option<Position>,
    pub(crate) default_colors: Option<DefaultColors>,
    pub(crate) keyboard_enhancement_supported: Option<bool>,
    pub(crate) input: Vec<StartupInput>,
}

pub(crate) struct StartupProbeFailure {
    pub(crate) error: io::Error,
    pub(crate) input: Vec<StartupInput>,
}

/// Whether the startup probe should query keyboard enhancement support.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum StartupKeyboardEnhancementProbe {
    Query,
    Skip,
}

/// Temporary terminal handle used while a probe owns terminal input.
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
    /// Opens an isolated reader and writer for terminal probes.
    ///
    /// The reader and writer must be newly opened file descriptions so switching the reader into
    /// nonblocking mode cannot affect stdin or the writer. Prefer each stdio descriptor's terminal
    /// device, then the controlling terminal, with duplicated stdio only as a last resort.
    fn open() -> io::Result<Self> {
        let stdio_paths = (|| -> io::Result<(File, File)> {
            let reader_path = terminal_path(libc::STDIN_FILENO)?;
            let writer_path = terminal_path(libc::STDOUT_FILENO)?;
            Ok((
                OpenOptions::new().read(true).open(reader_path)?,
                OpenOptions::new().write(true).open(writer_path)?,
            ))
        })();
        let stdio_err = match stdio_paths {
            Ok((reader, writer)) => return Self::new(reader, writer),
            Err(err) => err,
        };

        let controlling_tty = (|| -> io::Result<(File, File)> {
            Ok((
                OpenOptions::new().read(true).open("/dev/tty")?,
                OpenOptions::new().write(true).open("/dev/tty")?,
            ))
        })();
        match controlling_tty {
            Ok((reader, writer)) => Self::new(reader, writer),
            Err(controlling_tty_err) => Err(io::Error::new(
                controlling_tty_err.kind(),
                format!(
                    "failed to open stdio terminal paths ({stdio_err}) or /dev/tty ({controlling_tty_err})"
                ),
            )),
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

    fn read_once(&mut self, buffer: &mut Vec<u8>) -> io::Result<()> {
        let remaining = MAX_PROBE_BUFFER_BYTES.saturating_sub(buffer.len());
        if remaining == 0 {
            return Ok(());
        }
        let mut chunk = [0_u8; 256];
        let limit = remaining.min(chunk.len());
        let count = self.read_into(&mut chunk[..limit])?;
        buffer.extend_from_slice(&chunk[..count]);
        Ok(())
    }

    fn read_available(&mut self, buffer: &mut Vec<u8>) -> io::Result<()> {
        loop {
            let before = buffer.len();
            self.read_once(buffer)?;
            if buffer.len() == before || buffer.len() == MAX_PROBE_BUFFER_BYTES {
                return Ok(());
            }
        }
    }

    fn read_into(&mut self, chunk: &mut [u8]) -> io::Result<usize> {
        let count = unsafe {
            libc::read(
                self.reader.as_raw_fd(),
                chunk.as_mut_ptr().cast::<libc::c_void>(),
                chunk.len(),
            )
        };
        if count > 0 {
            return Ok(count as usize);
        }
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "terminal input closed during probe",
            ));
        }
        let err = io::Error::last_os_error();
        if matches!(
            err.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        ) {
            return Ok(0);
        }
        Err(err)
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

fn terminal_path(fd: libc::c_int) -> io::Result<PathBuf> {
    let mut buffer = vec![0_u8; 4096];
    let result =
        unsafe { libc::ttyname_r(fd, buffer.as_mut_ptr().cast::<libc::c_char>(), buffer.len()) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    let path = unsafe { CStr::from_ptr(buffer.as_ptr().cast::<libc::c_char>()) };
    Ok(PathBuf::from(OsStr::from_bytes(path.to_bytes())))
}

impl Drop for Tty {
    fn drop(&mut self) {
        let _ = unsafe { libc::fcntl(self.reader.as_raw_fd(), libc::F_SETFL, self.original_flags) };
    }
}

/// Polls stdin without creating crossterm decoder state before the raw startup probe.
pub(crate) fn startup_event_available(timeout: Duration) -> io::Result<bool> {
    let mut fd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout_ms = timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int;
    loop {
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

/// Reads one complete startup-screen input unit without reading past its final byte.
///
/// The manual OSS picker runs before the raw terminal probe. Reading one byte at a time keeps
/// subsequent prompt input in the tty queue. Some ambiguous escape prefixes require one lookahead
/// byte; all events decoded from that unit are returned so the picker can queue the lookahead.
pub(crate) fn read_startup_events() -> io::Result<Vec<Event>> {
    let mut tty = Tty::open()?;
    let mut buffer = Vec::new();
    let mut deadline = Instant::now() + super::DEFAULT_TIMEOUT;
    loop {
        let wait = if buffer.is_empty() {
            Duration::from_secs(/*secs*/ 60)
        } else {
            deadline.saturating_duration_since(Instant::now())
        };
        if !tty.poll_readable(wait)? {
            if buffer.is_empty() {
                continue;
            }
            let Some(input) =
                settle_incomplete_input(&buffer, IncompleteInputPhase::QueuedUserInput)
            else {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "startup picker input sequence did not terminate",
                ));
            };
            let events = startup_input_events(input);
            if !events.is_empty() {
                return Ok(events);
            }
            buffer.clear();
            deadline = Instant::now() + super::DEFAULT_TIMEOUT;
            continue;
        }

        let mut byte = [0_u8; 1];
        if tty.read_into(&mut byte)? == 0 {
            continue;
        }
        buffer.push(byte[0]);
        let extracted = parse_startup_input(&buffer);
        if extracted.paste_open {
            deadline = Instant::now() + PASTE_COMPLETION_TIMEOUT;
            continue;
        }
        if !extracted.complete {
            deadline = Instant::now() + super::DEFAULT_TIMEOUT;
            continue;
        }
        let events = startup_input_events(extracted.input);
        if events.is_empty() {
            buffer.clear();
            deadline = Instant::now() + super::DEFAULT_TIMEOUT;
            continue;
        }
        return Ok(events);
    }
}

fn startup_input_events(input: Vec<StartupInput>) -> Vec<Event> {
    let mut events = Vec::new();
    for input in input {
        push_startup_input_events(&mut events, input);
    }
    events
}

fn push_startup_input_events(events: &mut Vec<Event>, input: StartupInput) {
    match input {
        StartupInput::Plain(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let mut chars = text.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                events.push(Event::Key(startup_plain_key_event(ch)));
            }
        }
        StartupInput::Paste(bytes) => {
            events.push(Event::Paste(String::from_utf8_lossy(&bytes).into_owned()));
        }
        StartupInput::Key(key_event) => events.push(Event::Key(key_event)),
        StartupInput::UnknownAction => {
            events.push(Event::Key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE)));
        }
    }
}

fn startup_plain_key_event(ch: char) -> KeyEvent {
    match ch {
        '\r' | '\n' => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        '\t' => KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        '\u{8}' | '\u{7f}' => KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        '\0' => KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        ch @ '\u{1}'..='\u{1a}' => KeyEvent::new(
            KeyCode::Char(char::from((u32::from(ch) - 1 + u32::from('a')) as u8)),
            KeyModifiers::CONTROL,
        ),
        ch @ '\u{1c}'..='\u{1f}' => KeyEvent::new(
            KeyCode::Char(char::from((u32::from(ch) - 0x1c + u32::from('4')) as u8)),
            KeyModifiers::CONTROL,
        ),
        ch => KeyEvent::new(
            KeyCode::Char(ch),
            if ch.is_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            },
        ),
    }
}

/// Duplicates a process stdio descriptor so probe cleanup owns only the duplicate.
#[cfg(test)]
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
    initial_input: Vec<u8>,
    initial_input_truncated: bool,
) -> Result<StartupProbe, StartupProbeFailure> {
    let fallback_input = recover_initial_startup_input(&initial_input, initial_input_truncated);
    let mut tty = Tty::open().map_err(|error| StartupProbeFailure {
        error,
        input: fallback_input.clone(),
    })?;
    startup_with_tty(
        &mut tty,
        timeout,
        keyboard_probe,
        initial_input,
        initial_input_truncated,
    )
    .map_err(|error| StartupProbeFailure {
        error,
        input: fallback_input,
    })
}

fn recover_initial_startup_input(
    initial_input: &[u8],
    initial_input_truncated: bool,
) -> Vec<StartupInput> {
    let extracted = parse_startup_input(initial_input);
    let incomplete = !extracted.complete || extracted.paste_open;
    let mut input = extracted.input;
    if initial_input_truncated || incomplete {
        input.push(StartupInput::UnknownAction);
    }
    input
}

fn startup_with_tty(
    tty: &mut Tty,
    timeout: Duration,
    keyboard_probe: StartupKeyboardEnhancementProbe,
    initial_input: Vec<u8>,
    initial_input_truncated: bool,
) -> io::Result<StartupProbe> {
    // Input queued during the slower pre-TUI startup belongs to the user, not to the terminal
    // queries below. Drain it first so a key sequence that shares a response encoding (for
    // example modified F3 and a cursor-position report) cannot satisfy a probe.
    let mut queued_input = read_pending_startup_input(tty, timeout, initial_input)?;
    if initial_input_truncated {
        queued_input.push(StartupInput::UnknownAction);
    }
    match keyboard_probe {
        StartupKeyboardEnhancementProbe::Query => {
            tty.write_all(b"\r\x1B[6n\x1B]10;?\x1B\\\x1B]11;?\x1B\\\x1B[?u\x1B[c")?;
        }
        StartupKeyboardEnhancementProbe::Skip => {
            tty.write_all(b"\r\x1B[6n\x1B]10;?\x1B\\\x1B]11;?\x1B\\")?;
        }
    }
    let mut probe = read_startup_probe(tty, timeout, keyboard_probe)?;
    queued_input.append(&mut probe.input);
    probe.input = queued_input;
    Ok(probe)
}

fn read_pending_startup_input(
    tty: &mut Tty,
    timeout: Duration,
    mut buffer: Vec<u8>,
) -> io::Result<Vec<StartupInput>> {
    tty.read_available(&mut buffer)?;
    if buffer.is_empty() {
        return Ok(Vec::new());
    }

    let mut deadline = Instant::now() + timeout;
    loop {
        let extracted = parse_startup_input(&buffer);
        if extracted.paste_open {
            if let Err(err) = finish_open_startup_paste(tty, &mut buffer) {
                if err.kind() != io::ErrorKind::TimedOut {
                    return Err(err);
                }
                let mut input = extracted.input;
                input.push(StartupInput::UnknownAction);
                return Ok(input);
            }
            continue;
        }
        if extracted.complete {
            return Ok(extracted.input);
        }

        let now = Instant::now();
        if now >= deadline || !tty.poll_readable(deadline.saturating_duration_since(now))? {
            if let Some(input) =
                settle_incomplete_input(&buffer, IncompleteInputPhase::QueuedUserInput)
            {
                return Ok(input);
            }
            let mut input = parse_startup_input(&buffer).input;
            input.push(StartupInput::UnknownAction);
            return Ok(input);
        }
        let previous_len = buffer.len();
        tty.read_available(&mut buffer)?;
        if buffer.len() > previous_len {
            deadline = Instant::now() + timeout;
        }
    }
}

/// Completes and decodes input already removed from the tty by the bounded startup reader.
///
/// Reading from the tty again is necessary because the reader may have stopped between bytes of a
/// UTF-8 code point, key sequence, or bracketed-paste marker.
pub(crate) fn pending_startup_input_with_prefix(
    timeout: Duration,
    initial_input: Vec<u8>,
    initial_input_truncated: bool,
) -> io::Result<Vec<StartupInput>> {
    let mut tty = Tty::open()?;
    let mut input = read_pending_startup_input(&mut tty, timeout, initial_input)?;
    if initial_input_truncated {
        input.push(StartupInput::UnknownAction);
    }
    Ok(input)
}

/// Reads available terminal bytes until `parse` recognizes a probe response or time expires.
///
/// The accumulated buffer may include unrelated terminal input. This helper intentionally does
/// not try to replay those bytes, so callers must use it only during short, exclusive probe
/// windows before normal crossterm input polling begins or while that polling is paused.
fn read_until<T>(
    tty: &mut Tty,
    timeout: Duration,
    mut parse: impl FnMut(&[u8]) -> Option<T>,
) -> io::Result<Option<T>> {
    let deadline = Instant::now() + timeout;
    let mut buffer = Vec::new();
    loop {
        tty.read_once(&mut buffer)?;
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
        input: Vec::new(),
    };
    let mut saw_supported_keyboard = false;
    let mut probe_done = false;
    let mut input_handoff_deadline = None;
    loop {
        let previous_len = buffer.len();
        tty.read_once(&mut buffer)?;
        let input_progressed = buffer.len() > previous_len;
        if !probe_done {
            update_startup_probe(
                &mut probe,
                &mut saw_supported_keyboard,
                &buffer,
                keyboard_probe,
            );
            probe_done = startup_probe_complete(&probe, keyboard_probe);
        }
        let now = Instant::now();
        if !probe_done && now >= deadline {
            finish_startup_probe(&mut probe, keyboard_probe, saw_supported_keyboard);
            probe_done = true;
        }

        if probe_done {
            let extracted = parse_startup_input(&buffer);
            if extracted.paste_open {
                if let Err(err) = finish_open_startup_paste(tty, &mut buffer) {
                    if err.kind() != io::ErrorKind::TimedOut {
                        return Err(err);
                    }
                    probe.input = extracted.input;
                    probe.input.push(StartupInput::UnknownAction);
                    return Ok(probe);
                }
                continue;
            }
            if extracted.complete {
                probe.input = extracted.input;
                return Ok(probe);
            }

            if input_progressed {
                input_handoff_deadline = Some(now + timeout);
            }
            let handoff_deadline = *input_handoff_deadline.get_or_insert_with(|| now + timeout);
            if now >= handoff_deadline
                || !tty.poll_readable(handoff_deadline.saturating_duration_since(now))?
            {
                if let Some(input) =
                    settle_incomplete_input(&buffer, IncompleteInputPhase::ProbeResponse)
                {
                    probe.input = input;
                    return Ok(probe);
                }
                probe.input = parse_startup_input(&buffer).input;
                probe.input.push(StartupInput::UnknownAction);
                return Ok(probe);
            }
            continue;
        }

        if !tty.poll_readable(deadline.saturating_duration_since(now))? {
            finish_startup_probe(&mut probe, keyboard_probe, saw_supported_keyboard);
            probe_done = true;
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
        // Startup first returns the cursor to column one. Requiring that known column keeps
        // response-shaped modified function keys such as Shift-F3 (`CSI 1;2R`) from being
        // mistaken for the DSR reply.
        probe.cursor_position = parse_cursor_position_with_column(buffer, Some(/*column*/ 1));
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

fn finish_open_startup_paste(tty: &mut Tty, buffer: &mut Vec<u8>) -> io::Result<()> {
    finish_open_startup_paste_with_timeout(tty, buffer, PASTE_COMPLETION_TIMEOUT)
}

fn finish_open_startup_paste_with_timeout(
    tty: &mut Tty,
    buffer: &mut Vec<u8>,
    inactivity_timeout: Duration,
) -> io::Result<()> {
    const PASTE_END: &[u8] = b"\x1b[201~";

    let mut deadline = Instant::now() + inactivity_timeout;
    let prefix_len = (1..PASTE_END.len())
        .rev()
        .find(|len| buffer.ends_with(&PASTE_END[..*len]))
        .unwrap_or(0);
    let mut candidate = buffer.split_off(buffer.len() - prefix_len);
    loop {
        let mut incoming = [0_u8; 256];
        let count = tty.read_into(&mut incoming)?;
        if count > 0 {
            deadline = Instant::now() + inactivity_timeout;
        }
        if append_open_startup_paste_chunk(buffer, &mut candidate, &incoming[..count]) {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline || !tty.poll_readable(deadline.saturating_duration_since(now))? {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "startup paste did not terminate",
            ));
        }
    }
}

fn append_open_startup_paste_chunk(
    buffer: &mut Vec<u8>,
    candidate: &mut Vec<u8>,
    incoming: &[u8],
) -> bool {
    const PASTE_END: &[u8] = b"\x1b[201~";

    for (index, byte) in incoming.iter().copied().enumerate() {
        candidate.push(byte);
        while !PASTE_END.starts_with(candidate.as_slice()) {
            let byte = candidate.remove(0);
            if buffer.len() < MAX_PROBE_BUFFER_BYTES - PASTE_END.len() {
                buffer.push(byte);
            }
        }
        if candidate == PASTE_END {
            if buffer.len() + PASTE_END.len() > MAX_PROBE_BUFFER_BYTES {
                buffer.truncate(MAX_PROBE_BUFFER_BYTES - PASTE_END.len());
            }
            buffer.extend_from_slice(PASTE_END);
            buffer.extend(
                incoming[index + 1..]
                    .iter()
                    .copied()
                    .take(MAX_PROBE_BUFFER_BYTES.saturating_sub(buffer.len())),
            );
            return true;
        }
    }
    false
}

fn parse_cursor_position(buffer: &[u8]) -> Option<Position> {
    parse_cursor_position_with_column(buffer, None)
}

fn parse_cursor_position_with_column(
    buffer: &[u8],
    expected_column: Option<u16>,
) -> Option<Position> {
    for start in find_all_subslices(buffer, b"\x1B[") {
        if is_inside_bracketed_paste(buffer, start) {
            continue;
        }
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
        if expected_column.is_some_and(|expected_column| col != expected_column) {
            continue;
        }
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
        if is_inside_bracketed_paste(buffer, start) {
            continue;
        }
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
        if is_inside_bracketed_paste(buffer, start) {
            continue;
        }
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
#[path = "unix_tests.rs"]
mod tests;
