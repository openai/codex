//! Informational, warning, update, and policy notice history cells.

use super::*;
use crate::line_truncation::line_width;
use crate::wrapping::url_preserving_wrap_options;
use crate::wrapping::word_wrap_line;

#[cfg_attr(not(test), allow(dead_code))]
const RECAP_HEADING: &str = "Conversation recap";

#[cfg_attr(debug_assertions, allow(dead_code))]
#[derive(Debug)]
pub(crate) struct UpdateAvailableHistoryCell {
    latest_version: String,
    update_action: Option<UpdateAction>,
}

#[cfg_attr(debug_assertions, allow(dead_code))]
impl UpdateAvailableHistoryCell {
    pub(crate) fn new(latest_version: String, update_action: Option<UpdateAction>) -> Self {
        Self {
            latest_version,
            update_action,
        }
    }
}

impl HistoryCell for UpdateAvailableHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        use ratatui_macros::line;
        use ratatui_macros::text;
        let update_instruction = if let Some(update_action) = self.update_action {
            line!["Run ", update_action.command_str().cyan(), " to update."]
        } else {
            line![
                "See ",
                "https://github.com/openai/codex".cyan().underlined(),
                " for installation options."
            ]
        };

        let content = text![
            line![
                "✨\u{200A}".bold().cyan(),
                "Update available!".bold().cyan(),
                " ",
                format!("{CODEX_CLI_VERSION} -> {}", self.latest_version).bold(),
            ],
            update_instruction,
            "",
            "See full release notes:",
            "https://github.com/openai/codex/releases/latest"
                .cyan()
                .underlined(),
        ];

        let inner_width = content
            .width()
            .min(usize::from(width.saturating_sub(4)))
            .max(1);
        let lines = adaptive_wrap_lines(content.lines, RtOptions::new(inner_width));
        with_border_with_inner_width(lines, inner_width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let update_instruction = if let Some(update_action) = self.update_action {
            format!("Run {} to update.", update_action.command_str())
        } else {
            "See https://github.com/openai/codex for installation options.".to_string()
        };
        vec![
            Line::from("Update available!"),
            Line::from(format!("{CODEX_CLI_VERSION} -> {}", self.latest_version)),
            Line::from(update_instruction),
            Line::from(""),
            Line::from("See full release notes:"),
            Line::from("https://github.com/openai/codex/releases/latest"),
        ]
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        crate::terminal_hyperlinks::annotate_web_urls(self.display_lines(width))
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }
}
#[allow(clippy::disallowed_methods)]
pub(crate) fn new_warning_event(message: String) -> PrefixedWrappedHistoryCell {
    PrefixedWrappedHistoryCell::new(message.yellow(), "⚠ ".yellow(), "  ")
}

#[allow(clippy::disallowed_methods)]
pub(crate) fn new_server_version_warning(
    notice: crate::status::remote_connection::ServerVersionNotice,
) -> PrefixedWrappedHistoryCell {
    let mut lines = vec![Line::from(notice.message.yellow())];
    if notice.offer_update {
        lines.push(Line::from("To update the service, run:".yellow()));
        lines.push(Line::from("  codex app-server daemon update".cyan()));
        lines.push(Line::from(
            "Updating may interrupt active or queued work.".yellow(),
        ));
    }
    PrefixedWrappedHistoryCell::new(Text::from(lines), "⚠ ".yellow(), "  ")
}

#[derive(Debug)]
pub(crate) struct SafetyAccessBlockCell {
    title: &'static str,
    body: &'static str,
    actions: &'static [(&'static str, &'static str)],
}

const SAFETY_ACCESS_BLOCK_LEARN_MORE_URL: &str = "https://help.openai.com/en/articles/20001326";

pub(crate) fn new_safety_access_block_event() -> SafetyAccessBlockCell {
    SafetyAccessBlockCell {
        title: "This content can't be shown",
        body: "We take extra caution with requests involving biological research and applications that could pose safety risks. Eligible researchers can apply for Trusted Access.",
        actions: &[
            (
                "Trusted Access",
                "https://chatgpt.com/r/b749fb02595e04c3007a54375f3f4374",
            ),
            ("Learn more", SAFETY_ACCESS_BLOCK_LEARN_MORE_URL),
        ],
    }
}

pub(crate) fn new_cyber_policy_error_event(
    notice: crate::daybreak::Notice,
) -> SafetyAccessBlockCell {
    use crate::daybreak::Notice;
    let (body, actions): (_, &'static [(&str, &str)]) = match notice {
        Notice::Apply => (
            "We take extra care with some cybersecurity requests. If you’re doing authorized security work, apply for Daybreak to get broader access.",
            &[
                ("Learn more", SAFETY_ACCESS_BLOCK_LEARN_MORE_URL),
                (
                    "Apply for Daybreak",
                    "https://openai.com/form/enterprise-trusted-access-for-cyber/",
                ),
            ],
        ),
        Notice::Astra => (
            "Daybreak isn’t available for Astra. Some cybersecurity requests may still be limited.",
            &[("Learn more", SAFETY_ACCESS_BLOCK_LEARN_MORE_URL)],
        ),
        Notice::Limited => (
            "We take extra care with some cybersecurity requests.",
            &[("Learn more", SAFETY_ACCESS_BLOCK_LEARN_MORE_URL)],
        ),
    };
    SafetyAccessBlockCell {
        title: "This content can’t be shown",
        body,
        actions,
    }
}

impl HistoryCell for SafetyAccessBlockCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = vec![HyperlinkLine::new(
            vec!["ⓘ ".cyan(), self.title.bold()].into(),
        )];
        let body = Line::from(vec!["  ".into(), self.body.dim()]);
        let wrap_width = width.saturating_sub(2).max(1) as usize;
        let wrapped = adaptive_wrap_line(
            &body,
            RtOptions::new(wrap_width).subsequent_indent("  ".into()),
        );
        let mut wrapped_body = Vec::new();
        push_owned_lines(&wrapped, &mut wrapped_body);
        lines.extend(plain_hyperlink_lines(wrapped_body));

        for &(label, url) in self.actions {
            let source = crate::terminal_hyperlinks::annotate_web_urls_in_line(
                vec![format!("  {label}: ").dim(), url.cyan().underlined()].into(),
            );
            let wrapped = crate::wrapping::word_wrap_line(
                &source.line,
                RtOptions::new(wrap_width).subsequent_indent("  ".into()),
            );
            let mut wrapped_links = Vec::new();
            push_owned_lines(&wrapped, &mut wrapped_links);
            lines.extend(crate::terminal_hyperlinks::remap_wrapped_line(
                &source,
                wrapped_links,
            ));
        }
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(self.title), Line::from(self.body)];
        lines.extend(
            self.actions
                .iter()
                .map(|(label, target)| Line::from(format!("{label}: {target}"))),
        );
        lines
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }
}

#[derive(Debug)]
pub(crate) struct DeprecationNoticeCell {
    summary: String,
    details: Option<String>,
}

pub(crate) fn new_deprecation_notice(
    summary: String,
    details: Option<String>,
) -> DeprecationNoticeCell {
    DeprecationNoticeCell { summary, details }
}

impl HistoryCell for DeprecationNoticeCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(vec!["⚠ ".red().bold(), self.summary.clone().red()].into());

        let wrap_width = width.saturating_sub(4).max(1) as usize;

        if let Some(details) = &self.details {
            let detail_line = Line::from(details.clone().dim());
            let wrapped = adaptive_wrap_line(&detail_line, RtOptions::new(wrap_width));
            push_owned_lines(&wrapped, &mut lines);
        }

        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(self.summary.clone())];
        if let Some(details) = &self.details {
            lines.extend(raw_lines_from_source(details));
        }
        lines
    }
}
pub(crate) fn new_info_event(message: String, hint: Option<String>) -> PlainHistoryCell {
    let mut line = vec!["• ".dim(), message.into()];
    if let Some(hint) = hint {
        line.push(" ".into());
        line.push(hint.dark_gray());
    }
    let lines: Vec<Line<'static>> = vec![line.into()];
    PlainHistoryCell { lines }
}

pub(crate) fn new_error_event(message: String) -> PlainHistoryCell {
    // Use a hair space (U+200A) to create a subtle, near-invisible separation
    // before the text. VS16 is intentionally omitted to keep spacing tighter
    // in terminals like Ghostty.
    let lines: Vec<Line<'static>> = vec![vec![format!("■ {message}").red()].into()];
    PlainHistoryCell { lines }
}

#[derive(Debug)]
pub(crate) struct ThreadRecapLoadingCell {
    start_time: Instant,
    animations_enabled: bool,
}

impl ThreadRecapLoadingCell {
    pub(crate) fn new(animations_enabled: bool) -> Self {
        Self {
            start_time: Instant::now(),
            animations_enabled,
        }
    }
}

impl HistoryCell for ThreadRecapLoadingCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![
            vec![
                activity_indicator(
                    Some(self.start_time),
                    MotionMode::from_animations_enabled(self.animations_enabled),
                    ReducedMotionIndicator::StaticBullet,
                )
                .unwrap_or_else(|| "•".dim()),
                " ".into(),
                "Generating conversation recap".bold(),
                "…".dim(),
            ]
            .into(),
        ]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from("Generating conversation recap...")]
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        if !self.animations_enabled {
            return None;
        }

        Some((self.start_time.elapsed().as_millis() / 50) as u64)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct ThreadRecapHistoryCell {
    recap: String,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ThreadRecapHistoryCell {
    pub(crate) fn new(recap: String) -> Self {
        Self { recap }
    }
}

impl HistoryCell for ThreadRecapHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }

        let wrap_width = usize::from(width.saturating_sub(/*rhs*/ 2).max(/*other*/ 1));
        let mut body = raw_lines_from_source(&self.recap)
            .into_iter()
            .map(Line::italic)
            .collect::<Vec<_>>();
        let prefix = Line::from(vec!["  ".into(), "↳ ".dim(), "Recap: ".bold()]).italic();
        let mut options = if wrap_width <= prefix.width() {
            // Keep the text readable when the terminal cannot fit the hanging indent.
            body.insert(
                /*index*/ 0,
                Line::from(vec!["↳ ".dim(), "Recap:".bold()]).italic(),
            );
            RtOptions::new(wrap_width)
        } else {
            RtOptions::new(wrap_width)
                .subsequent_indent(" ".repeat(prefix.width()).into())
                .initial_indent(prefix)
        };
        let mut lines = Vec::new();
        for line in body {
            let mut wrapped = adaptive_wrap_line(&line, options.clone());
            if wrapped.iter().any(|line| line_width(line) > wrap_width) {
                // Terminal autowrap loses the hanging indent. Rewrap the source line so only
                // oversized tokens split, without applying the recap prefix a second time.
                wrapped = word_wrap_line(
                    &line,
                    url_preserving_wrap_options(options.clone())
                        .break_words(/*break_words*/ true),
                );
            }
            push_owned_lines(&wrapped, &mut lines);
            options.initial_indent = options.subsequent_indent.clone();
        }
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(RECAP_HEADING)];
        lines.extend(raw_lines_from_source(&self.recap));
        lines
    }
}
