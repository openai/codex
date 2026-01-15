use std::io;
use std::sync::LazyLock;

use codex_core::DEFAULT_LMSTUDIO_PORT;
use codex_core::DEFAULT_OLLAMA_PORT;
use codex_core::LMSTUDIO_OSS_PROVIDER_ID;
use codex_core::OLLAMA_CHAT_PROVIDER_ID;
use codex_core::OLLAMA_OSS_PROVIDER_ID;
use codex_core::config::set_default_oss_provider;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
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
use std::time::Duration;

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
        SelectOption {
            label: Line::from(vec!["Ollama (".into(), "c".underlined(), "hat)".into()]),
            description: "Local Ollama server (chat wire API, default port 11434)",
            key: KeyCode::Char('c'),
            provider_id: OLLAMA_CHAT_PROVIDER_ID,
        },
    ]
});

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
        match key_event.code {
            KeyCode::Char('c')
                if key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.send_decision("__CANCELLED__".to_string());
            }
            KeyCode::Left => {
                self.selected_option = (self.selected_option + self.select_options.len() - 1)
                    % self.select_options.len();
            }
            KeyCode::Right => {
                self.selected_option = (self.selected_option + 1) % self.select_options.len();
            }
            KeyCode::Enter => {
                let opt = &self.select_options[self.selected_option];
                self.send_decision(opt.provider_id.to_string());
            }
            KeyCode::Esc => {
                self.send_decision(LMSTUDIO_OSS_PROVIDER_ID.to_string());
            }
            other => {
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

pub async fn select_oss_provider(codex_home: &std::path::Path) -> io::Result<String> {
    // Check provider statuses first
    let lmstudio_status = check_lmstudio_status().await;
    let ollama_status = check_ollama_status().await;

    // Autoselect if only one is running
    match (&lmstudio_status, &ollama_status) {
        (ProviderStatus::Running, ProviderStatus::NotRunning) => {
            let provider = LMSTUDIO_OSS_PROVIDER_ID.to_string();
            return Ok(provider);
        }
        (ProviderStatus::NotRunning, ProviderStatus::Running) => {
            let provider = OLLAMA_OSS_PROVIDER_ID.to_string();
            return Ok(provider);
        }
        _ => {
            // Both running or both not running - show UI
        }
    }

    let mut widget = OssSelectionWidget::new(lmstudio_status, ollama_status)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = loop {
        terminal.draw(|f| {
            (&widget).render_ref(f.area(), f.buffer_mut());
        })?;

        if let Event::Key(key_event) = event::read()?
            && let Some(selection) = widget.handle_key_event(key_event)
        {
            break Ok(selection);
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    // If the user manually selected an OSS provider, we save it as the
    // default one to use later.
    if let Ok(ref provider) = result
        && let Err(e) = set_default_oss_provider(codex_home, provider)
    {
        tracing::warn!("Failed to save OSS provider preference: {e}");
    }

    result
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
