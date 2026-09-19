//! Local provider discovery and shared startup picker presentation.
//! Discovery and selection outcomes stay independent of the picker layout.

use std::io;
use std::sync::LazyLock;

use crate::bottom_pane::picker_option_list;
use crate::bottom_pane::render_menu_surface;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::render::Insets;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::render::renderable::RenderableItem;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_model_provider_info::DEFAULT_LMSTUDIO_PORT;
use codex_model_provider_info::DEFAULT_OLLAMA_PORT;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::time::Duration;

#[derive(Clone)]
pub(crate) enum ProviderStatus {
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
// it retains the built-in horizontal shortcuts alongside vertical navigation.
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
    provider_statuses: [ProviderStatus; 2],

    /// Currently selected index in *select* mode.
    selected_option: usize,

    /// Set to `true` once a decision has been sent – the parent view can then
    /// remove this widget from its queue.
    done: bool,

    selection: Option<String>,
}

impl OssSelectionWidget<'_> {
    fn new(lmstudio_status: ProviderStatus, ollama_status: ProviderStatus) -> Self {
        Self {
            select_options: &OSS_SELECT_OPTIONS,
            provider_statuses: [lmstudio_status, ollama_status],
            selected_option: 0,
            done: false,
            selection: None,
        }
    }

    /// Consume a press while the picker is visible, returning a completed decision.
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
            _ if MOVE_LEFT_KEYS.is_pressed(key_event)
                || matches!(key_event.code, KeyCode::Up | KeyCode::Char('k')) =>
            {
                self.selected_option = (self.selected_option + self.select_options.len() - 1)
                    % self.select_options.len();
            }
            _ if MOVE_RIGHT_KEYS.is_pressed(key_event)
                || matches!(key_event.code, KeyCode::Down | KeyCode::Char('j')) =>
            {
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
            KeyEvent {
                code: KeyCode::Char('1' | '2'),
                ..
            } => {
                let index = usize::from(key_event.code == KeyCode::Char('2'));
                self.send_decision(self.select_options[index].provider_id.to_string());
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
}

impl WidgetRef for &OssSelectionWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = FlexRenderable::new();
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        column.push(
            /*flex*/ 0,
            Paragraph::new("Select an open-source provider".bold())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(
            /*flex*/ 1,
            Paragraph::new("Choose a local AI server for this session.".dim())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        let labels: Vec<_> = self
            .select_options
            .iter()
            .zip(&self.provider_statuses)
            .map(|(option, status)| {
                let (label, color) = match status {
                    ProviderStatus::Running => ("● Running", Color::Green),
                    ProviderStatus::NotRunning => ("○ Not running", Color::Red),
                    ProviderStatus::Unknown => ("? Unknown", Color::Yellow),
                };
                let mut line = option.label.clone();
                line.spans.extend([" · ".dim(), label.fg(color)]);
                line
            })
            .collect();
        column.push(
            /*flex*/ 0,
            picker_option_list(labels, self.selected_option),
        );
        column.push(
            /*flex*/ 1,
            Paragraph::new(self.select_options[self.selected_option].description.dim())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        column.push(
            /*flex*/ 0,
            Paragraph::new(Line::from(vec![
                "↑/↓".bold(),
                " choose · ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " select · ".dim(),
                key_hint::plain(KeyCode::Esc).into(),
                " LM Studio · ".dim(),
                key_hint::ctrl(KeyCode::Char('c')).into(),
                " exit".dim(),
            ]))
            .wrap(Wrap { trim: false })
            .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        let panel = Rect {
            height: column.desired_height(area.width).min(area.height),
            ..area
        };
        render_menu_surface(panel, buf);
        column.render(panel, buf);
    }
}

pub(crate) struct OssProviderSelection {
    pub(crate) provider: String,
    pub(crate) manually_selected: bool,
}

pub(crate) enum OssProviderDetection {
    AutoSelected(OssProviderSelection),
    NeedsSelection {
        lmstudio_status: ProviderStatus,
        ollama_status: ProviderStatus,
    },
}

/// Probe local providers without suspending the interactive startup composer.
pub(crate) async fn detect_oss_provider() -> OssProviderDetection {
    // These probes intentionally bypass proxy discovery because both targets are
    // hardcoded plaintext loopback endpoints. Preserve the legacy custom-CA fallback so an
    // invalid inherited certificate bundle cannot prevent best-effort provider detection.
    #[allow(deprecated)]
    let client = HttpClientBuilder::new().build_direct_with_custom_ca_fallback();

    // Check provider statuses first
    let lmstudio_status = check_lmstudio_status(&client).await;
    let ollama_status = check_ollama_status(&client).await;

    // Autoselect if only one is running
    match (&lmstudio_status, &ollama_status) {
        (ProviderStatus::Running, ProviderStatus::NotRunning) => {
            let provider = LMSTUDIO_OSS_PROVIDER_ID.to_string();
            OssProviderDetection::AutoSelected(OssProviderSelection {
                provider,
                manually_selected: false,
            })
        }
        (ProviderStatus::NotRunning, ProviderStatus::Running) => {
            let provider = OLLAMA_OSS_PROVIDER_ID.to_string();
            OssProviderDetection::AutoSelected(OssProviderSelection {
                provider,
                manually_selected: false,
            })
        }
        _ => OssProviderDetection::NeedsSelection {
            lmstudio_status,
            ollama_status,
        },
    }
}

/// Run the actionable provider picker after provider discovery requires a user decision.
pub(crate) async fn select_oss_provider(
    lmstudio_status: ProviderStatus,
    ollama_status: ProviderStatus,
) -> io::Result<OssProviderSelection> {
    let mut widget = OssSelectionWidget::new(lmstudio_status, ollama_status);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        terminal.draw(|f| {
            (&widget).render_ref(f.area(), f.buffer_mut());
        })?;
        crate::tui::discard_pending_terminal_input()?;

        loop {
            if let Event::Key(key_event) = event::read()?
                && let Some(selection) = widget.handle_key_event(key_event)
            {
                break Ok(OssProviderSelection {
                    provider: selection,
                    manually_selected: true,
                });
            }

            terminal.draw(|f| {
                (&widget).render_ref(f.area(), f.buffer_mut());
            })?;
        }
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn check_lmstudio_status(client: &HttpClient) -> ProviderStatus {
    match check_port_status(client, DEFAULT_LMSTUDIO_PORT).await {
        Ok(true) => ProviderStatus::Running,
        Ok(false) => ProviderStatus::NotRunning,
        Err(_) => ProviderStatus::Unknown,
    }
}

async fn check_ollama_status(client: &HttpClient) -> ProviderStatus {
    match check_port_status(client, DEFAULT_OLLAMA_PORT).await {
        Ok(true) => ProviderStatus::Running,
        Ok(false) => ProviderStatus::NotRunning,
        Err(_) => ProviderStatus::Unknown,
    }
}

async fn check_port_status(client: &HttpClient, port: u16) -> io::Result<bool> {
    let url = format!("http://localhost:{port}");

    match client
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
    {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false), // Connection failed = not running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_h_l_move_provider_selection() {
        let mut widget = OssSelectionWidget::new(ProviderStatus::Unknown, ProviderStatus::Unknown);

        assert_eq!(widget.selected_option, 0);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(widget.selected_option, 1);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(widget.selected_option, 0);
    }

    #[tokio::test]
    async fn localhost_probe_succeeds_with_invalid_inherited_ca_bundle() {
        const CHILD_ENV: &str = "CODEX_OSS_SELECTION_INVALID_CA_TEST_CHILD";

        if std::env::var_os(CHILD_ENV).is_none() {
            let temp_dir = tempfile::tempdir().expect("temporary directory should be created");
            let invalid_ca_path = temp_dir.path().join("invalid-ca.pem");
            std::fs::write(&invalid_ca_path, "not a PEM certificate")
                .expect("invalid CA fixture should be written");

            for ca_env in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
                let output = std::process::Command::new(
                    std::env::current_exe().expect("test executable should be available"),
                )
                .arg("--exact")
                .arg("oss_selection::tests::localhost_probe_succeeds_with_invalid_inherited_ca_bundle")
                .arg("--nocapture")
                .env_remove("CODEX_CA_CERTIFICATE")
                .env_remove("SSL_CERT_FILE")
                .env(ca_env, &invalid_ca_path)
                .env(CHILD_ENV, "1")
                .output()
                .expect("isolated CA subprocess should run");

                assert!(
                    output.status.success(),
                    "localhost probe failed with invalid {ca_env}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
            }
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        #[allow(deprecated)]
        let client = HttpClientBuilder::new().build_direct_with_custom_ca_fallback();
        assert!(
            check_port_status(&client, server.address().port())
                .await
                .expect("localhost provider probe should complete")
        );
    }
}

#[cfg(test)]
#[path = "oss_selection_tests.rs"]
mod presentation_tests;
