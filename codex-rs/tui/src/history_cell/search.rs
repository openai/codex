//! Compact web activity summaries with complete details in the transcript and raw output.

use super::*;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;

fn web_search_action_detail(action: &WebSearchAction) -> String {
    match action {
        WebSearchAction::Search { query, queries } => query
            .clone()
            .filter(|q| !q.is_empty())
            .unwrap_or_else(|| queries.as_deref().unwrap_or_default().join(", ")),
        WebSearchAction::OpenPage { url } => url.clone().unwrap_or_default(),
        WebSearchAction::FindInPage { url, pattern } => match (
            pattern.as_deref().filter(|pattern| !pattern.is_empty()),
            url.as_deref().filter(|url| !url.is_empty()),
        ) {
            (Some(pattern), Some(url)) => format!("'{pattern}' in {url}"),
            (Some(pattern), None) => format!("'{pattern}'"),
            (None, Some(url)) => url.to_string(),
            (None, None) => String::new(),
        },
        WebSearchAction::Other => String::new(),
    }
}

fn web_search_detail(action: Option<&WebSearchAction>, query: &str) -> String {
    let detail = action.map(web_search_action_detail).unwrap_or_default();
    if detail.is_empty() {
        query.to_string()
    } else {
        detail
    }
}

#[derive(Debug)]
pub(crate) struct WebSearchCell {
    call_id: String,
    query: String,
    action: Option<WebSearchAction>,
    start_time: Instant,
    completed: bool,
    animations_enabled: bool,
}

impl WebSearchCell {
    pub(crate) fn new(
        call_id: String,
        query: String,
        action: Option<WebSearchAction>,
        animations_enabled: bool,
    ) -> Self {
        Self {
            call_id,
            query,
            action,
            start_time: Instant::now(),
            completed: false,
            animations_enabled,
        }
    }

    pub(crate) fn call_id(&self) -> &str {
        &self.call_id
    }

    pub(crate) fn update(&mut self, action: WebSearchAction, query: String) {
        self.action = Some(action);
        self.query = query;
    }

    pub(crate) fn complete(&mut self) {
        self.completed = true;
    }

    fn summary(&self) -> Line<'static> {
        let detail = web_search_detail(self.action.as_ref(), &self.query);
        let (header, separator) = match (&self.action, self.completed) {
            (Some(WebSearchAction::OpenPage { .. }), completed) => (
                match (completed, detail.is_empty()) {
                    (true, true) => "Opened page",
                    (true, false) => "Opened",
                    (false, true) => "Opening page",
                    (false, false) => "Opening",
                },
                " ",
            ),
            (Some(WebSearchAction::FindInPage { pattern, .. }), completed) => {
                let has_pattern = pattern.as_ref().is_some_and(|pattern| !pattern.is_empty());
                (
                    match (completed, has_pattern) {
                        (true, true) => "Searched for",
                        (true, false) => "Searched page",
                        (false, true) => "Searching for",
                        (false, false) => "Searching page",
                    },
                    " ",
                )
            }
            (None | Some(WebSearchAction::Other), false) if detail.is_empty() => {
                ("Browsing the web", " ")
            }
            (Some(WebSearchAction::Search { .. } | WebSearchAction::Other) | None, completed) => (
                if completed {
                    "Searched the web"
                } else {
                    "Searching the web"
                },
                " for ",
            ),
        };
        let mut line = Line::from(header.bold());
        if !detail.is_empty() {
            line.extend([separator.into(), detail.into()]);
        }
        line
    }
}

impl HistoryCell for WebSearchCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let bullet = if self.completed {
            "•".dim()
        } else {
            activity_indicator(
                Some(self.start_time),
                MotionMode::from_animations_enabled(self.animations_enabled),
                ReducedMotionIndicator::StaticBullet,
            )
            .unwrap_or_else(|| "•".dim())
        };
        let mut line = Line::from(vec![bullet, " ".into()]);
        let mut summary = self.summary();
        for span in &mut summary.spans {
            span.content = span.content.replace(['\n', '\r', '\t'], " ").into();
        }
        line.extend(summary);
        vec![truncate_line_with_ellipsis_if_overflow(
            line,
            width as usize,
        )]
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        PrefixedWrappedHistoryCell::new(self.summary(), vec!["• ".dim()], "  ").display_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(vec![self.summary()])
    }
}

pub(crate) fn new_active_web_search_call(
    call_id: String,
    query: String,
    animations_enabled: bool,
) -> WebSearchCell {
    WebSearchCell::new(call_id, query, /*action*/ None, animations_enabled)
}

pub(crate) fn new_web_search_call(
    call_id: String,
    query: String,
    action: WebSearchAction,
) -> WebSearchCell {
    let mut cell = WebSearchCell::new(
        call_id,
        query,
        Some(action),
        /*animations_enabled*/ false,
    );
    cell.complete();
    cell
}
