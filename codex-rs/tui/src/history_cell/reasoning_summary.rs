use super::HistoryCell;
use crate::markdown::append_markdown;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Model-provided reasoning summary shown alongside or inside the transcript.
///
/// Captures the assistant’s self-reported reasoning buffer so users can read concise bullet points
/// without inspecting raw deltas. When `transcript_only` is true, the summary is omitted from the
/// on-screen history and only emitted in exports. Renders as a dim, italic bullet with hanging
/// indent when visible.
///
/// # Output
///
/// ```plain
/// • we wrap the summary with a hanging indent
/// ```
#[derive(Debug)]
pub(crate) struct ReasoningSummaryCell {
    _header: String,
    content: String,
    transcript_only: bool,
}

impl ReasoningSummaryCell {
    /// Create a reasoning summary entry anchored to the assistant’s optional header.
    ///
    /// `content` is the markdown-formatted summary text; when `transcript_only` is true the summary
    /// is omitted from the on-screen history while remaining in transcript exports.
    pub(crate) fn new(header: String, content: String, transcript_only: bool) -> Self {
        Self {
            _header: header,
            content,
            transcript_only,
        }
    }

    /// Render summary content as dim, italic bullet lines.
    ///
    /// Parses markdown to spans, dims/italicizes the text, then wraps with a bullet and hanging
    /// indent so multi-line summaries stay aligned within the available width.
    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        append_markdown(
            &self.content,
            Some((width as usize).saturating_sub(2)),
            &mut lines,
        );
        let summary_style = Style::default().dim().italic();
        let summary_lines = lines
            .into_iter()
            .map(|mut line| {
                line.spans = line
                    .spans
                    .into_iter()
                    .map(|span| span.patch_style(summary_style))
                    .collect();
                line
            })
            .collect::<Vec<_>>();

        // Render as a bullet with hanging indent so multi-line reasoning stays aligned.
        word_wrap_lines(
            &summary_lines,
            RtOptions::new(width as usize)
                .initial_indent("• ".dim().into())
                .subsequent_indent("  ".into()),
        )
    }
}

impl HistoryCell for ReasoningSummaryCell {
    /// Return dim, italic bullet lines for the reasoning summary or hide when transcript-only.
    ///
    /// When `transcript_only` is false, wraps markdown-rendered content to the given width with a
    /// dim bullet and hanging indent. When true, returns an empty vector so on-screen history omits
    /// the summary, but `transcript_lines` still returns the full wrapped content.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.transcript_only {
            Vec::new()
        } else {
            self.lines(width)
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        if self.transcript_only {
            0
        } else {
            self.lines(width).len() as u16
        }
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.lines(width)
    }

    fn desired_transcript_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }
}

/// Construct a reasoning summary cell, stripping any experimental header markup when configured.
pub(crate) fn new_reasoning_summary_block(
    full_reasoning_buffer: String,
    config: &codex_core::config::Config,
) -> Box<dyn HistoryCell> {
    use codex_core::config::types::ReasoningSummaryFormat;

    if config.model_family.reasoning_summary_format == ReasoningSummaryFormat::Experimental {
        let full_reasoning_buffer = full_reasoning_buffer.trim();
        if let Some(open) = full_reasoning_buffer.find("**") {
            let after_open = &full_reasoning_buffer[(open + 2)..];
            if let Some(close) = after_open.find("**") {
                let after_close_idx = open + 2 + close + 2;
                if after_close_idx < full_reasoning_buffer.len() {
                    let header_buffer = full_reasoning_buffer[..after_close_idx].to_string();
                    let summary_buffer = full_reasoning_buffer[after_close_idx..].to_string();
                    return Box::new(ReasoningSummaryCell::new(
                        header_buffer,
                        summary_buffer,
                        false,
                    ));
                }
            }
        }
    }
    Box::new(ReasoningSummaryCell::new(
        "".to_string(),
        full_reasoning_buffer,
        true,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;
    use codex_core::config::types::ReasoningSummaryFormat;
    use insta::assert_snapshot;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config")
    }

    #[test]
    fn hides_when_transcript_only() {
        let cell = ReasoningSummaryCell::new("".into(), "hidden".into(), true);
        assert!(cell.display_lines(80).is_empty());
        assert_eq!(cell.desired_height(80), 0);
        assert!(!cell.transcript_lines(80).is_empty());
    }

    #[test]
    fn renders_and_wraps_summary() {
        let cell = ReasoningSummaryCell::new(
            "".into(),
            "A fairly long reasoning line that will wrap in the bullet summary when narrow".into(),
            false,
        );

        let rendered = cell.display_string(30);
        assert!(
            rendered.starts_with('•'),
            "expected bullet prefix in reasoning summary"
        );
        assert!(
            rendered.contains('\n'),
            "expected wrapping when narrow summary width"
        );
    }

    #[test]
    fn visible_summary_wraps() {
        let cell = ReasoningSummaryCell::new(
            "".into(),
            "We should refactor the history cells into modules and add snapshot coverage to lock rendering behavior."
                .into(),
            false,
        );

        assert_snapshot!(cell.display_string(46));
    }

    #[test]
    fn experimental_blocking_logic_matches_helper() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cell = ReasoningSummaryCell::new("header".into(), "body".into(), false);
        let rendered = cell.display_string(80);
        assert!(rendered.contains("body"));
    }

    #[test]
    fn splits_header_and_summary_when_present() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cell = new_reasoning_summary_block(
            "**High level plan**\n\nWe should fix the bug next.".to_string(),
            &config,
        );

        let rendered_display = cell.display_string(80);
        let rendered_transcript = cell.transcript_string(80);
        assert_eq!(rendered_display, "• We should fix the bug next.");
        assert_eq!(rendered_transcript, "• We should fix the bug next.");
    }

    #[test]
    fn falls_back_when_header_is_missing() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cell = new_reasoning_summary_block(
            "**High level reasoning without closing".to_string(),
            &config,
        );

        let rendered = cell.transcript_string(80);
        assert_eq!(rendered, "• **High level reasoning without closing");
    }

    #[test]
    fn falls_back_when_summary_is_missing() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cell = new_reasoning_summary_block(
            "**High level reasoning without closing**".to_string(),
            &config,
        );
        assert_eq!(
            cell.transcript_string(80),
            "• High level reasoning without closing"
        );

        let cell = new_reasoning_summary_block(
            "**High level reasoning without closing**\n\n  ".to_string(),
            &config,
        );
        assert_eq!(
            cell.transcript_string(80),
            "• High level reasoning without closing"
        );
    }

    #[test]
    fn returns_reasoning_cell_when_feature_disabled() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cell =
            new_reasoning_summary_block("Detailed reasoning goes here.".to_string(), &config);

        assert_eq!(
            cell.transcript_string(80),
            "• Detailed reasoning goes here."
        );
    }

    #[test]
    fn happy_path_header_and_summary() {
        let mut config = test_config();
        config.model_family.reasoning_summary_format = ReasoningSummaryFormat::Experimental;

        let cell = new_reasoning_summary_block(
            "**High level reasoning**\n\nDetailed reasoning goes here.".to_string(),
            &config,
        );

        assert_eq!(cell.display_string(80), "• Detailed reasoning goes here.");
        assert_eq!(
            cell.transcript_string(80),
            "• Detailed reasoning goes here."
        );
    }
}
