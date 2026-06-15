use codex_protocol::protocol::TokenUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoCompactWindowSnapshot {
    pub(crate) prefill_input_tokens: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoCompactWindowPrefill {
    ServerObserved(i64),
    Estimated(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AutoCompactWindow {
    window_id: u64,
    new_context_window_requested: bool,
    /// Absolute input-token baseline for the current compaction window.
    ///
    /// `body_after_prefix` subtracts this from later active-context usage. It is
    /// not the growth itself; server-observed usage replaces estimated
    /// resume/recompute baselines when available.
    prefill_input_tokens: Option<AutoCompactWindowPrefill>,
}

impl AutoCompactWindow {
    pub(super) fn new() -> Self {
        Self {
            window_id: 0,
            new_context_window_requested: false,
            prefill_input_tokens: None,
        }
    }

    pub(super) fn clear_prefill(&mut self) {
        self.prefill_input_tokens = None;
    }

    pub(super) fn window_id(&self) -> u64 {
        self.window_id
    }

    pub(super) fn set_window_id(&mut self, window_id: u64) {
        self.window_id = window_id;
    }

    pub(super) fn advance_window_id(&mut self) -> u64 {
        self.window_id = self.window_id.saturating_add(1);
        self.new_context_window_requested = false;
        self.window_id
    }

    pub(super) fn request_new_context_window(&mut self) {
        self.new_context_window_requested = true;
    }

    pub(super) fn take_new_context_window_request(&mut self) -> bool {
        let requested = self.new_context_window_requested;
        self.new_context_window_requested = false;
        requested
    }

    /// Records the request-input side of the first server usage sample. The
    /// sampled output from that response is body growth and should remain
    /// counted against the scoped auto-compact budget.
    pub(super) fn ensure_server_observed_prefill_from_usage(&mut self, usage: &TokenUsage) {
        if matches!(
            self.prefill_input_tokens,
            Some(AutoCompactWindowPrefill::ServerObserved(_))
        ) {
            return;
        }

        self.prefill_input_tokens = Some(AutoCompactWindowPrefill::ServerObserved(
            usage.input_tokens.max(0),
        ));
    }

    pub(super) fn set_estimated_prefill(&mut self, tokens: i64) {
        if matches!(
            self.prefill_input_tokens,
            Some(AutoCompactWindowPrefill::ServerObserved(_))
        ) {
            return;
        }

        self.prefill_input_tokens = Some(AutoCompactWindowPrefill::Estimated(tokens.max(0)));
    }

    pub(super) fn snapshot(&self) -> AutoCompactWindowSnapshot {
        let prefill_input_tokens = match self.prefill_input_tokens {
            Some(AutoCompactWindowPrefill::ServerObserved(tokens))
            | Some(AutoCompactWindowPrefill::Estimated(tokens)) => Some(tokens),
            None => None,
        };
        AutoCompactWindowSnapshot {
            prefill_input_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn tracks_prefill_and_window_boundaries() {
        let mut window = AutoCompactWindow::new();

        assert_eq!(window.window_id(), 0);
        window.set_window_id(/*window_id*/ 3);
        assert_eq!(window.window_id(), 3);
        window.request_new_context_window();
        assert!(window.take_new_context_window_request());
        assert!(!window.take_new_context_window_request());
        window.request_new_context_window();
        assert_eq!(window.advance_window_id(), 4);
        assert_eq!(window.window_id(), 4);
        assert!(!window.take_new_context_window_request());

        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                prefill_input_tokens: None,
            }
        );

        window.set_estimated_prefill(/*tokens*/ 150);
        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                prefill_input_tokens: Some(150),
            }
        );

        window.ensure_server_observed_prefill_from_usage(&TokenUsage {
            input_tokens: 120,
            total_tokens: 170,
            ..Default::default()
        });
        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                prefill_input_tokens: Some(120),
            }
        );

        window.ensure_server_observed_prefill_from_usage(&TokenUsage {
            input_tokens: 130,
            total_tokens: 180,
            ..Default::default()
        });
        window.set_estimated_prefill(/*tokens*/ 90);
        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                prefill_input_tokens: Some(120),
            }
        );
    }
}
