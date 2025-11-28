use super::CompositeHistoryCell;
use super::HistoryCell;
use super::with_border;
use crate::exec_command::relativize_to_home;
use codex_core::config::Config;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::path::Path;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

/// Boxed header that shows the current model, reasoning effort, and working directory.
///
/// Rendered at the top of a session in the history panel to orient the user before any exchanges
/// occur. Uses a bordered card with a dim prompt prefix, bold title, model line (including
/// reasoning effort when configured), and the current working directory.
///
/// # Output
///
/// ```plain
/// ╭──────────────────────────────╮
/// │ >_ OpenAI Codex (vX.Y.Z)     │
/// │                              │
/// │ model: gpt-4o high   /model …│
/// │ directory: ~/code/project    │
/// ╰──────────────────────────────╯
/// ```
#[derive(Debug)]
pub(crate) struct SessionHeaderHistoryCell {
    version: &'static str,
    model: String,
    reasoning_effort: Option<ReasoningEffortConfig>,
    directory: PathBuf,
}

impl SessionHeaderHistoryCell {
    pub(crate) fn new(
        model: String,
        reasoning_effort: Option<ReasoningEffortConfig>,
        directory: PathBuf,
        version: &'static str,
    ) -> Self {
        Self {
            version,
            model,
            reasoning_effort,
            directory,
        }
    }

    /// Format a directory path with `~` shorthand and optional truncation to fit the card width.
    pub(crate) fn format_directory(&self, max_width: Option<usize>) -> String {
        Self::format_directory_inner(&self.directory, max_width)
    }

    fn format_directory_inner(directory: &Path, max_width: Option<usize>) -> String {
        let formatted = if let Some(rel) = relativize_to_home(directory) {
            if rel.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display())
            }
        } else {
            directory.display().to_string()
        };

        if let Some(max_width) = max_width {
            if max_width == 0 {
                return String::new();
            }
            if UnicodeWidthStr::width(formatted.as_str()) > max_width {
                return crate::text_formatting::center_truncate_path(&formatted, max_width);
            }
        }

        formatted
    }

    fn reasoning_label(&self) -> Option<&'static str> {
        self.reasoning_effort.map(|effort| match effort {
            ReasoningEffortConfig::Minimal => "minimal",
            ReasoningEffortConfig::Low => "low",
            ReasoningEffortConfig::Medium => "medium",
            ReasoningEffortConfig::High => "high",
            ReasoningEffortConfig::XHigh => "xhigh",
            ReasoningEffortConfig::None => "none",
        })
    }
}

impl HistoryCell for SessionHeaderHistoryCell {
    /// Render the banner boxed with model/version and directory lines sized to the available width.
    ///
    /// Computes the inner card width (clamped to `SESSION_HEADER_MAX_INNER_WIDTH`), then lays out
    /// a prompt-style title, a blank spacer line, a model line with reasoning label and `/model`
    /// hint, and a directory line with truncation as needed. The entire block is wrapped in a light
    /// border.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(inner_width) = card_inner_width(width, SESSION_HEADER_MAX_INNER_WIDTH) else {
            return Vec::new();
        };

        let make_row = |spans: Vec<Span<'static>>| Line::from(spans);

        let title_spans: Vec<Span<'static>> = vec![
            Span::from(">_ ").dim(),
            Span::from("OpenAI Codex").bold(),
            Span::from(" ").dim(),
            Span::from(format!("(v{})", self.version)).dim(),
        ];

        const CHANGE_MODEL_HINT_COMMAND: &str = "/model";
        const CHANGE_MODEL_HINT_EXPLANATION: &str = " to change";
        const DIR_LABEL: &str = "directory:";
        let label_width = DIR_LABEL.len();
        let model_label = format!(
            "{model_label:<label_width$}",
            model_label = "model:",
            label_width = label_width
        );
        let reasoning_label = self.reasoning_label();
        let mut model_spans: Vec<Span<'static>> = vec![
            Span::from(format!("{model_label} ")).dim(),
            Span::from(self.model.clone()),
        ];
        if let Some(reasoning) = reasoning_label {
            model_spans.push(Span::from(" "));
            model_spans.push(Span::from(reasoning));
        }
        model_spans.push("   ".dim());
        model_spans.push(CHANGE_MODEL_HINT_COMMAND.cyan());
        model_spans.push(CHANGE_MODEL_HINT_EXPLANATION.dim());

        let dir_label = format!("{DIR_LABEL:<label_width$}");
        let dir_prefix = format!("{dir_label} ");
        let dir_prefix_width = UnicodeWidthStr::width(dir_prefix.as_str());
        let dir_max_width = inner_width.saturating_sub(dir_prefix_width);
        let dir = self.format_directory(Some(dir_max_width));
        let dir_spans = vec![Span::from(dir_prefix).dim(), Span::from(dir)];

        let lines = vec![
            make_row(title_spans),
            make_row(Vec::new()),
            make_row(model_spans),
            make_row(dir_spans),
        ];

        with_border(lines)
    }
}

/// Composite cell used for the session header and onboarding hints.
///
/// Displays the initial "OpenAI Codex" banner with model/directory info and a short help list when
/// a session starts, or an empty placeholder when reconfiguring without changes. Combines a header
/// card with plain help lines so the history entry is a single block.
///
/// # Output
///
/// ```plain
/// ╭─ header card ─╮
/// │ model / dir   │
/// ╰───────────────╯
///
///   To get started...
///   /init - ...
/// ```
#[derive(Debug)]
pub struct SessionInfoCell(CompositeHistoryCell);

impl SessionInfoCell {
    pub(crate) fn new(
        config: &Config,
        event: SessionConfiguredEvent,
        is_first_event: bool,
    ) -> Self {
        let SessionConfiguredEvent {
            model,
            reasoning_effort,
            ..
        } = event;
        if is_first_event {
            let header = SessionHeaderHistoryCell::new(
                model,
                reasoning_effort,
                config.cwd.clone(),
                crate::version::CODEX_CLI_VERSION,
            );

            let help_lines: Vec<Line<'static>> = vec![
                "  To get started, describe a task or try one of these commands:"
                    .dim()
                    .into(),
                Line::from(""),
                Line::from(vec![
                    "  ".into(),
                    "/init".into(),
                    " - create an AGENTS.md file with instructions for Codex".dim(),
                ]),
                Line::from(vec![
                    "  ".into(),
                    "/status".into(),
                    " - show current session configuration".dim(),
                ]),
                Line::from(vec![
                    "  ".into(),
                    "/approvals".into(),
                    " - choose what Codex can do without approval".dim(),
                ]),
                Line::from(vec![
                    "  ".into(),
                    "/model".into(),
                    " - choose what model and reasoning effort to use".dim(),
                ]),
                Line::from(vec![
                    "  ".into(),
                    "/review".into(),
                    " - review any changes and find issues".dim(),
                ]),
            ];

            Self(CompositeHistoryCell {
                parts: vec![
                    Box::new(header),
                    Box::new(crate::history_cell::PlainHistoryCell { lines: help_lines }),
                ],
            })
        } else if config.model == model {
            Self(CompositeHistoryCell { parts: vec![] })
        } else {
            let lines = vec![
                "model changed:".magenta().bold().into(),
                format!("requested: {}", config.model).into(),
                format!("used: {model}").into(),
            ];

            Self(CompositeHistoryCell {
                parts: vec![Box::new(crate::history_cell::PlainHistoryCell { lines })],
            })
        }
    }
}

pub(crate) fn new_session_info(
    config: &Config,
    event: SessionConfiguredEvent,
    is_first_event: bool,
) -> SessionInfoCell {
    SessionInfoCell::new(config, event, is_first_event)
}

impl HistoryCell for SessionInfoCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.0.display_lines(width)
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.0.desired_height(width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.0.transcript_lines(width)
    }
}

pub(crate) const SESSION_HEADER_MAX_INNER_WIDTH: usize = 56; // eyeballed

pub(crate) fn card_inner_width(width: u16, max_inner_width: usize) -> Option<usize> {
    if width < 4 {
        return None;
    }
    let inner_width = std::cmp::min(width.saturating_sub(4) as usize, max_inner_width);
    Some(inner_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;
    use codex_core::protocol::AskForApproval;
    use codex_core::protocol::SandboxPolicy;
    use codex_protocol::ConversationId;
    use dirs::home_dir;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config")
    }

    #[test]
    fn includes_reasoning_level_when_present() {
        let cell = SessionHeaderHistoryCell::new(
            "gpt-4o".to_string(),
            Some(ReasoningEffortConfig::High),
            std::env::temp_dir(),
            "test",
        );

        let lines: Vec<String> = cell
            .display_string(80)
            .split('\n')
            .map(std::string::ToString::to_string)
            .collect();
        let model_line = lines
            .iter()
            .find(|line| line.contains("model:"))
            .expect("model line");

        assert!(model_line.contains("gpt-4o high"));
        assert!(model_line.contains("/model to change"));
    }

    #[test]
    fn directory_center_truncates_for_nested_paths() {
        let mut dir = home_dir().expect("home directory");
        for part in ["hello", "the", "fox", "is", "very", "fast"] {
            dir.push(part);
        }

        let formatted = SessionHeaderHistoryCell::format_directory_inner(&dir, Some(24));
        let sep = std::path::MAIN_SEPARATOR;
        let expected = format!("~{sep}hello{sep}the{sep}…{sep}very{sep}fast");
        assert_eq!(formatted, expected);
    }

    #[test]
    fn directory_front_truncates_long_segment() {
        let mut dir = home_dir().expect("home directory");
        dir.push("supercalifragilisticexpialidocious");

        let formatted = SessionHeaderHistoryCell::format_directory_inner(&dir, Some(18));
        let sep = std::path::MAIN_SEPARATOR;
        let expected = format!("~{sep}…cexpialidocious");
        assert_eq!(formatted, expected);
    }

    #[test]
    fn session_info_renders_header_and_help() {
        let config = test_config();
        let cell = SessionInfoCell::new(
            &config,
            SessionConfiguredEvent {
                session_id: ConversationId::new(),
                model: "gpt-4o".into(),
                model_provider_id: "test".into(),
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: SandboxPolicy::ReadOnly,
                cwd: config.cwd.clone(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: PathBuf::from("/tmp/rollout"),
            },
            true,
        );

        let rendered = cell.display_string(60);
        assert!(rendered.contains("OpenAI Codex"));
        assert!(rendered.contains("/model"));
        assert!(rendered.contains("/status"));
    }

    #[test]
    fn session_header_full_width() {
        let cell = SessionHeaderHistoryCell::new(
            "gpt-4o-mini".to_string(),
            Some(ReasoningEffortConfig::High),
            PathBuf::from("/Users/me/projects/codex"),
            "1.2.3",
        );

        assert_snapshot!(cell.display_string(72));
    }

    #[test]
    fn session_header_truncates_directory() {
        let cell = SessionHeaderHistoryCell::new(
            "gpt-4o-mini".to_string(),
            None,
            PathBuf::from("/Users/me/projects/codex"),
            "1.2.3",
        );

        assert_snapshot!(cell.display_string(36));
    }

    #[test]
    fn session_info_includes_help() {
        let config = test_config();
        let cell = SessionInfoCell::new(
            &config,
            SessionConfiguredEvent {
                session_id: ConversationId::new(),
                model: "gpt-4o".into(),
                model_provider_id: "test".into(),
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: SandboxPolicy::ReadOnly,
                cwd: config.cwd.clone(),
                reasoning_effort: Some(ReasoningEffortConfig::Medium),
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: PathBuf::from("/tmp/rollout"),
            },
            true,
        );

        assert_snapshot!(cell.display_string(70));
    }
}
