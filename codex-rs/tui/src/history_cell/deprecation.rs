use super::HistoryCell;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Alert that a feature or flag is deprecated.
///
/// Renders a red warning line and optional wrapped detail text so users can migrate without digging
/// into logs. Used for CLI warnings/informational notices in the history panel.
///
/// # Output
///
/// ```plain
/// ⚠ Feature flag `foo`
/// Use flag `bar` instead.
/// ```
#[derive(Debug)]
pub(crate) struct DeprecationNoticeCell {
    summary: String,
    details: Option<String>,
}

impl DeprecationNoticeCell {
    pub(crate) fn new(summary: String, details: Option<String>) -> Self {
        Self { summary, details }
    }
}

pub(crate) fn new_deprecation_notice(
    summary: String,
    details: Option<String>,
) -> DeprecationNoticeCell {
    DeprecationNoticeCell::new(summary, details)
}

impl HistoryCell for DeprecationNoticeCell {
    /// Render the summary in red with an optional wrapped detail following on subsequent lines.
    ///
    /// Uses a bold red `⚠` prefix and summary on the first line, then wraps `details` at
    /// `width-4` with dim styling so follow-up context is readable but secondary.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(vec!["⚠ ".red().bold(), self.summary.clone().red()].into());

        let wrap_width = width.saturating_sub(4).max(1) as usize;

        if let Some(details) = &self.details {
            let line = textwrap::wrap(details, wrap_width)
                .into_iter()
                .map(|s| s.to_string().dim().into())
                .collect::<Vec<_>>();
            lines.extend(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn deprecation_with_details_wraps() {
        let cell = DeprecationNoticeCell::new(
            "Feature flag `foo`".to_string(),
            Some("Use flag `bar` instead of relying on implicit defaults.".to_string()),
        );

        let rendered = cell.display_string(32);
        assert_snapshot!(rendered);
    }

    #[test]
    fn renders_summary_only_when_no_details() {
        let cell = DeprecationNoticeCell::new("Old endpoint deprecated".to_string(), None);
        let rendered = cell.display_string(40);

        assert_snapshot!(rendered);
    }
}
