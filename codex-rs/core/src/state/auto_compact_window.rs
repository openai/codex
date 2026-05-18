use codex_protocol::protocol::TokenUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoCompactWindowSnapshot {
    pub(crate) ordinal: u64,
    pub(crate) prefill_input_tokens: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoCompactWindowPrefill {
    ServerObserved(i64),
    Estimated(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AutoCompactWindow {
    ordinal: u64,
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
            ordinal: 1,
            prefill_input_tokens: None,
        }
    }

    pub(super) fn clear_prefill(&mut self) {
        self.prefill_input_tokens = None;
    }

    pub(super) fn start_next(&mut self) {
        self.ordinal = self.ordinal.saturating_add(1);
        self.clear_prefill();
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
            ordinal: self.ordinal,
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

        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                ordinal: 1,
                prefill_input_tokens: None,
            }
        );

        window.set_estimated_prefill(/*tokens*/ 150);
        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                ordinal: 1,
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
                ordinal: 1,
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
                ordinal: 1,
                prefill_input_tokens: Some(120),
            }
        );

        window.start_next();
        assert_eq!(
            window.snapshot(),
            AutoCompactWindowSnapshot {
                ordinal: 2,
                prefill_input_tokens: None,
            }
        );
    }
}
