use codex_protocol::protocol::TokenUsage;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoCompactWindowIds {
    pub(crate) first_window_id: Uuid,
    pub(crate) previous_window_id: Option<Uuid>,
    pub(crate) window_id: Uuid,
}

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
    window_number: u64,
    ids: AutoCompactWindowIds,
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
        let window_id = Uuid::now_v7();
        Self {
            window_number: 0,
            ids: AutoCompactWindowIds {
                first_window_id: window_id,
                previous_window_id: None,
                window_id,
            },
            new_context_window_requested: false,
            prefill_input_tokens: None,
        }
    }

    pub(super) fn clear_prefill(&mut self) {
        self.prefill_input_tokens = None;
    }

    pub(super) fn window_number(&self) -> u64 {
        self.window_number
    }

    pub(super) fn ids(&self) -> AutoCompactWindowIds {
        self.ids
    }

    pub(super) fn restore(&mut self, window_number: u64, ids: AutoCompactWindowIds) {
        self.window_number = window_number;
        self.ids = ids;
    }

    pub(super) fn advance(&mut self) -> (u64, AutoCompactWindowIds) {
        self.window_number = self.window_number.saturating_add(1);
        self.ids.previous_window_id = Some(self.ids.window_id);
        self.ids.window_id = Uuid::now_v7();
        self.new_context_window_requested = false;
        (self.window_number, self.ids)
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

        assert_eq!(window.window_number(), 0);
        let initial_window_id = window.ids().window_id;
        assert_eq!(initial_window_id.get_version_num(), 7);
        assert_eq!(
            window.ids(),
            AutoCompactWindowIds {
                first_window_id: initial_window_id,
                previous_window_id: None,
                window_id: initial_window_id,
            }
        );
        let first_window_id = initial_window_id;
        let restored_window_id = Uuid::now_v7();
        let restored_previous_window_id = Uuid::now_v7();
        window.restore(
            /*window_number*/ 3,
            AutoCompactWindowIds {
                first_window_id,
                previous_window_id: Some(restored_previous_window_id),
                window_id: restored_window_id,
            },
        );
        assert_eq!(window.window_number(), 3);
        assert_eq!(window.ids().window_id, restored_window_id);
        window.request_new_context_window();
        assert!(window.take_new_context_window_request());
        assert!(!window.take_new_context_window_request());
        window.request_new_context_window();
        let (window_number, ids) = window.advance();
        assert_eq!(window_number, 4);
        assert_eq!(window.window_number(), 4);
        assert_eq!(window.ids(), ids);
        assert_eq!(ids.first_window_id, first_window_id);
        assert_eq!(ids.previous_window_id, Some(restored_window_id));
        assert_eq!(ids.window_id.get_version_num(), 7);
        assert_ne!(ids.window_id, restored_window_id);
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
