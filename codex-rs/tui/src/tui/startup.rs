use std::io::IsTerminal;
use std::io::Result;
use std::io::stdin;
use std::io::stdout;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::fd::FromRawFd;
#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(any(unix, windows))]
use std::sync::Arc;
use std::sync::Mutex;
#[cfg(any(unix, windows))]
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::atomic::AtomicU64;
#[cfg(any(unix, windows))]
use std::sync::atomic::Ordering;
#[cfg(any(unix, windows))]
use std::thread::JoinHandle;
#[cfg(not(unix))]
use std::time::Duration;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;

use crate::key_hint::KeyBinding;

use super::CustomTerminal;
use super::InitializedTerminal;
use super::keyboard_modes;

pub(super) const MAX_STARTUP_INPUT_CHARS: usize =
    codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;

/// Discards terminal input if startup exits before the TUI takes ownership of it.
pub(crate) struct PreparedTerminal {
    active: bool,
    startup_input: StartupInputBuffer,
    startup_action_latch: StartupActionLatch,
}

impl Drop for PreparedTerminal {
    fn drop(&mut self) {
        if self.active {
            let _ = super::restore_after_exit_best_effort();
        }
    }
}

#[cfg(unix)]
struct StartupCaptureMode {
    original_termios: libc::termios,
    original_fd_flags: libc::c_int,
    original_file_status_flags: libc::c_int,
    owns_input: bool,
    terminal_mode: StartupTerminalMode,
    input_reader: Option<StartupInputReader>,
    input_reader_enabled: bool,
    captured_input: CapturedStartupBytes,
}

#[cfg(unix)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum StartupTerminalMode {
    Capture,
    FullRaw,
}

#[cfg(unix)]
const MAX_CAPTURED_STARTUP_BYTES: usize = MAX_STARTUP_INPUT_CHARS * 4 + 32 * 1024;
#[cfg(unix)]
const CAPTURED_STARTUP_TAIL_BYTES: usize = 64;

#[cfg(unix)]
#[derive(Default)]
struct CapturedStartupBytes {
    prefix: Vec<u8>,
    tail: std::collections::VecDeque<u8>,
    truncated: bool,
}

#[cfg(unix)]
impl CapturedStartupBytes {
    fn push(&mut self, bytes: &[u8]) {
        let prefix_remaining = MAX_CAPTURED_STARTUP_BYTES.saturating_sub(self.prefix.len());
        let (prefix, overflow) = bytes.split_at(bytes.len().min(prefix_remaining));
        self.prefix.extend_from_slice(prefix);
        if overflow.is_empty() {
            return;
        }
        self.truncated = true;
        for &byte in overflow {
            if self.tail.len() == CAPTURED_STARTUP_TAIL_BYTES {
                self.tail.pop_front();
            }
            self.tail.push_back(byte);
        }
    }

    fn merge(&mut self, other: Self) {
        self.push(&other.prefix);
        if other.truncated {
            self.truncated = true;
            for byte in other.tail {
                if self.tail.len() == CAPTURED_STARTUP_TAIL_BYTES {
                    self.tail.pop_front();
                }
                self.tail.push_back(byte);
            }
        }
    }

    fn into_parts(mut self) -> (Vec<u8>, bool) {
        let truncated = self.truncated;
        if truncated {
            self.prefix.extend(self.tail);
        }
        (self.prefix, truncated)
    }
}

#[cfg(unix)]
struct StartupInputReader {
    stop_writer: OwnedFd,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<CapturedStartupBytes>>,
}

#[cfg(unix)]
impl StartupInputReader {
    fn start() -> Result<Self> {
        let mut stop_pipe = [0; 2];
        if unsafe { libc::pipe(stop_pipe.as_mut_ptr()) } == -1 {
            return Err(std::io::Error::last_os_error());
        }
        let stop_reader = unsafe { OwnedFd::from_raw_fd(stop_pipe[0]) };
        let stop_writer = unsafe { OwnedFd::from_raw_fd(stop_pipe[1]) };
        for fd in [&stop_reader, &stop_writer] {
            let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
            if flags == -1
                || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) }
                    == -1
            {
                return Err(std::io::Error::last_os_error());
            }
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("codex-startup-input".to_string())
            .spawn(move || capture_startup_bytes(stop_reader, worker_shutdown))?;
        Ok(Self {
            stop_writer,
            shutdown,
            worker: Some(worker),
        })
    }

    fn stop(mut self) -> CapturedStartupBytes {
        self.finish()
    }

    fn finish(&mut self) -> CapturedStartupBytes {
        let Some(worker) = self.worker.take() else {
            return CapturedStartupBytes::default();
        };
        self.shutdown.store(true, Ordering::SeqCst);
        let byte = [1_u8];
        let _ = unsafe {
            libc::write(
                self.stop_writer.as_raw_fd(),
                byte.as_ptr().cast(),
                byte.len(),
            )
        };
        worker.join().unwrap_or_default()
    }
}

#[cfg(unix)]
impl Drop for StartupInputReader {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(unix)]
fn capture_startup_bytes(stop_reader: OwnedFd, shutdown: Arc<AtomicBool>) -> CapturedStartupBytes {
    let mut captured = CapturedStartupBytes::default();
    let mut chunk = [0_u8; 4096];
    loop {
        let mut poll_fds = [
            libc::pollfd {
                fd: stop_reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: libc::STDIN_FILENO,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let result = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as _, -1) };
        if result == -1 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        if poll_fds[0].revents != 0 || shutdown.load(Ordering::SeqCst) {
            break;
        }
        if poll_fds[1].revents & libc::POLLIN == 0 {
            if poll_fds[1].revents != 0 {
                break;
            }
            continue;
        }
        let read =
            unsafe { libc::read(libc::STDIN_FILENO, chunk.as_mut_ptr().cast(), chunk.len()) };
        if read > 0 {
            captured.push(&chunk[..read as usize]);
        } else if read == 0
            || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            break;
        }
    }
    captured
}

#[cfg(unix)]
fn pause_startup_input_reader(mode: &mut StartupCaptureMode) {
    if let Some(reader) = mode.input_reader.take() {
        mode.captured_input.merge(reader.stop());
    }
}

#[cfg(unix)]
fn resume_startup_input_reader(mode: &mut StartupCaptureMode) -> Result<()> {
    if mode.input_reader_enabled
        && mode.owns_input
        && mode.terminal_mode == StartupTerminalMode::Capture
        && mode.input_reader.is_none()
    {
        mode.input_reader = Some(StartupInputReader::start()?);
    }
    Ok(())
}

#[cfg(unix)]
fn take_captured_startup_bytes_locked() -> (Vec<u8>, bool) {
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return (Vec::new(), false);
    };
    pause_startup_input_reader(mode);
    std::mem::take(&mut mode.captured_input).into_parts()
}

#[cfg(windows)]
struct StartupCaptureMode {
    input_handle: windows_sys::Win32::Foundation::HANDLE,
    original_console_mode: u32,
    original_output_modes: Vec<(windows_sys::Win32::Foundation::HANDLE, u32)>,
    inherited: bool,
    owns_input: bool,
    input_reader: Option<StartupInputReader>,
    input_reader_enabled: bool,
    captured_input: CapturedStartupEvents,
}

#[cfg(windows)]
const MAX_CAPTURED_STARTUP_ACTION_EVENTS: usize = 32 * 1024;

#[cfg(windows)]
#[derive(Default)]
struct CapturedStartupEvents {
    events: Vec<Event>,
    text_chars: usize,
    action_events: usize,
    truncated: bool,
}

#[cfg(windows)]
impl CapturedStartupEvents {
    fn push(&mut self, event: Event) {
        let event = match event {
            Event::Paste(text) => {
                let remaining = MAX_STARTUP_INPUT_CHARS.saturating_sub(self.text_chars);
                let text = text.chars().take(remaining).collect::<String>();
                self.text_chars += text.chars().count();
                if self.text_chars == MAX_STARTUP_INPUT_CHARS {
                    self.truncated = true;
                }
                if text.is_empty() {
                    return;
                }
                Event::Paste(text)
            }
            Event::Key(key)
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && matches!(key.code, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Tab) =>
            {
                if self.text_chars == MAX_STARTUP_INPUT_CHARS {
                    self.truncated = true;
                    return;
                }
                self.text_chars += 1;
                Event::Key(key)
            }
            event => {
                if self.action_events == MAX_CAPTURED_STARTUP_ACTION_EVENTS {
                    self.truncated = true;
                    return;
                }
                self.action_events += 1;
                event
            }
        };
        self.events.push(event);
    }

    fn merge(&mut self, other: Self) {
        for event in other.events {
            self.push(event);
        }
        self.truncated |= other.truncated;
    }
}

#[cfg(any(test, windows))]
pub(super) fn coalesce_windows_startup_pastes(events: Vec<Event>) -> Vec<Event> {
    fn flush_candidate(output: &mut Vec<Event>, candidate: &mut Vec<Event>, text: &mut String) {
        if candidate.len() > 1 && text.contains('\n') {
            output.push(Event::Paste(std::mem::take(text)));
            candidate.clear();
        } else {
            output.append(candidate);
            text.clear();
        }
    }

    let mut output = Vec::new();
    let mut candidate = Vec::new();
    let mut text = String::new();
    for event in events {
        let candidate_char = match event {
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) if !crate::key_hint::has_ctrl_or_alt(modifiers) => match code {
                KeyCode::Char(ch) if !ch.is_control() => Some(ch),
                KeyCode::Enter => Some('\n'),
                KeyCode::Tab => Some('\t'),
                _ => None,
            },
            _ => None,
        };
        if let Some(ch) = candidate_char {
            text.push(ch);
            candidate.push(event);
        } else {
            flush_candidate(&mut output, &mut candidate, &mut text);
            output.push(event);
        }
    }
    flush_candidate(&mut output, &mut candidate, &mut text);
    output
}

#[cfg(windows)]
struct StartupInputReader {
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<CapturedStartupEvents>>,
}

#[cfg(windows)]
impl StartupInputReader {
    fn start() -> Result<Self> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("codex-startup-input".to_string())
            .spawn(move || {
                let mut captured = CapturedStartupEvents::default();
                while !worker_shutdown.load(Ordering::SeqCst) {
                    match crossterm::event::poll(Duration::from_millis(/*millis*/ 25)) {
                        Ok(true) => match crossterm::event::read() {
                            Ok(event) => captured.push(event),
                            Err(_) => break,
                        },
                        Ok(false) => {}
                        Err(_) => break,
                    }
                }
                captured
            })?;
        Ok(Self {
            shutdown,
            worker: Some(worker),
        })
    }

    fn finish(&mut self) -> CapturedStartupEvents {
        let Some(worker) = self.worker.take() else {
            return CapturedStartupEvents::default();
        };
        self.shutdown.store(true, Ordering::SeqCst);
        worker.join().unwrap_or_default()
    }

    fn stop(mut self) -> CapturedStartupEvents {
        self.finish()
    }
}

#[cfg(windows)]
impl Drop for StartupInputReader {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(windows)]
fn pause_startup_input_reader(mode: &mut StartupCaptureMode) {
    if let Some(reader) = mode.input_reader.take() {
        mode.captured_input.merge(reader.stop());
    }
}

#[cfg(windows)]
fn resume_startup_input_reader(mode: &mut StartupCaptureMode) -> Result<()> {
    if mode.input_reader_enabled && mode.owns_input && mode.input_reader.is_none() {
        mode.input_reader = Some(StartupInputReader::start()?);
    }
    Ok(())
}

#[cfg(windows)]
fn take_captured_startup_events_locked() -> CapturedStartupEvents {
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return CapturedStartupEvents::default();
    };
    pause_startup_input_reader(mode);
    std::mem::take(&mut mode.captured_input)
}

#[cfg(not(any(unix, windows)))]
struct StartupCaptureMode;

static STARTUP_CAPTURE_MODE: Mutex<Option<StartupCaptureMode>> = Mutex::new(None);
#[cfg(any(unix, windows))]
static SIGNAL_RESTORE_INSTALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(any(unix, windows))]
struct SignalRestoreRegistration;

#[cfg(any(unix, windows))]
impl Drop for SignalRestoreRegistration {
    fn drop(&mut self) {
        SIGNAL_RESTORE_INSTALLED.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
#[cfg(unix)]
static MANAGED_CONTINUE_PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
static NEXT_SIGNAL_SUSPEND_REGISTRATION: AtomicU64 = AtomicU64::new(1);
#[cfg(unix)]
static SIGNAL_SUSPEND_CONTEXT: Mutex<Option<(u64, SignalSuspendContext)>> = Mutex::new(None);

#[cfg(unix)]
#[derive(Clone)]
struct SignalSuspendContext {
    event_broker: Arc<super::event_stream::EventBroker>,
    suspend_context: super::job_control::SuspendContext,
    alt_screen_active: Arc<AtomicBool>,
    frame_requester: super::FrameRequester,
    active: Arc<AtomicBool>,
    operation: Arc<Mutex<()>>,
    external_owner: Arc<AtomicBool>,
}

#[cfg(unix)]
pub(super) struct SignalSuspendRegistration {
    id: u64,
    active: Arc<AtomicBool>,
    operation: Arc<Mutex<()>>,
    external_owner: Arc<AtomicBool>,
}

#[cfg(unix)]
impl SignalSuspendRegistration {
    pub(super) fn synchronization(&self) -> (Arc<Mutex<()>>, Arc<AtomicBool>) {
        (self.operation.clone(), self.external_owner.clone())
    }
}

#[cfg(unix)]
pub(super) fn register_signal_suspend_context(
    event_broker: Arc<super::event_stream::EventBroker>,
    suspend_context: super::job_control::SuspendContext,
    alt_screen_active: Arc<AtomicBool>,
    frame_requester: super::FrameRequester,
) -> SignalSuspendRegistration {
    let id = NEXT_SIGNAL_SUSPEND_REGISTRATION.fetch_add(1, Ordering::Relaxed);
    let active = Arc::new(AtomicBool::new(true));
    let operation = Arc::new(Mutex::new(()));
    let external_owner = Arc::new(AtomicBool::new(false));
    *SIGNAL_SUSPEND_CONTEXT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((
        id,
        SignalSuspendContext {
            event_broker,
            suspend_context,
            alt_screen_active,
            frame_requester,
            active: active.clone(),
            operation: operation.clone(),
            external_owner: external_owner.clone(),
        },
    ));
    SignalSuspendRegistration {
        id,
        active,
        operation,
        external_owner,
    }
}

#[cfg(unix)]
pub(super) fn unregister_signal_suspend_context(registration: SignalSuspendRegistration) {
    registration.active.store(false, Ordering::SeqCst);
    let _operation = registration
        .operation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut context = SIGNAL_SUSPEND_CONTEXT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if context
        .as_ref()
        .is_some_and(|(registered, _)| *registered == registration.id)
    {
        context.take();
    }
}

pub(super) fn has_startup_capture_mode() -> bool {
    STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
}

#[cfg(unix)]
fn enable_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "startup terminal capture is already active",
        ));
    }
    install_signal_restore()?;

    let mut original_termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut original_termios) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    let original_fd_flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFD) };
    if original_fd_flags == -1 {
        return Err(std::io::Error::last_os_error());
    }
    let original_file_status_flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) };
    if original_file_status_flags == -1 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe {
        libc::fcntl(
            libc::STDIN_FILENO,
            libc::F_SETFD,
            original_fd_flags | libc::FD_CLOEXEC,
        )
    } == -1
    {
        return Err(std::io::Error::last_os_error());
    }

    let mut capture_termios = original_termios;
    capture_termios.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHONL);
    capture_termios.c_cc[libc::VMIN] = 1;
    capture_termios.c_cc[libc::VTIME] = 0;
    if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &capture_termios) } == -1 {
        let err = std::io::Error::last_os_error();
        let _ = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, original_fd_flags) };
        return Err(err);
    }

    let input_reader = match StartupInputReader::start() {
        Ok(reader) => reader,
        Err(err) => {
            let _ =
                unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &original_termios) };
            let _ = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, original_fd_flags) };
            return Err(err);
        }
    };

    *state = Some(StartupCaptureMode {
        original_termios,
        original_fd_flags,
        original_file_status_flags,
        owns_input: true,
        terminal_mode: StartupTerminalMode::Capture,
        input_reader: Some(input_reader),
        input_reader_enabled: true,
        captured_input: CapturedStartupBytes::default(),
    });
    Ok(())
}

#[cfg(unix)]
fn install_signal_restore() -> Result<()> {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return Ok(());
    };
    if SIGNAL_RESTORE_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }
    let signals = (|| {
        Ok((
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?,
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::quit())?,
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?,
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::from_raw(libc::SIGTSTP))?,
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::from_raw(libc::SIGCONT))?,
        ))
    })();
    let (mut interrupt, mut quit, mut terminate, mut hangup, mut suspend, mut resume) =
        match signals {
            Ok(signals) => signals,
            Err(err) => {
                SIGNAL_RESTORE_INSTALLED.store(false, Ordering::SeqCst);
                return Err(err);
            }
        };
    let registration = SignalRestoreRegistration;
    runtime.spawn(async move {
        let _registration = registration;
        loop {
            let code = tokio::select! {
                signal = interrupt.recv() => signal.map(|()| 130),
                signal = quit.recv() => signal.map(|()| 131),
                signal = terminate.recv() => signal.map(|()| 143),
                signal = hangup.recv() => signal.map(|()| 129),
                signal = suspend.recv() => {
                    if signal.is_some() {
                        suspend_for_signal();
                    }
                    None
                }
                signal = resume.recv() => {
                    if signal.is_some() {
                        reapply_capture_after_continue();
                    }
                    None
                }
            };
            if let Some(code) = code {
                super::exit_after_terminal_signal(code);
            }
        }
    });
    Ok(())
}

#[cfg(unix)]
fn suspend_for_signal() {
    let signal_context = SIGNAL_SUSPEND_CONTEXT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|(_, context)| context.clone());
    let _signal_operation = signal_context.as_ref().map(|context| {
        context
            .operation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    });
    if signal_context
        .as_ref()
        .is_some_and(|context| !context.active.load(Ordering::SeqCst))
    {
        return;
    }
    if signal_context
        .as_ref()
        .is_some_and(|context| context.external_owner.load(Ordering::SeqCst))
    {
        expect_managed_continue();
        if unsafe { libc::kill(std::process::id() as libc::pid_t, libc::SIGSTOP) } == -1 {
            cancel_managed_continue();
            tracing::warn!(
                "failed to suspend alongside external terminal owner: {}",
                std::io::Error::last_os_error()
            );
        }
        return;
    }

    let event_broker_paused = signal_context
        .as_ref()
        .is_some_and(|context| context.event_broker.pause_running_events());
    let lifecycle = super::terminal_lifecycle_guard();
    let terminal_mode = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|mode| mode.terminal_mode);
    if terminal_mode == Some(StartupTerminalMode::FullRaw)
        && let Some(context) = signal_context.as_ref()
    {
        let result = context
            .suspend_context
            .suspend_locked(&context.alt_screen_active);
        drop(lifecycle);
        if event_broker_paused {
            context.event_broker.resume_events();
        }
        context.frame_requester.schedule_frame();
        if let Err(err) = result {
            tracing::warn!("failed to suspend TUI after SIGTSTP: {err}");
        }
        return;
    };

    let Some(terminal_mode) = terminal_mode else {
        if unsafe { libc::kill(std::process::id() as libc::pid_t, libc::SIGSTOP) } == -1 {
            tracing::warn!(
                "failed to suspend after SIGTSTP: {}",
                std::io::Error::last_os_error()
            );
        }
        drop(lifecycle);
        if let Some(context) = signal_context.as_ref() {
            if event_broker_paused {
                context.event_broker.resume_events();
            }
            context.frame_requester.schedule_frame();
        }
        return;
    };

    let was_alt_screen = terminal_mode == StartupTerminalMode::FullRaw
        && super::ALT_SCREEN_OWNED.load(Ordering::SeqCst);
    let mut first_error = None;
    if was_alt_screen {
        if let Err(err) = execute!(stdout(), super::DisableAlternateScroll) {
            first_error = Some(err);
        }
        match execute!(stdout(), crossterm::terminal::LeaveAlternateScreen) {
            Ok(()) => super::note_alt_screen_left(),
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }
    match terminal_mode {
        StartupTerminalMode::Capture => {
            if let Err(err) = execute!(stdout(), DisableBracketedPaste) {
                first_error.get_or_insert(err);
            }
        }
        StartupTerminalMode::FullRaw => {
            if let Err(err) = super::restore_common(
                super::RawModeRestore::Disable,
                super::KeyboardRestore::PopStack,
            ) {
                first_error.get_or_insert(err);
            }
        }
    }
    if let Err(err) = restore_startup_capture_mode() {
        first_error.get_or_insert(err);
    }
    if let Err(err) = super::terminal_stderr::pause() {
        first_error.get_or_insert(err);
    }

    expect_managed_continue();
    if unsafe { libc::kill(std::process::id() as libc::pid_t, libc::SIGSTOP) } == -1 {
        cancel_managed_continue();
        first_error.get_or_insert_with(std::io::Error::last_os_error);
    }

    if let Err(err) = super::terminal_stderr::resume() {
        first_error.get_or_insert(err);
    }
    if let Err(err) = reapply_startup_capture_mode_locked() {
        first_error.get_or_insert(err);
    }
    match terminal_mode {
        StartupTerminalMode::Capture => {
            if let Err(err) = execute!(stdout(), EnableBracketedPaste) {
                first_error.get_or_insert(err);
            }
        }
        StartupTerminalMode::FullRaw => {
            if let Err(err) = super::set_modes_unlocked() {
                first_error.get_or_insert(err);
            }
        }
    }
    if was_alt_screen {
        match execute!(
            stdout(),
            crossterm::terminal::EnterAlternateScreen,
            super::EnableAlternateScroll
        ) {
            Ok(()) => super::note_alt_screen_entered(),
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }
    drop(lifecycle);
    if let Some(context) = signal_context.as_ref() {
        if event_broker_paused {
            context.event_broker.resume_events();
        }
        context.frame_requester.schedule_frame();
    }
    if let Some(err) = first_error {
        tracing::warn!("failed to fully restore terminal state around SIGTSTP: {err}");
    }
}

#[cfg(windows)]
fn install_signal_restore() -> Result<()> {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return Ok(());
    };
    use std::sync::atomic::Ordering;

    if SIGNAL_RESTORE_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }
    let signals = (|| {
        Ok((
            tokio::signal::windows::ctrl_c()?,
            tokio::signal::windows::ctrl_break()?,
        ))
    })();
    let (mut ctrl_c, mut ctrl_break) = match signals {
        Ok(signals) => signals,
        Err(err) => {
            SIGNAL_RESTORE_INSTALLED.store(false, Ordering::SeqCst);
            return Err(err);
        }
    };
    let registration = SignalRestoreRegistration;
    runtime.spawn(async move {
        let _registration = registration;
        let code = tokio::select! {
            signal = ctrl_c.recv() => signal.map(|()| 130),
            signal = ctrl_break.recv() => signal.map(|()| 131),
        };
        if let Some(code) = code {
            super::exit_after_terminal_signal(code);
        }
    });
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn install_signal_restore() -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn enable_startup_capture_mode() -> Result<()> {
    use windows_sys::Win32::Foundation::GetHandleInformation;
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Foundation::SetHandleInformation;
    use windows_sys::Win32::System::Console::ENABLE_ECHO_INPUT;
    use windows_sys::Win32::System::Console::ENABLE_LINE_INPUT;
    use windows_sys::Win32::System::Console::ENABLE_PROCESSED_INPUT;
    use windows_sys::Win32::System::Console::ENABLE_WINDOW_INPUT;
    use windows_sys::Win32::System::Console::GetConsoleMode;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
    use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let _lifecycle = super::terminal_lifecycle_guard();
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "startup terminal capture is already active",
        ));
    }
    install_signal_restore()?;

    let input_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if input_handle == INVALID_HANDLE_VALUE || input_handle == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut original_console_mode = 0;
    if unsafe { GetConsoleMode(input_handle, &mut original_console_mode) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut handle_flags = 0;
    if unsafe { GetHandleInformation(input_handle, &mut handle_flags) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let inherited = handle_flags & HANDLE_FLAG_INHERIT != 0;
    if inherited && unsafe { SetHandleInformation(input_handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let capture_mode = (original_console_mode | ENABLE_PROCESSED_INPUT | ENABLE_WINDOW_INPUT)
        & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT);
    if unsafe { SetConsoleMode(input_handle, capture_mode) } == 0 {
        let err = std::io::Error::last_os_error();
        if inherited {
            let _ = unsafe {
                SetHandleInformation(input_handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
            };
        }
        return Err(err);
    }

    let mut original_output_modes = Vec::new();
    for standard_handle in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        let output_handle = unsafe { GetStdHandle(standard_handle) };
        if output_handle == INVALID_HANDLE_VALUE
            || output_handle == 0
            || original_output_modes
                .iter()
                .any(|(handle, _)| *handle == output_handle)
        {
            continue;
        }
        let mut output_mode = 0;
        if unsafe { GetConsoleMode(output_handle, &mut output_mode) } != 0 {
            original_output_modes.push((output_handle, output_mode));
        }
    }

    let input_reader = match StartupInputReader::start() {
        Ok(reader) => reader,
        Err(err) => {
            let _ = unsafe { SetConsoleMode(input_handle, original_console_mode) };
            if inherited {
                let _ = unsafe {
                    SetHandleInformation(input_handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
                };
            }
            return Err(err);
        }
    };

    *state = Some(StartupCaptureMode {
        input_handle,
        original_console_mode,
        original_output_modes,
        inherited,
        owns_input: true,
        input_reader: Some(input_reader),
        input_reader_enabled: true,
        captured_input: CapturedStartupEvents::default(),
    });
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn enable_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    install_signal_restore()?;
    *STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(StartupCaptureMode);
    Ok(())
}

#[cfg(unix)]
pub(super) fn restore_startup_capture_mode() -> Result<()> {
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    restore_original_startup_input(mode)
}

#[cfg(unix)]
fn restore_original_startup_input(mode: &mut StartupCaptureMode) -> Result<()> {
    pause_startup_input_reader(mode);
    let mut first_error = None;
    if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &mode.original_termios) } == -1 {
        first_error = Some(std::io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, mode.original_fd_flags) } == -1 {
        first_error.get_or_insert_with(std::io::Error::last_os_error);
    }
    if unsafe {
        libc::fcntl(
            libc::STDIN_FILENO,
            libc::F_SETFL,
            mode.original_file_status_flags,
        )
    } == -1
    {
        first_error.get_or_insert_with(std::io::Error::last_os_error);
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(windows)]
pub(super) fn restore_startup_capture_mode() -> Result<()> {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    let mut first_error = restore_original_startup_input(mode).err();
    for &(output_handle, output_mode) in &mode.original_output_modes {
        if unsafe { SetConsoleMode(output_handle, output_mode) } == 0 {
            first_error.get_or_insert_with(std::io::Error::last_os_error);
        }
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(windows)]
fn restore_original_startup_input(mode: &mut StartupCaptureMode) -> Result<()> {
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::Foundation::SetHandleInformation;
    use windows_sys::Win32::System::Console::SetConsoleMode;

    pause_startup_input_reader(mode);
    let mut first_error = None;
    if unsafe { SetConsoleMode(mode.input_handle, mode.original_console_mode) } == 0 {
        first_error = Some(std::io::Error::last_os_error());
    }
    if mode.inherited
        && unsafe {
            SetHandleInformation(mode.input_handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
        } == 0
    {
        first_error.get_or_insert_with(std::io::Error::last_os_error);
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn restore_startup_capture_mode() -> Result<()> {
    Ok(())
}

pub(super) fn finish_startup_capture_restore() {
    let _ = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
}

#[cfg(unix)]
fn reapply_capture_after_continue() {
    if MANAGED_CONTINUE_PENDING.swap(false, Ordering::SeqCst) {
        return;
    }
    let _lifecycle = super::terminal_lifecycle_guard();
    let capture_state = {
        let state = STARTUP_CAPTURE_MODE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .as_ref()
            .filter(|mode| mode.owns_input)
            .map(|mode| mode.terminal_mode)
    };
    let Some(terminal_mode) = capture_state else {
        return;
    };
    match terminal_mode {
        StartupTerminalMode::Capture => {
            if let Err(err) = reapply_startup_capture_mode_locked() {
                tracing::warn!("failed to restore startup terminal capture after resume: {err}");
            }
            if let Err(err) = execute!(stdout(), EnableBracketedPaste) {
                tracing::warn!("failed to restore bracketed paste after resume: {err}");
            }
        }
        StartupTerminalMode::FullRaw => {
            if let Err(err) = super::reapply_raw_mode_after_continue_unlocked() {
                tracing::warn!("failed to restore raw terminal mode after resume: {err}");
            }
        }
    }
}

#[cfg(unix)]
pub(super) fn note_full_terminal_modes() {
    if let Some(mode) = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
    {
        mode.terminal_mode = StartupTerminalMode::FullRaw;
    }
}

#[cfg(not(unix))]
pub(super) fn note_full_terminal_modes() {}

#[cfg(unix)]
pub(super) fn note_capture_terminal_mode() {
    if let Some(mode) = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
    {
        mode.terminal_mode = StartupTerminalMode::Capture;
    }
}

#[cfg(not(unix))]
pub(super) fn note_capture_terminal_mode() {}

#[cfg(unix)]
pub(super) fn expect_managed_continue() {
    MANAGED_CONTINUE_PENDING.store(true, Ordering::SeqCst);
}

#[cfg(unix)]
pub(super) fn cancel_managed_continue() {
    MANAGED_CONTINUE_PENDING.store(false, Ordering::SeqCst);
}

#[cfg(unix)]
pub(super) fn finish_startup_input_capture() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    if !mode.owns_input {
        return Ok(());
    }
    pause_startup_input_reader(mode);
    mode.input_reader_enabled = false;
    if unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, mode.original_fd_flags) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    mode.owns_input = false;
    Ok(())
}

#[cfg(windows)]
pub(super) fn finish_startup_input_capture() -> Result<()> {
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::Foundation::SetHandleInformation;

    let _lifecycle = super::terminal_lifecycle_guard();
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    if !mode.owns_input {
        return Ok(());
    }
    pause_startup_input_reader(mode);
    mode.input_reader_enabled = false;
    if mode.inherited
        && unsafe {
            SetHandleInformation(mode.input_handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
        } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    mode.owns_input = false;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(super) fn finish_startup_input_capture() -> Result<()> {
    Ok(())
}

#[cfg(any(unix, windows))]
pub(super) fn pause_startup_input_capture_for_full_modes() {
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(mode) = state.as_mut() {
        mode.input_reader_enabled = false;
        pause_startup_input_reader(mode);
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn pause_startup_input_capture_for_full_modes() {}

/// Temporarily restore the caller's terminal state without relinquishing startup ownership.
///
/// Startup screens enter raw mode on top of the capture mode, so ordinary screen teardown should
/// leave capture active. External programs and job-control suspension are different: they need the
/// exact caller state while they own the terminal, then capture mode must be reapplied on return.
#[cfg(unix)]
pub(super) fn temporarily_restore_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    temporarily_restore_startup_capture_mode_locked()
}

#[cfg(unix)]
pub(super) fn temporarily_restore_startup_capture_mode_locked() -> Result<()> {
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    restore_original_startup_input(mode)
}

#[cfg(windows)]
pub(super) fn temporarily_restore_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    restore_original_startup_input(mode)
}

#[cfg(not(any(unix, windows)))]
pub(super) fn temporarily_restore_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    Ok(())
}

#[cfg(unix)]
pub(super) fn reapply_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    reapply_startup_capture_mode_locked()
}

#[cfg(unix)]
pub(super) fn reapply_startup_capture_mode_locked() -> Result<()> {
    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    let mut capture_termios = mode.original_termios;
    capture_termios.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHONL);
    capture_termios.c_cc[libc::VMIN] = 1;
    capture_termios.c_cc[libc::VTIME] = 0;
    let fd_flags = if mode.owns_input {
        mode.original_fd_flags | libc::FD_CLOEXEC
    } else {
        mode.original_fd_flags
    };
    if unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, fd_flags) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe {
        libc::fcntl(
            libc::STDIN_FILENO,
            libc::F_SETFL,
            mode.original_file_status_flags,
        )
    } == -1
    {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &capture_termios) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    resume_startup_input_reader(mode)?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn reapply_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    reapply_startup_capture_mode_locked()
}

#[cfg(windows)]
pub(super) fn reapply_startup_capture_mode_locked() -> Result<()> {
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::Foundation::SetHandleInformation;
    use windows_sys::Win32::System::Console::ENABLE_ECHO_INPUT;
    use windows_sys::Win32::System::Console::ENABLE_LINE_INPUT;
    use windows_sys::Win32::System::Console::ENABLE_PROCESSED_INPUT;
    use windows_sys::Win32::System::Console::ENABLE_WINDOW_INPUT;
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let mut state = STARTUP_CAPTURE_MODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mode) = state.as_mut() else {
        return Ok(());
    };
    if mode.owns_input
        && mode.inherited
        && unsafe { SetHandleInformation(mode.input_handle, HANDLE_FLAG_INHERIT, 0) } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let capture_mode = (mode.original_console_mode | ENABLE_PROCESSED_INPUT | ENABLE_WINDOW_INPUT)
        & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT);
    if unsafe { SetConsoleMode(mode.input_handle, capture_mode) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    resume_startup_input_reader(mode)?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(super) fn reapply_startup_capture_mode() -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    Ok(())
}

#[derive(Default)]
pub(crate) struct StartupInputBuffer {
    text: String,
    char_count: usize,
    typed_text_actions: Vec<(usize, StartupBlockedAction)>,
    repeat_actions: Vec<StartupBlockedAction>,
    pending_plain_whitespace: String,
    pending_plain_whitespace_actions: Vec<StartupBlockedAction>,
    trailing_printable_action: Option<(KeyBinding, bool)>,
    active_actions: Vec<StartupBlockedAction>,
    quarantined_actions: Vec<StartupBlockedAction>,
    unknown_action_seen: bool,
    interrupt_requested: bool,
    suspend_requested: bool,
    restored_text: bool,
}

#[derive(Default)]
pub(super) struct StartupInputHandoff {
    pub(super) claimed: bool,
    pub(super) interrupt_requested: bool,
    pub(super) suspend_requested: bool,
    pub(super) resume_draw_requested: bool,
    pub(super) pending_plain_whitespace: String,
    pub(super) pending_plain_whitespace_actions: Vec<StartupBlockedAction>,
    pub(super) trailing_printable_action: Option<(KeyBinding, bool)>,
    pub(super) quarantined_actions: Vec<StartupBlockedAction>,
    pub(super) repeat_actions: Vec<StartupBlockedAction>,
    pub(super) unknown_action_seen: bool,
    pub(super) restored_text: bool,
    pub(super) submission_bindings: Vec<KeyBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StartupBlockedAction {
    pub(super) binding: KeyBinding,
    pub(super) from_raw_probe: bool,
    pub(super) release_observed: bool,
    pub(super) quiet_elapsed: bool,
    pub(super) preserve_after_quiet: bool,
}

impl StartupBlockedAction {
    fn from_screen(binding: KeyBinding, quiet_elapsed: bool) -> Self {
        Self {
            binding,
            from_raw_probe: false,
            release_observed: false,
            quiet_elapsed,
            preserve_after_quiet: is_plain_printable(binding),
        }
    }

    pub(super) fn captured(binding: KeyBinding, from_raw_probe: bool) -> Self {
        Self {
            binding,
            from_raw_probe,
            release_observed: false,
            quiet_elapsed: false,
            preserve_after_quiet: is_plain_printable(binding),
        }
    }
}

#[derive(Default)]
pub(super) struct StartupActionLatch {
    blocked: Vec<StartupBlockedAction>,
}

impl StartupActionLatch {
    pub(super) fn record(&mut self, key_event: KeyEvent) {
        let binding = KeyBinding::from_event(key_event);
        match key_event.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => {
                let action =
                    StartupBlockedAction::from_screen(binding, /*quiet_elapsed*/ false);
                if let Some(existing) = self
                    .blocked
                    .iter_mut()
                    .find(|existing| existing.binding == binding)
                {
                    *existing = action;
                } else {
                    self.blocked.push(action);
                }
            }
            KeyEventKind::Release => {
                self.blocked.retain(|blocked| blocked.binding != binding);
            }
        }
    }

    pub(super) fn retain_blocked(&mut self, actions: Vec<StartupBlockedAction>) {
        for action in actions {
            if let Some(existing) = self
                .blocked
                .iter_mut()
                .find(|existing| existing.binding == action.binding)
            {
                *existing = action;
            } else {
                self.blocked.push(action);
            }
        }
    }

    pub(super) fn drain_into(&mut self, input: &mut StartupInputBuffer) -> bool {
        let blocked = std::mem::take(&mut self.blocked);
        for action in blocked.iter().copied() {
            input.quarantine_action(action);
        }
        !blocked.is_empty()
    }

    pub(super) fn note_input_drained(&mut self) {
        for action in &mut self.blocked {
            action.quiet_elapsed = true;
        }
    }
}

impl StartupInputBuffer {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(
                key_event @ KeyEvent {
                    code,
                    modifiers,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                },
            ) => {
                let binding = KeyBinding::from_event(key_event);
                if let Some(index) = self.quarantined_actions.iter().position(|action| {
                    startup_action_matches(action.binding, action.from_raw_probe, binding)
                }) {
                    let action = self.quarantined_actions[index];
                    if !action.quiet_elapsed || !action.preserve_after_quiet {
                        if action.quiet_elapsed {
                            self.quarantined_actions.remove(index);
                        }
                        return;
                    }
                    self.quarantined_actions.remove(index);
                }
                if code == KeyCode::Char('c')
                    && modifiers.contains(KeyModifiers::CONTROL)
                    && !crate::key_hint::is_altgr(modifiers)
                {
                    self.record_active_action(binding, /*from_raw_probe*/ false);
                    self.clear_pending_plain_whitespace();
                    self.interrupt_requested = true;
                    return;
                }
                if code == KeyCode::Char('z')
                    && modifiers.contains(KeyModifiers::CONTROL)
                    && !crate::key_hint::is_altgr(modifiers)
                {
                    self.record_active_action(binding, /*from_raw_probe*/ false);
                    self.clear_pending_plain_whitespace();
                    self.suspend_requested = true;
                    return;
                }
                if !crate::key_hint::has_ctrl_or_alt(modifiers) {
                    match code {
                        KeyCode::Char(ch) if !ch.is_control() => {
                            self.trailing_printable_action = Some((binding, false));
                            self.push_plain_char(ch, binding, /*from_raw_probe*/ false);
                        }
                        KeyCode::Backspace => {
                            self.record_active_action(binding, /*from_raw_probe*/ false);
                            self.pop_char();
                        }
                        KeyCode::Enter => {
                            self.record_active_action(binding, /*from_raw_probe*/ false);
                            self.push_pending_plain_whitespace(
                                '\n', binding, /*from_raw_probe*/ false,
                            );
                        }
                        KeyCode::Tab => {
                            self.record_active_action(binding, /*from_raw_probe*/ false);
                            self.push_pending_plain_whitespace(
                                '\t', binding, /*from_raw_probe*/ false,
                            );
                        }
                        _ => {
                            self.record_active_action(binding, /*from_raw_probe*/ false);
                            self.clear_pending_plain_whitespace();
                        }
                    }
                } else {
                    self.record_active_action(binding, /*from_raw_probe*/ false);
                    self.clear_pending_plain_whitespace();
                }
            }
            Event::Key(
                key_event @ KeyEvent {
                    kind: KeyEventKind::Release,
                    ..
                },
            ) => {
                let binding = KeyBinding::from_event(key_event);
                self.quarantined_actions.retain(|action| {
                    !startup_action_matches(action.binding, action.from_raw_probe, binding)
                });
                self.active_actions.retain(|action| {
                    !startup_action_matches(action.binding, action.from_raw_probe, binding)
                });
                self.repeat_actions.retain(|action| {
                    !startup_action_matches(action.binding, action.from_raw_probe, binding)
                });
                for action in self
                    .pending_plain_whitespace_actions
                    .iter_mut()
                    .chain(self.typed_text_actions.iter_mut().map(|(_, action)| action))
                {
                    if startup_action_matches(action.binding, action.from_raw_probe, binding) {
                        action.release_observed = true;
                    }
                }
                if self
                    .trailing_printable_action
                    .is_some_and(|(action, from_raw_probe)| {
                        startup_action_matches(action, from_raw_probe, binding)
                    })
                {
                    self.trailing_printable_action = None;
                }
            }
            Event::Paste(text) => self.push_text(&text),
            _ => {}
        }
    }

    fn push_char(&mut self, ch: char) -> bool {
        if self.char_count < MAX_STARTUP_INPUT_CHARS {
            self.text.push(ch);
            self.char_count += 1;
            true
        } else {
            false
        }
    }

    fn push_plain_char(&mut self, ch: char, binding: KeyBinding, from_raw_probe: bool) {
        self.commit_pending_plain_whitespace();
        let char_index = self.char_count;
        if self.push_char(ch) {
            self.typed_text_actions.push((
                char_index,
                StartupBlockedAction::captured(binding, from_raw_probe),
            ));
        }
    }

    fn push_pending_plain_whitespace(
        &mut self,
        ch: char,
        binding: KeyBinding,
        from_raw_probe: bool,
    ) {
        if self.char_count + self.pending_plain_whitespace.len() < MAX_STARTUP_INPUT_CHARS {
            self.pending_plain_whitespace.push(ch);
            self.pending_plain_whitespace_actions
                .push(StartupBlockedAction::captured(binding, from_raw_probe));
        }
    }

    fn commit_pending_plain_whitespace(&mut self) {
        let pending = std::mem::take(&mut self.pending_plain_whitespace);
        let actions = std::mem::take(&mut self.pending_plain_whitespace_actions);
        debug_assert_eq!(pending.chars().count(), actions.len());
        for (ch, mut action) in pending.chars().zip(actions) {
            // Once later text commits this whitespace into the draft, repeats should follow the
            // ordinary-text path. The submission filter still quarantines an actual submit key.
            action.preserve_after_quiet = true;
            let char_index = self.char_count;
            if self.push_char(ch) && !action.release_observed {
                self.typed_text_actions.push((char_index, action));
            }
        }
    }

    fn clear_pending_plain_whitespace(&mut self) {
        self.pending_plain_whitespace.clear();
        self.pending_plain_whitespace_actions.clear();
    }

    fn pop_char(&mut self) {
        if self.pending_plain_whitespace.pop().is_some() {
            self.pending_plain_whitespace_actions.pop();
            return;
        }
        if let Some((grapheme_start, grapheme)) =
            unicode_segmentation::UnicodeSegmentation::grapheme_indices(
                self.text.as_str(),
                /*is_extended*/ true,
            )
            .next_back()
        {
            let removed_char_count = grapheme.chars().count();
            self.text.truncate(grapheme_start);
            self.char_count -= removed_char_count;
            if let Some((_, popped_action)) = self
                .typed_text_actions
                .iter()
                .rev()
                .find(|(char_index, _)| *char_index >= self.char_count)
                && self
                    .trailing_printable_action
                    .is_some_and(|(binding, from_raw_probe)| {
                        popped_action.binding == binding
                            && popped_action.from_raw_probe == from_raw_probe
                    })
            {
                self.trailing_printable_action = None;
            }
            self.typed_text_actions
                .retain(|(char_index, _)| *char_index < self.char_count);
        }
    }

    pub(super) fn push_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.commit_pending_plain_whitespace();
        }
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    self.push_char('\n');
                }
                '\n' | '\t' => {
                    self.push_char(ch);
                }
                ch if !ch.is_control() => {
                    self.push_char(ch);
                }
                _ => {}
            }
        }
    }

    pub(super) fn quarantine_action(&mut self, action: StartupBlockedAction) {
        self.remember_quarantined_action(action);
        self.clear_pending_plain_whitespace();
    }

    fn remember_quarantined_action(&mut self, action: StartupBlockedAction) {
        if let Some(existing) = self
            .quarantined_actions
            .iter_mut()
            .find(|existing| existing.binding == action.binding)
        {
            *existing = action;
        } else {
            self.quarantined_actions.push(action);
        }
    }

    fn record_active_action(&mut self, binding: KeyBinding, from_raw_probe: bool) {
        let action = StartupBlockedAction::captured(binding, from_raw_probe);
        if let Some(existing) = self
            .active_actions
            .iter_mut()
            .find(|existing| existing.binding == binding)
        {
            *existing = action;
        } else {
            self.active_actions.push(action);
        }
    }

    pub(super) fn handle_probe_input(&mut self, input: &[u8]) {
        let input = String::from_utf8_lossy(input);
        let mut chars = input.chars().peekable();
        while let Some(ch) = chars.next() {
            let code = match ch {
                '\u{8}' | '\u{7f}' => KeyCode::Backspace,
                '\r' | '\n' => KeyCode::Enter,
                '\t' => KeyCode::Tab,
                ch => KeyCode::Char(ch),
            };
            let modifiers = match code {
                KeyCode::Char(ch) if ch.is_uppercase() => KeyModifiers::SHIFT,
                _ => KeyModifiers::NONE,
            };
            let binding = KeyBinding::from_event(KeyEvent::new(code, modifiers));
            if let Some(index) = self.quarantined_actions.iter().position(|action| {
                startup_action_matches(action.binding, action.from_raw_probe, binding)
            }) {
                let action = self.quarantined_actions[index];
                if !action.quiet_elapsed || !action.preserve_after_quiet {
                    if action.quiet_elapsed {
                        self.quarantined_actions.remove(index);
                    }
                    if ch == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    continue;
                }
                self.quarantined_actions.remove(index);
            }
            match ch {
                '\u{8}' | '\u{7f}' => {
                    self.record_active_action(binding, /*from_raw_probe*/ true);
                    self.pop_char();
                }
                '\u{3}' => {
                    self.record_active_action(
                        crate::key_hint::ctrl(KeyCode::Char('c')),
                        /*from_raw_probe*/ true,
                    );
                    self.clear_pending_plain_whitespace();
                    self.interrupt_requested = true;
                }
                '\u{1a}' => {
                    self.record_active_action(
                        crate::key_hint::ctrl(KeyCode::Char('z')),
                        /*from_raw_probe*/ true,
                    );
                    self.clear_pending_plain_whitespace();
                    self.suspend_requested = true;
                }
                '\r' => {
                    self.record_active_action(
                        crate::key_hint::plain(KeyCode::Enter),
                        /*from_raw_probe*/ true,
                    );
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    self.push_pending_plain_whitespace('\n', binding, /*from_raw_probe*/ true);
                }
                '\n' => {
                    self.record_active_action(
                        crate::key_hint::plain(KeyCode::Enter),
                        /*from_raw_probe*/ true,
                    );
                    self.push_pending_plain_whitespace('\n', binding, /*from_raw_probe*/ true);
                }
                '\t' => {
                    self.record_active_action(
                        crate::key_hint::plain(KeyCode::Tab),
                        /*from_raw_probe*/ true,
                    );
                    self.push_pending_plain_whitespace('\t', binding, /*from_raw_probe*/ true);
                }
                ch if !ch.is_control() => {
                    self.trailing_printable_action = Some((binding, true));
                    self.push_plain_char(ch, binding, /*from_raw_probe*/ true);
                }
                ch => {
                    let code = if ch == '\u{1b}' {
                        KeyCode::Esc
                    } else {
                        KeyCode::Char(ch)
                    };
                    self.record_active_action(
                        KeyBinding::from_event(KeyEvent::new(code, KeyModifiers::NONE)),
                        /*from_raw_probe*/ true,
                    );
                    self.clear_pending_plain_whitespace();
                }
            }
        }
    }

    fn handle_unknown_probe_action(&mut self) {
        self.unknown_action_seen = true;
        self.clear_pending_plain_whitespace();
    }

    #[cfg(unix)]
    fn handle_startup_probe_input(&mut self, input: &[crate::terminal_probe::StartupInput]) {
        for input in input {
            match input {
                crate::terminal_probe::StartupInput::Plain(input) => {
                    self.handle_probe_input(input);
                }
                crate::terminal_probe::StartupInput::Paste(input) => {
                    self.push_text(&String::from_utf8_lossy(input));
                }
                crate::terminal_probe::StartupInput::Key(key_event) => {
                    self.handle_event(Event::Key(*key_event));
                }
                crate::terminal_probe::StartupInput::UnknownAction => {
                    self.handle_unknown_probe_action();
                }
            }
        }
    }

    pub(super) fn take_text_excluding_submission_bindings(
        &mut self,
        submission_bindings: &[KeyBinding],
    ) -> Option<String> {
        let pending = std::mem::take(&mut self.pending_plain_whitespace);
        let pending_actions = std::mem::take(&mut self.pending_plain_whitespace_actions);
        debug_assert_eq!(pending.chars().count(), pending_actions.len());
        for (ch, action) in pending.chars().zip(pending_actions) {
            if !action.release_observed
                && !self.repeat_actions.iter().any(|existing| {
                    existing.binding == action.binding
                        && existing.from_raw_probe == action.from_raw_probe
                })
            {
                self.repeat_actions.push(action);
            }
            if submission_bindings.iter().copied().any(|binding| {
                startup_action_matches(action.binding, action.from_raw_probe, binding)
            }) {
                if !action.release_observed {
                    self.remember_quarantined_action(action);
                }
            } else {
                let char_index = self.char_count;
                if self.push_char(ch) && !action.release_observed {
                    self.typed_text_actions.push((char_index, action));
                }
            }
        }
        self.char_count = 0;
        let typed_text_actions = std::mem::take(&mut self.typed_text_actions);
        for (_, action) in &typed_text_actions {
            if !action.release_observed
                && !self.repeat_actions.iter().any(|existing| {
                    existing.binding == action.binding
                        && existing.from_raw_probe == action.from_raw_probe
                })
            {
                self.repeat_actions.push(*action);
            }
        }
        let mut filtered_indexes = Vec::new();
        let mut filtered_actions = Vec::new();
        for (char_index, action) in typed_text_actions {
            if submission_bindings.iter().copied().any(|binding| {
                startup_action_matches(action.binding, action.from_raw_probe, binding)
                    && !matches!(action.binding.parts().0, KeyCode::Enter | KeyCode::Tab)
            }) {
                filtered_indexes.push(char_index);
                if !action.release_observed {
                    filtered_actions.push(action);
                }
            }
        }
        let mut filtered_indexes = filtered_indexes.into_iter().peekable();
        let text = std::mem::take(&mut self.text)
            .chars()
            .enumerate()
            .filter_map(|(char_index, ch)| {
                if filtered_indexes
                    .peek()
                    .is_some_and(|filtered_index| *filtered_index == char_index)
                {
                    filtered_indexes.next();
                    None
                } else {
                    Some(ch)
                }
            })
            .collect::<String>();
        for action in filtered_actions {
            self.remember_quarantined_action(action);
        }
        let text = (!text.is_empty()).then_some(text);
        self.restored_text |= text.is_some();
        text
    }

    pub(super) fn into_handoff(mut self) -> StartupInputHandoff {
        for (_, action) in std::mem::take(&mut self.typed_text_actions) {
            if !action.release_observed
                && !self.repeat_actions.iter().any(|existing| {
                    existing.binding == action.binding
                        && existing.from_raw_probe == action.from_raw_probe
                })
            {
                self.repeat_actions.push(action);
            }
        }
        for action in self.active_actions {
            if let Some(existing) = self
                .quarantined_actions
                .iter_mut()
                .find(|existing| existing.binding == action.binding)
            {
                existing.from_raw_probe |= action.from_raw_probe;
                existing.quiet_elapsed &= action.quiet_elapsed;
                existing.preserve_after_quiet |= action.preserve_after_quiet;
            } else {
                self.quarantined_actions.push(action);
            }
        }
        StartupInputHandoff {
            claimed: true,
            interrupt_requested: self.interrupt_requested,
            suspend_requested: self.suspend_requested,
            resume_draw_requested: false,
            pending_plain_whitespace: self.pending_plain_whitespace,
            pending_plain_whitespace_actions: self.pending_plain_whitespace_actions,
            trailing_printable_action: self.trailing_printable_action,
            quarantined_actions: self.quarantined_actions,
            repeat_actions: self.repeat_actions,
            unknown_action_seen: self.unknown_action_seen,
            restored_text: self.restored_text,
            submission_bindings: Vec::new(),
        }
    }
}

fn is_plain_printable(binding: KeyBinding) -> bool {
    let (code, modifiers) = binding.parts();
    matches!(code, KeyCode::Char(ch) if !ch.is_control())
        && !crate::key_hint::has_ctrl_or_alt(modifiers)
}

pub(super) fn startup_action_matches(
    expected: KeyBinding,
    from_raw_probe: bool,
    actual: KeyBinding,
) -> bool {
    if expected == actual {
        return true;
    }
    if !from_raw_probe {
        return false;
    }
    let (actual_code, _) = actual.parts();
    match expected.parts().0 {
        KeyCode::Enter => {
            actual_code == KeyCode::Enter
                || [
                    crate::key_hint::ctrl(KeyCode::Char('m')),
                    crate::key_hint::ctrl(KeyCode::Char('j')),
                ]
                .contains(&actual)
        }
        KeyCode::Tab => {
            actual_code == KeyCode::Tab || crate::key_hint::ctrl(KeyCode::Char('i')) == actual
        }
        KeyCode::Backspace => {
            actual_code == KeyCode::Backspace || crate::key_hint::ctrl(KeyCode::Char('h')) == actual
        }
        _ => false,
    }
}

/// Flush the underlying stdin buffer to clear any input buffered at the terminal level.
#[cfg(unix)]
pub(super) fn flush_terminal_input_buffer() {
    // Safety: flushing the stdin queue is safe and does not move ownership.
    let result = unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!("failed to tcflush stdin: {err}");
    }
}

/// Flush the underlying stdin buffer to clear any input buffered at the terminal level.
#[cfg(windows)]
pub(super) fn flush_terminal_input_buffer() {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::FlushConsoleInputBuffer;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;

    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle == INVALID_HANDLE_VALUE || handle == 0 {
        let err = unsafe { GetLastError() };
        tracing::warn!("failed to get stdin handle for flush: error {err}");
        return;
    }

    let result = unsafe { FlushConsoleInputBuffer(handle) };
    if result == 0 {
        let err = unsafe { GetLastError() };
        tracing::warn!("failed to flush stdin buffer: error {err}");
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn flush_terminal_input_buffer() {}

pub(crate) fn abandon_prepared_terminal() {
    let _ = super::restore_after_exit_best_effort();
}

fn pause_and_capture_startup_input_locked(input: &mut StartupInputBuffer) -> Result<()> {
    #[cfg(unix)]
    {
        let (captured_bytes, captured_bytes_truncated) = take_captured_startup_bytes_locked();
        let captured = crate::terminal_probe::pending_startup_input_with_prefix(
            crate::terminal_probe::DEFAULT_TIMEOUT,
            captured_bytes,
            captured_bytes_truncated,
        )?;
        input.handle_startup_probe_input(&captured);
    }
    #[cfg(windows)]
    {
        let captured = take_captured_startup_events_locked();
        for event in coalesce_windows_startup_pastes(captured.events) {
            input.handle_event(event);
        }
        if captured.truncated {
            input.handle_unknown_probe_action();
        }
        while crossterm::event::poll(Duration::ZERO)? {
            input.handle_event(crossterm::event::read()?);
        }
    }
    #[cfg(not(any(unix, windows)))]
    while crossterm::event::poll(Duration::ZERO)? {
        input.handle_event(crossterm::event::read()?);
    }
    Ok(())
}

/// Transfer bounded startup input, then enter full TUI modes without exposing an ownership gap.
pub(super) fn capture_startup_input_for_full_modes(input: &mut StartupInputBuffer) -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    pause_and_capture_startup_input_locked(input)?;
    super::set_modes_unlocked()
}

fn resume_startup_input_capture_locked() -> Result<()> {
    #[cfg(any(unix, windows))]
    {
        let mut state = STARTUP_CAPTURE_MODE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(mode) = state.as_mut() {
            resume_startup_input_reader(mode)?;
        }
    }
    Ok(())
}

pub(super) fn capture_startup_input(input: &mut StartupInputBuffer) -> Result<()> {
    let _lifecycle = super::terminal_lifecycle_guard();
    let capture_result = pause_and_capture_startup_input_locked(input);
    let resume_result = resume_startup_input_capture_locked();
    capture_result.and(resume_result)
}

impl PreparedTerminal {
    /// Claim queued terminal input before slower startup work begins.
    pub(crate) fn prepare() -> Result<Self> {
        if !stdin().is_terminal() {
            return Err(std::io::Error::other("stdin is not a terminal"));
        }
        if !stdout().is_terminal() {
            return Err(std::io::Error::other("stdout is not a terminal"));
        }
        enable_startup_capture_mode()?;
        let prepared = Self {
            active: true,
            startup_input: StartupInputBuffer::default(),
            startup_action_latch: StartupActionLatch::default(),
        };
        let _lifecycle = super::terminal_lifecycle_guard();
        if let Err(err) = super::ensure_virtual_terminal_processing() {
            let _ = super::restore_after_exit_unlocked();
            return Err(err);
        }
        if let Err(err) = execute!(stdout(), EnableBracketedPaste) {
            let _ = super::restore_after_exit_unlocked();
            return Err(err);
        }
        Ok(prepared)
    }

    pub(crate) fn quarantine_action_repeats(&mut self, key_event: KeyEvent) {
        self.startup_action_latch.record(key_event);
    }

    #[cfg(any(unix, windows))]
    pub(crate) fn pause_input_capture(&mut self) -> Result<()> {
        let _lifecycle = super::terminal_lifecycle_guard();
        {
            let mut state = STARTUP_CAPTURE_MODE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(mode) = state.as_mut() {
                mode.input_reader_enabled = false;
            }
        }
        pause_and_capture_startup_input_locked(&mut self.startup_input)
    }

    #[cfg(any(unix, windows))]
    pub(crate) fn resume_input_capture(&mut self) -> Result<()> {
        let _lifecycle = super::terminal_lifecycle_guard();
        let mut state = STARTUP_CAPTURE_MODE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(mode) = state.as_mut() {
            mode.input_reader_enabled = true;
            resume_startup_input_reader(mode)?;
        }
        Ok(())
    }

    /// Initialize the TUI and move queued printable input into application-owned memory.
    pub(crate) fn activate(mut self) -> Result<InitializedTerminal> {
        // Startup may intentionally catch panics, and the error-reporting setup installs its own
        // hook before activation. Wrap that final hook only once the terminal is about to become
        // fully raw so caught startup panics do not relinquish capture ownership.
        super::set_panic_hook();
        let mut startup_input = std::mem::take(&mut self.startup_input);
        self.startup_action_latch.drain_into(&mut startup_input);
        #[cfg(any(unix, windows))]
        let activation_lifecycle = super::terminal_lifecycle_guard();
        #[cfg(unix)]
        let (captured_startup_input, captured_startup_input_truncated) =
            take_captured_startup_bytes_locked();
        #[cfg(windows)]
        {
            let captured = take_captured_startup_events_locked();
            for event in coalesce_windows_startup_pastes(captured.events) {
                startup_input.handle_event(event);
            }
            if captured.truncated {
                startup_input.handle_unknown_probe_action();
            }
        }
        #[cfg(not(any(unix, windows)))]
        capture_startup_input(&mut startup_input)?;

        #[cfg(unix)]
        let backend = CrosstermBackend::new(stdout());

        #[cfg(unix)]
        let startup_probe = {
            use crate::terminal_probe::StartupKeyboardEnhancementProbe;

            let started_at = std::time::Instant::now();
            let keyboard_probe = if keyboard_modes::keyboard_enhancement_disabled() {
                StartupKeyboardEnhancementProbe::Skip
            } else {
                StartupKeyboardEnhancementProbe::Query
            };
            match crate::terminal_probe::startup(
                crate::terminal_probe::DEFAULT_TIMEOUT,
                keyboard_probe,
                captured_startup_input,
                captured_startup_input_truncated,
            ) {
                Ok(probe) => {
                    tracing::info!(
                        duration_ms = %started_at.elapsed().as_millis(),
                        cursor_position = probe.cursor_position.is_some(),
                        default_colors = probe.default_colors.is_some(),
                        keyboard_enhancement_supported = ?probe.keyboard_enhancement_supported,
                        "terminal startup probes completed"
                    );
                    probe
                }
                Err(failure)
                    if matches!(
                        failure.error.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(failure.error);
                }
                Err(failure) => {
                    tracing::warn!(
                        duration_ms = %started_at.elapsed().as_millis(),
                        "terminal startup probes failed: {}", failure.error
                    );
                    crate::terminal_probe::StartupProbe {
                        cursor_position: None,
                        default_colors: None,
                        keyboard_enhancement_supported: None,
                        input: failure.input,
                    }
                }
            }
        };

        #[cfg(unix)]
        startup_input.handle_startup_probe_input(&startup_probe.input);

        #[cfg(unix)]
        crate::terminal_palette::set_default_colors_from_startup_probe(
            startup_probe.default_colors,
        );

        #[cfg(unix)]
        let cursor_pos = match startup_probe.cursor_position {
            Some(pos) => pos,
            None => {
                tracing::warn!("initial cursor position probe timed out; defaulting to origin");
                Position { x: 0, y: 0 }
            }
        };

        #[cfg(unix)]
        let enhanced_keys_supported = startup_probe
            .keyboard_enhancement_supported
            .unwrap_or(/*default*/ false);

        #[cfg(not(unix))]
        let mut backend = CrosstermBackend::new(stdout());

        #[cfg(not(unix))]
        let cursor_pos = super::cursor_position_with_crossterm(&mut backend);

        #[cfg(not(unix))]
        let enhanced_keys_supported = !keyboard_modes::keyboard_enhancement_disabled()
            && super::detect_keyboard_enhancement_supported();

        #[cfg(windows)]
        super::probe_windows_default_colors();

        #[cfg(any(unix, windows))]
        {
            pause_and_capture_startup_input_locked(&mut startup_input)?;
            resume_startup_input_capture_locked()?;
            drop(activation_lifecycle);
        }
        #[cfg(not(any(unix, windows)))]
        capture_startup_input(&mut startup_input)?;

        let terminal = CustomTerminal::with_options_and_cursor_position(backend, cursor_pos)?;
        let stderr_guard = super::terminal_stderr::TerminalStderrGuard::install()?;
        let initialized = InitializedTerminal {
            terminal,
            enhanced_keys_supported,
            stderr_guard,
            startup_input,
            startup_capture_active: true,
        };
        self.active = false;
        Ok(initialized)
    }
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
