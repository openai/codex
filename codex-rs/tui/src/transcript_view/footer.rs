//! Project transcript interactions and page-loading feedback into the existing footer.
//! Loading never inserts transcript rows; the owner schedules motion only while a page is pending.
//! Surface owners supply the latest shortcut because legacy pagers reserve Escape for editing.
//! Known navigation labels style keys separately from prose; transcript and query text stay untouched.

use super::*;
use crate::bottom_pane::TranscriptFooter;
use crate::footer_hint::first_fitting_line;
use crate::key_hint::key_label_spans;
use crate::motion::MotionMode;
use crate::motion::shimmer_text;
use crate::style::accent_color;
use crossterm::event::KeyCode;
use unicode_width::UnicodeWidthStr;

impl TranscriptView {
    pub(crate) fn footer_with_navigation(
        &self,
        width: u16,
        motion: MotionMode,
        latest_navigation: &str,
    ) -> Option<TranscriptFooter> {
        // A mouse-down anchor owns the gesture, but has no text to copy yet. Compare
        // source positions so movement across wrapping or padding is not a selection.
        let has_selected_text = self.selection.as_ref().is_some_and(|selection| {
            (selection.start.key, selection.start.offset)
                != (selection.end.key, selection.end.offset)
        });
        if let Some((query, cursor)) = self.search_footer(width) {
            return Some(TranscriptFooter {
                text: vec![
                    query,
                    if has_selected_text {
                        selection_hint(width)
                    } else {
                        self.search.status_line(width, self.history)
                    },
                ]
                .into(),
                cursor_column: self.selection.is_none().then_some(cursor),
                is_interactive: true,
            });
        }
        if has_selected_text {
            return Some(TranscriptFooter {
                text: first_fitting_line(
                    [
                        self.status_line_with_navigation(
                            "ctrl+c copy · enter copy & follow · esc clear",
                            motion,
                        ),
                        selection_hint(width),
                    ],
                    width,
                )
                .into(),
                cursor_column: None,
                is_interactive: true,
            });
        }
        if self.is_activity_focused() {
            return self.disclosure_footer(width);
        }
        let pending = self.is_loading_history() || self.history == TranscriptHistoryState::Failed;
        let can_return = self.selection.is_none() && self.can_return_to_latest();
        (pending || self.unseen_activity || can_return)
            .then(|| {
                let navigation = if self.can_return_to_latest() {
                    latest_navigation
                } else {
                    ""
                };
                let mut status = self.status_line_with_navigation(navigation, motion);
                // Loading must not resize the composer or move its caret. Shorten secondary text
                // before clipping either shortcut; failed loads prioritize their retry action.
                if status.width() > usize::from(width) {
                    status = if self.is_loading_history() {
                        let loading = if navigation.is_empty() {
                            if "↑ Loading…".width() <= usize::from(width) {
                                "Loading…"
                            } else {
                                "…"
                            }
                        } else if "↑ Loading… · ".width() + navigation.width() <= usize::from(width)
                        {
                            "Loading… · "
                        } else {
                            "… "
                        };
                        let mut spans = vec!["↑ ".fg(accent_color())];
                        spans.extend(shimmer_text(loading, motion));
                        spans.extend(navigation_line(navigation).spans);
                        Line::from(spans)
                    } else if self.history == TranscriptHistoryState::Failed {
                        let mut spans = vec!["Retry: ".dim()];
                        spans.extend(key_label_spans(&JumpTarget::Beginning.hint_label()));
                        Line::from(spans)
                    } else if navigation.is_empty() {
                        Line::from("New activity").dim()
                    } else if self.unseen_activity {
                        let mut spans = vec!["New · ".dim()];
                        spans.extend(navigation_line(navigation).spans);
                        Line::from(spans)
                    } else {
                        navigation_line(navigation)
                    };
                }
                let retry = if self.history == TranscriptHistoryState::Failed {
                    format!(
                        "{} retry",
                        crate::key_hint::ctrl(KeyCode::Home).display_label()
                    )
                } else {
                    navigation.to_owned()
                };
                TranscriptFooter {
                    text: first_fitting_line(
                        [status, navigation_line(&retry), navigation_line(navigation)],
                        width,
                    )
                    .into(),
                    cursor_column: None,
                    is_interactive: false,
                }
            })
            .or_else(|| self.disclosure_footer(width))
    }

    pub(crate) fn status_line_with_navigation(
        &self,
        navigation: &str,
        motion: MotionMode,
    ) -> Line<'static> {
        if self.is_loading_history() {
            let mut spans = vec!["↑ ".fg(accent_color())];
            spans.extend(shimmer_text("Loading earlier messages…", motion));
            if self.unseen_activity {
                spans.push(" · New activity".dim());
            }
            if !navigation.is_empty() {
                spans.push(" · ".dim());
                spans.extend(navigation_line(navigation).spans);
            }
            return Line::from(spans);
        }
        if self.selection.is_some() && self.history == TranscriptHistoryState::Failed {
            let mut spans = navigation_line(navigation).spans;
            spans.push(" · Retry: ".dim());
            spans.extend(key_label_spans(&JumpTarget::Beginning.hint_label()));
            return Line::from(spans);
        }
        let history = match self.history {
            TranscriptHistoryState::Failed => {
                let mut spans = vec!["Retry history: ".dim()];
                spans.extend(key_label_spans(&JumpTarget::Beginning.hint_label()));
                spans.push(".  ".dim());
                spans
            }
            TranscriptHistoryState::Partial => vec!["Earlier messages available.  ".dim()],
            TranscriptHistoryState::LoadingOlder
            | TranscriptHistoryState::LoadingBeginning
            | TranscriptHistoryState::Complete
            | TranscriptHistoryState::Idle => Vec::new(),
        };
        let activity = if self.unseen_activity {
            if history.is_empty() && navigation.is_empty() {
                "New activity"
            } else {
                "New activity · "
            }
        } else {
            ""
        };
        let mut spans = vec![activity.dim()];
        spans.extend(history);
        spans.extend(navigation_line(navigation).spans);
        Line::from(spans)
    }

    pub(crate) fn is_loading_history(&self) -> bool {
        matches!(
            self.history,
            TranscriptHistoryState::LoadingOlder | TranscriptHistoryState::LoadingBeginning
        )
    }
}

fn selection_hint(width: u16) -> Line<'static> {
    first_fitting_line(
        [
            "ctrl+c copy · enter copy & follow · esc clear",
            "enter copy & follow · esc clear",
            "enter copy+↓ · esc",
            "esc clear",
        ]
        .map(navigation_line),
        width,
    )
}

/// Style only the explicit key-label prefixes of trusted navigation hints.
///
/// Callers supply UI-authored hints, including runtime key chords followed by a known action.
/// Unknown segments remain dim prose; this must never be applied to transcript or query content.
pub(super) fn navigation_line(navigation: &str) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, hint) in navigation.split(" · ").enumerate() {
        if index > 0 {
            spans.push(" · ".dim());
        }
        let action = [
            " copy & follow",
            " clear selection",
            " copy+↓",
            " previous",
            " latest",
            " retry",
            " select",
            " copy",
            " clear",
            " next",
            " close",
        ]
        .into_iter()
        .find(|action| hint.ends_with(*action));
        if let Some(action) = action {
            spans.extend(key_label_spans(&hint[..hint.len() - action.len()]));
            spans.push(action.dim());
        } else if hint == "esc" {
            spans.extend(key_label_spans(hint));
        } else {
            spans.push(hint.to_owned().dim());
        }
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
