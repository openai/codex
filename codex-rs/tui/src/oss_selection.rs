use std::io;
use std::sync::LazyLock;

use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use codex_model_provider_info::DEFAULT_LMSTUDIO_PORT;
use codex_model_provider_info::DEFAULT_OLLAMA_PORT;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
#[cfg(not(unix))]
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone)]
struct ProviderOption {
    name: String,
    status: ProviderStatus,
}

#[derive(Clone)]
enum ProviderStatus {
    Running,
    NotRunning,
    Unknown,
}

/// Options displayed in the *select* mode.
///
/// The `key` is matched case-insensitively.
struct SelectOption {
    label: Line<'static>,
    description: &'static str,
    key: KeyCode,
    provider_id: &'static str,
}

static OSS_SELECT_OPTIONS: LazyLock<Vec<SelectOption>> = LazyLock::new(|| {
    vec![
        SelectOption {
            label: Line::from(vec!["L".underlined(), "M Studio".into()]),
            description: "Local LM Studio server (default port 1234)",
            key: KeyCode::Char('l'),
            provider_id: LMSTUDIO_OSS_PROVIDER_ID,
        },
        SelectOption {
            label: Line::from(vec!["O".underlined(), "llama".into()]),
            description: "Local Ollama server (Responses API, default port 11434)",
            key: KeyCode::Char('o'),
            provider_id: OLLAMA_OSS_PROVIDER_ID,
        },
    ]
});

// This startup wizard runs before the main TUI runtime keymap is available, so
// it mirrors the built-in horizontal list defaults instead of reading config.
// The shared matcher still covers raw C0 Ctrl-H/Ctrl-L terminal reports.
const MOVE_LEFT_KEYS: [KeyBinding; 2] = [
    key_hint::plain(KeyCode::Left),
    key_hint::ctrl(KeyCode::Char('h')),
];
const MOVE_RIGHT_KEYS: [KeyBinding; 2] = [
    key_hint::plain(KeyCode::Right),
    key_hint::ctrl(KeyCode::Char('l')),
];

pub struct OssSelectionWidget<'a> {
    select_options: &'a Vec<SelectOption>,
    confirmation_prompt: Paragraph<'a>,

    /// Currently selected index in *select* mode.
    selected_option: usize,

    /// Set to `true` once a decision has been sent – the parent view can then
    /// remove this widget from its queue.
    done: bool,

    selection: Option<String>,
}

impl OssSelectionWidget<'_> {
    fn new(lmstudio_status: ProviderStatus, ollama_status: ProviderStatus) -> io::Result<Self> {
        let providers = vec![
            ProviderOption {
                name: "LM Studio".to_string(),
                status: lmstudio_status,
            },
            ProviderOption {
                name: "Ollama (Responses)".to_string(),
                status: ollama_status.clone(),
            },
            ProviderOption {
                name: "Ollama (Chat)".to_string(),
                status: ollama_status,
            },
        ];

        let mut contents: Vec<Line> = vec![
            Line::from(vec![
                "? ".fg(Color::Blue),
                "Select an open-source provider".bold(),
            ]),
            Line::from(""),
            Line::from("  Choose which local AI server to use for your session."),
            Line::from(""),
        ];

        // Add status indicators for each provider
        for provider in &providers {
            let (status_symbol, status_color) = get_status_symbol_and_color(&provider.status);
            contents.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(status_symbol, Style::default().fg(status_color)),
                Span::raw(format!(" {} ", provider.name)),
            ]));
        }
        contents.push(Line::from(""));
        contents.push(Line::from("  ● Running  ○ Not Running").add_modifier(Modifier::DIM));

        contents.push(Line::from(""));
        contents.push(
            Line::from("  Press Enter to select • Ctrl+C to exit").add_modifier(Modifier::DIM),
        );

        let confirmation_prompt = Paragraph::new(contents).wrap(Wrap { trim: false });

        Ok(Self {
            select_options: &OSS_SELECT_OPTIONS,
            confirmation_prompt,
            selected_option: 0,
            done: false,
            selection: None,
        })
    }

    fn get_confirmation_prompt_height(&self, width: u16) -> u16 {
        // Should cache this for last value of width.
        self.confirmation_prompt.line_count(width) as u16
    }

    /// Process a `KeyEvent` coming from crossterm. Always consumes the event
    /// while the modal is visible.
    /// Process a key event originating from crossterm. As the modal fully
    /// captures input while visible, we don't need to report whether the event
    /// was consumed—callers can assume it always is.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<String> {
        if key.kind == KeyEventKind::Press {
            self.handle_select_key(key);
        }
        if self.done {
            self.selection.clone()
        } else {
            None
        }
    }

    /// Normalize a key for comparison.
    /// - For `KeyCode::Char`, converts to lowercase for case-insensitive matching.
    /// - Other key codes are returned unchanged.
    fn normalize_keycode(code: KeyCode) -> KeyCode {
        match code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        }
    }

    fn handle_select_key(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_decision("__CANCELLED__".to_string());
            }
            _ if MOVE_LEFT_KEYS.is_pressed(key_event) => {
                self.selected_option = (self.selected_option + self.select_options.len() - 1)
                    % self.select_options.len();
            }
            _ if MOVE_RIGHT_KEYS.is_pressed(key_event) => {
                self.selected_option = (self.selected_option + 1) % self.select_options.len();
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                let opt = &self.select_options[self.selected_option];
                self.send_decision(opt.provider_id.to_string());
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.send_decision(LMSTUDIO_OSS_PROVIDER_ID.to_string());
            }
            KeyEvent { code, .. } => {
                let other = code;
                let normalized = Self::normalize_keycode(other);
                if let Some(opt) = self
                    .select_options
                    .iter()
                    .find(|opt| Self::normalize_keycode(opt.key) == normalized)
                {
                    self.send_decision(opt.provider_id.to_string());
                }
            }
        }
    }

    fn send_decision(&mut self, selection: String) {
        self.selection = Some(selection);
        self.done = true;
    }

    /// Returns `true` once the user has made a decision and the widget no
    /// longer needs to be displayed.
    pub fn is_complete(&self) -> bool {
        self.done
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.get_confirmation_prompt_height(width) + self.select_options.len() as u16
    }
}

impl WidgetRef for &OssSelectionWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let prompt_height = self.get_confirmation_prompt_height(area.width);
        let [prompt_chunk, response_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(prompt_height), Constraint::Min(0)])
            .areas(area);

        let lines: Vec<Line> = self
            .select_options
            .iter()
            .enumerate()
            .map(|(idx, opt)| {
                let style = if idx == self.selected_option {
                    Style::new().bg(Color::Cyan).fg(Color::Black)
                } else {
                    Style::new().bg(Color::DarkGray)
                };
                opt.label.clone().alignment(Alignment::Center).style(style)
            })
            .collect();

        let [title_area, button_area, description_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(response_chunk.inner(Margin::new(1, 0)));

        Line::from("Select provider?").render(title_area, buf);

        self.confirmation_prompt.clone().render(prompt_chunk, buf);
        let areas = Layout::horizontal(
            lines
                .iter()
                .map(|l| Constraint::Length(l.width() as u16 + 2)),
        )
        .spacing(1)
        .split(button_area);
        for (idx, area) in areas.iter().enumerate() {
            let line = &lines[idx];
            line.render(*area, buf);
        }

        Line::from(self.select_options[self.selected_option].description)
            .style(Style::new().italic().fg(Color::DarkGray))
            .render(description_area.inner(Margin::new(1, 0)), buf);
    }
}

fn get_status_symbol_and_color(status: &ProviderStatus) -> (&'static str, Color) {
    match status {
        ProviderStatus::Running => ("●", Color::Green),
        ProviderStatus::NotRunning => ("○", Color::Red),
        ProviderStatus::Unknown => ("?", Color::Yellow),
    }
}

pub(crate) struct OssProviderSelection {
    pub(crate) provider: String,
    pub(crate) manually_selected: bool,
    pub(crate) active_keys: Vec<KeyEvent>,
}

pub async fn select_oss_provider(
    prepared_terminal: &mut crate::tui::PreparedTerminal,
) -> io::Result<OssProviderSelection> {
    // Check provider statuses first
    let lmstudio_status = check_lmstudio_status().await;
    let ollama_status = check_ollama_status().await;

    // Autoselect if only one is running
    match (&lmstudio_status, &ollama_status) {
        (ProviderStatus::Running, ProviderStatus::NotRunning) => {
            let provider = LMSTUDIO_OSS_PROVIDER_ID.to_string();
            return Ok(OssProviderSelection {
                provider,
                manually_selected: false,
                active_keys: Vec::new(),
            });
        }
        (ProviderStatus::NotRunning, ProviderStatus::Running) => {
            let provider = OLLAMA_OSS_PROVIDER_ID.to_string();
            return Ok(OssProviderSelection {
                provider,
                manually_selected: false,
                active_keys: Vec::new(),
            });
        }
        _ => {
            // Both running or both not running - show UI
        }
    }

    let mut widget = OssSelectionWidget::new(lmstudio_status, ollama_status)?;

    #[cfg(any(unix, windows))]
    prepared_terminal.pause_input_capture()?;
    crate::tui::set_modes()?;
    let mut stdout = io::stdout();
    let mut alternate_screen_active = false;
    let result = (|| -> io::Result<OssProviderSelection> {
        {
            let _lifecycle = crate::tui::terminal_lifecycle_guard();
            execute!(stdout, EnterAlternateScreen)?;
            crate::tui::note_alt_screen_entered();
        }
        alternate_screen_active = true;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|f| {
            (&widget).render_ref(f.area(), f.buffer_mut());
        })?;
        let mut event_reader = StartupEventReader::default();
        let mut startup_keys = match discard_startup_events_until_quiet(&mut event_reader)? {
            StartupInputDrain::Cancelled => {
                return Ok(OssProviderSelection {
                    provider: "__CANCELLED__".to_string(),
                    manually_selected: true,
                    active_keys: Vec::new(),
                });
            }
            StartupInputDrain::Settled(startup_keys) => startup_keys,
        };

        loop {
            let event = if let Some(timeout) = startup_keys.poll_timeout(Instant::now()) {
                if !event_reader.poll(timeout)? {
                    startup_keys.expire_after_quiet();
                    continue;
                }
                event_reader.read()?
            } else {
                event_reader.read()?
            };
            if let Event::Key(key_event) = event
                && !startup_keys.should_discard(key_event, Instant::now())
                && let Some(selection) = widget.handle_key_event(key_event)
            {
                break Ok(OssProviderSelection {
                    provider: selection,
                    manually_selected: true,
                    active_keys: startup_keys.active_keys(),
                });
            }
            terminal.draw(|f| {
                (&widget).render_ref(f.area(), f.buffer_mut());
            })?;
        }
    })();

    let mut cleanup_error = None;
    if alternate_screen_active {
        let _lifecycle = crate::tui::terminal_lifecycle_guard();
        let alternate_scroll_disabled =
            match execute!(io::stdout(), crate::tui::DisableAlternateScroll) {
                Ok(()) => true,
                Err(err) => {
                    cleanup_error = Some(err);
                    false
                }
            };
        match execute!(io::stdout(), LeaveAlternateScreen) {
            Ok(()) if alternate_scroll_disabled => crate::tui::note_alt_screen_left(),
            Ok(()) => {}
            Err(err) => {
                cleanup_error.get_or_insert(err);
            }
        }
    }
    if let Err(err) = crate::tui::restore_startup_screen() {
        cleanup_error.get_or_insert(err);
    }
    #[cfg(any(unix, windows))]
    if let Err(err) = prepared_terminal.resume_input_capture() {
        cleanup_error.get_or_insert(err);
    }
    match (result, cleanup_error) {
        (Err(err), _) | (Ok(_), Some(err)) => Err(err),
        (Ok(selection), None) => Ok(selection),
    }
}

fn discard_startup_events_until_quiet(
    event_reader: &mut StartupEventReader,
) -> io::Result<StartupInputDrain> {
    let event_reader = RefCell::new(event_reader);
    discard_startup_events_until_quiet_with(
        |timeout| event_reader.borrow().poll(timeout),
        || event_reader.borrow_mut().read(),
        Instant::now,
    )
}

#[derive(Default)]
struct StartupEventReader {
    #[cfg(unix)]
    pending: VecDeque<Event>,
}

impl StartupEventReader {
    fn poll(&self, timeout: Duration) -> io::Result<bool> {
        #[cfg(unix)]
        {
            if !self.pending.is_empty() {
                return Ok(true);
            }
            crate::terminal_probe::startup_event_available(timeout)
        }
        #[cfg(not(unix))]
        {
            event::poll(timeout)
        }
    }

    fn read(&mut self) -> io::Result<Event> {
        #[cfg(unix)]
        {
            if let Some(event) = self.pending.pop_front() {
                return Ok(event);
            }
            self.pending
                .extend(crate::terminal_probe::read_startup_events()?);
            self.pending.pop_front().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "startup input did not decode to an event",
                )
            })
        }
        #[cfg(not(unix))]
        {
            event::read()
        }
    }
}

enum StartupInputDrain {
    Cancelled,
    Settled(StartupKeyLatch),
}

#[derive(Default)]
struct StartupKeyLatch {
    blocked: Vec<KeyEvent>,
    quiet_deadline: Option<Instant>,
}

impl StartupKeyLatch {
    fn record(&mut self, key_event: KeyEvent, now: Instant) {
        let binding = KeyBinding::from_event(key_event);
        match key_event.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => {
                if let Some(existing) = self
                    .blocked
                    .iter_mut()
                    .find(|existing| KeyBinding::from_event(**existing) == binding)
                {
                    *existing = key_event;
                } else {
                    self.blocked.push(key_event);
                }
                self.quiet_deadline = Some(now + crate::tui::STARTUP_INPUT_QUIET_PERIOD);
            }
            KeyEventKind::Release => {
                self.blocked
                    .retain(|blocked| KeyBinding::from_event(*blocked) != binding);
                if self.blocked.is_empty() {
                    self.quiet_deadline = None;
                }
            }
        }
    }

    fn should_discard(&mut self, key_event: KeyEvent, now: Instant) -> bool {
        let binding = KeyBinding::from_event(key_event);
        if key_event.kind == KeyEventKind::Release {
            self.record(key_event, now);
            return true;
        }
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return true;
        }
        if self
            .blocked
            .iter()
            .any(|blocked| KeyBinding::from_event(*blocked) == binding)
        {
            self.quiet_deadline = Some(now + crate::tui::STARTUP_INPUT_QUIET_PERIOD);
            return true;
        }
        self.record(key_event, now);
        false
    }

    fn finish_initial_drain(&mut self) {
        self.quiet_deadline = None;
    }

    fn poll_timeout(&self, now: Instant) -> Option<Duration> {
        self.quiet_deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    fn expire_after_quiet(&mut self) {
        self.blocked.clear();
        self.quiet_deadline = None;
    }

    fn active_keys(&self) -> Vec<KeyEvent> {
        self.blocked.clone()
    }
}

fn discard_startup_events_until_quiet_with(
    mut poll: impl FnMut(Duration) -> io::Result<bool>,
    mut read: impl FnMut() -> io::Result<Event>,
    mut now: impl FnMut() -> Instant,
) -> io::Result<StartupInputDrain> {
    let mut quiet_deadline = None;
    let mut startup_keys = StartupKeyLatch::default();
    loop {
        let timeout = quiet_deadline
            .map(|deadline: Instant| deadline.saturating_duration_since(now()))
            .unwrap_or(Duration::ZERO);
        if !poll(timeout)? {
            startup_keys.finish_initial_drain();
            return Ok(StartupInputDrain::Settled(startup_keys));
        }
        let event = read()?;
        if matches!(
            &event,
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) if modifiers.contains(KeyModifiers::CONTROL) && !key_hint::is_altgr(*modifiers)
        ) {
            return Ok(StartupInputDrain::Cancelled);
        }
        if let Event::Key(key_event) = event {
            startup_keys.record(key_event, now());
        }
        quiet_deadline = (!startup_keys.blocked.is_empty())
            .then(|| now() + crate::tui::STARTUP_INPUT_QUIET_PERIOD);
    }
}

async fn check_lmstudio_status() -> ProviderStatus {
    match check_port_status(DEFAULT_LMSTUDIO_PORT).await {
        Ok(true) => ProviderStatus::Running,
        Ok(false) => ProviderStatus::NotRunning,
        Err(_) => ProviderStatus::Unknown,
    }
}

async fn check_ollama_status() -> ProviderStatus {
    match check_port_status(DEFAULT_OLLAMA_PORT).await {
        Ok(true) => ProviderStatus::Running,
        Ok(false) => ProviderStatus::NotRunning,
        Err(_) => ProviderStatus::Unknown,
    }
}

async fn check_port_status(port: u16) -> io::Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(io::Error::other)?;

    let url = format!("http://localhost:{port}");

    match client.get(&url).send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false), // Connection failed = not running
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn ctrl_h_l_move_provider_selection() {
        let mut widget = OssSelectionWidget::new(ProviderStatus::Unknown, ProviderStatus::Unknown)
            .expect("widget should initialize");

        assert_eq!(widget.selected_option, 0);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(widget.selected_option, 1);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(widget.selected_option, 0);
    }

    #[cfg(unix)]
    #[test]
    fn startup_reader_preserves_all_events_from_one_decoded_unit() {
        let first = Event::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT));
        let second = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let mut reader = StartupEventReader {
            pending: VecDeque::from([first.clone(), second.clone()]),
        };

        assert!(reader.poll(Duration::ZERO).expect("queued event"));
        assert_eq!(reader.read().expect("first event"), first);
        assert_eq!(reader.read().expect("second event"), second);
    }

    #[test]
    fn startup_drain_waits_for_repeated_enter_to_settle() {
        let now = Cell::new(Instant::now());
        let poll_count = Cell::new(0);
        let poll_timeouts = RefCell::new(Vec::new());
        let events = RefCell::new(VecDeque::from([
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        ]));

        let result = discard_startup_events_until_quiet_with(
            |timeout| {
                poll_timeouts.borrow_mut().push(timeout);
                let count = poll_count.get();
                poll_count.set(count + 1);
                if count == 1 {
                    now.set(now.get() + crate::tui::STARTUP_INPUT_QUIET_PERIOD / 2);
                } else if count == 2 {
                    now.set(now.get() + timeout);
                }
                Ok(count < 2)
            },
            || Ok(events.borrow_mut().pop_front().expect("queued event")),
            || now.get(),
        )
        .expect("startup drain");

        let StartupInputDrain::Settled(mut startup_keys) = result else {
            panic!("expected settled startup input");
        };
        assert!(
            startup_keys
                .should_discard(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now.get())
        );
        assert!(
            !startup_keys
                .should_discard(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), now.get())
        );
        assert_eq!(
            poll_timeouts.into_inner(),
            vec![
                Duration::ZERO,
                crate::tui::STARTUP_INPUT_QUIET_PERIOD,
                crate::tui::STARTUP_INPUT_QUIET_PERIOD,
            ]
        );
    }

    #[test]
    fn startup_drain_preserves_ctrl_c_repeat_as_cancellation() {
        let event = RefCell::new(Some(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Repeat,
        ))));

        let result = discard_startup_events_until_quiet_with(
            |_| Ok(true),
            || Ok(event.borrow_mut().take().expect("queued event")),
            Instant::now,
        )
        .expect("startup drain");

        assert!(matches!(result, StartupInputDrain::Cancelled));
    }

    #[test]
    fn startup_key_latch_blocks_delayed_repeat_until_release() {
        let now = Instant::now();
        let mut startup_keys = StartupKeyLatch::default();
        startup_keys.record(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        startup_keys.finish_initial_drain();

        assert!(startup_keys.should_discard(
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat,),
            now
        ));
        assert!(startup_keys.should_discard(
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release,),
            now
        ));
        assert!(
            !startup_keys.should_discard(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now)
        );
    }

    #[test]
    fn startup_key_latch_rearms_until_a_full_quiet_interval() {
        let now = Instant::now();
        let mut startup_keys = StartupKeyLatch::default();
        startup_keys.record(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        startup_keys.finish_initial_drain();

        assert!(startup_keys.should_discard(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            now + crate::tui::STARTUP_INPUT_QUIET_PERIOD / 2,
        ));
        assert!(startup_keys.should_discard(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            now + crate::tui::STARTUP_INPUT_QUIET_PERIOD,
        ));
        assert_eq!(
            startup_keys.poll_timeout(now + crate::tui::STARTUP_INPUT_QUIET_PERIOD),
            Some(crate::tui::STARTUP_INPUT_QUIET_PERIOD)
        );
        assert_eq!(
            startup_keys.poll_timeout(now + crate::tui::STARTUP_INPUT_QUIET_PERIOD * 2),
            Some(Duration::ZERO)
        );
        startup_keys.expire_after_quiet();
        assert!(!startup_keys.should_discard(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            now + crate::tui::STARTUP_INPUT_QUIET_PERIOD * 2,
        ));
    }

    #[test]
    fn startup_key_latch_keeps_all_unreleased_drained_keys() {
        let mut startup_keys = StartupKeyLatch::default();
        let now = Instant::now();
        startup_keys.record(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        startup_keys.record(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), now);

        assert!(startup_keys.should_discard(
            KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Repeat,),
            now
        ));
        assert!(startup_keys.should_discard(
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat,),
            now
        ));
        assert_eq!(startup_keys.active_keys().len(), 2);
    }

    #[test]
    fn startup_key_latch_records_release_during_drain() {
        let mut startup_keys = StartupKeyLatch::default();
        let now = Instant::now();
        startup_keys.record(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        startup_keys.record(
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release),
            now,
        );

        assert!(
            !startup_keys.should_discard(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now)
        );
    }

    #[cfg(windows)]
    #[test]
    fn startup_drain_does_not_treat_altgr_c_as_cancellation() {
        let poll_count = Cell::new(0);
        let event = RefCell::new(Some(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ))));

        let result = discard_startup_events_until_quiet_with(
            |_| {
                let count = poll_count.get();
                poll_count.set(count + 1);
                Ok(count == 0)
            },
            || Ok(event.borrow_mut().take().expect("queued event")),
            Instant::now,
        )
        .expect("startup drain");

        assert!(matches!(result, StartupInputDrain::Settled(_)));
    }
}
