//! Block ordinary input after a misalignment failure. Live findings can be reviewed and
//! explicitly acknowledged; review identity prevents stale UI actions from authorizing a turn.

use super::*;
use codex_app_server_protocol::MisalignmentErrorDetails;

const PRECAUTION_VIEW: &str = "misalignment_precaution";

#[derive(Debug)]
pub(crate) struct MisalignmentReview {
    pub(crate) thread_id: ThreadId,
    pub(crate) turn_id: String,
    pub(crate) details: MisalignmentErrorDetails,
}

impl MisalignmentReview {
    pub(crate) fn continuation_message(&self) -> Option<&str> {
        self.details
            .steer
            .as_ref()
            .map(|steer| steer.message.as_str())
            .filter(|message| !message.trim().is_empty() && message.len() <= 1024)
    }
}

pub(super) struct MisalignmentViolation {
    turn_id: Option<String>,
    review: Option<Arc<MisalignmentReview>>,
    retired_voice: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum MisalignmentTurnSource {
    ServerNotification,
    AcknowledgedContinuation,
}

const MISALIGNMENT_POLICY_TITLE: &str = "Chat stopped as a precaution";
const MISALIGNMENT_POLICY_DESCRIPTION: &str = "We couldn’t confirm the agent was acting safely and following your instructions. To continue working, start or resume another chat.";

impl ChatWidget {
    pub(crate) fn has_misalignment_policy_violation(&self) -> bool {
        self.misalignment_policy_violation.is_some()
    }

    pub(crate) fn rejects_misalignment_policy_op(&self, op: &AppCommand) -> bool {
        self.misalignment_policy_violation.is_some()
            && !matches!(
                op,
                AppCommand::Interrupt
                    | AppCommand::RealtimeConversationStop { .. }
                    | AppCommand::CleanBackgroundTerminals
                    | AppCommand::OverrideTurnContext { .. }
                    | AppCommand::ReloadUserConfig
                    | AppCommand::ListSkills { .. }
                    | AppCommand::SetThreadName { .. }
            )
    }

    pub(crate) fn on_misalignment_policy_violation(&mut self) {
        self.on_misalignment_error(/*turn_id*/ None, /*details*/ None);
    }

    pub(super) fn on_misalignment_error(
        &mut self,
        turn_id: Option<String>,
        details: Option<MisalignmentErrorDetails>,
    ) {
        if let (Some(turn_id), Some(latest)) = (&turn_id, &self.turn_lifecycle.last_turn_id)
            && turn_id != latest
        {
            return;
        }
        // Bound memory-only findings and newly submitted model input. Never truncate a steer.
        let missing_details = details.is_none();
        let details = details.filter(|details| {
            details
                .detailed_explanation
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty() && text.len() <= 64 * 1024)
        });
        if let Some(current) = &self.misalignment_policy_violation
            // A parent-thread precaution cannot become a continuable side-thread warning.
            && (current.turn_id.is_none()
                || (current.turn_id == turn_id
                    && (missing_details
                        || current.review.as_ref().map(|review| &review.details) == details.as_ref())))
        {
            return;
        }
        let retired_voice = self.realtime_conversation_is_running();
        if retired_voice {
            self.stop_realtime_conversation();
        }
        let review = self.thread_id.zip(turn_id.clone()).zip(details).map(
            |((thread_id, turn_id), details)| {
                Arc::new(MisalignmentReview {
                    thread_id,
                    turn_id,
                    details,
                })
            },
        );
        self.misalignment_policy_violation = Some(MisalignmentViolation {
            turn_id,
            review,
            retired_voice,
        });
        self.input_queue.clear();
        self.finalize_turn();
        self.refresh_pending_input_preview();
        self.bottom_pane.drain_pending_submission_state();
        self.bottom_pane
            .set_composer_text(String::new(), Vec::new(), Vec::new());
        self.bottom_pane.set_composer_input_enabled(
            /*enabled*/ false,
            Some(MISALIGNMENT_POLICY_TITLE.to_string()),
        );

        self.show_misalignment_policy_precaution();
    }

    pub(crate) fn show_misalignment_policy_precaution(&mut self) {
        if !self.has_misalignment_policy_violation() {
            return;
        }
        self.bottom_pane.dismiss_view_by_id(PRECAUTION_VIEW);
        let mut items = vec![
            SelectionItem {
                name: "New chat".to_string(),
                actions: vec![Box::new(|tx| tx.send(AppEvent::NewSession { name: None }))],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Resume another chat".to_string(),
                actions: vec![Box::new(|tx| tx.send(AppEvent::OpenResumePicker))],
                ..Default::default()
            },
        ];
        let review = self
            .misalignment_policy_violation
            .as_ref()
            .and_then(|violation| violation.review.clone());
        if let Some(review) = &review {
            let review = Arc::clone(review);
            items.insert(
                0,
                SelectionItem {
                    name: "Review findings".to_string(),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::ReviewMisalignment(Arc::clone(&review)));
                    })],
                    ..Default::default()
                },
            );
        }
        if self.remote_connection.is_some() {
            items.insert(
                1,
                SelectionItem {
                    name: "Agent command center".to_string(),
                    actions: vec![Box::new(|tx| tx.send(AppEvent::OpenAgentsOverview))],
                    ..Default::default()
                },
            );
        }
        self.bottom_pane.show_selection_view(SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            view_id: Some(PRECAUTION_VIEW),
            header: Box::new(
                Paragraph::new(vec![
                    Line::from(if review.is_some() { "Chat paused as a precaution" } else { MISALIGNMENT_POLICY_TITLE }).bold(),
                    Line::from(if review.is_some() {
                        "We couldn’t confirm the agent was interpreting your instructions correctly. Review what we detected before deciding to continue."
                    } else { MISALIGNMENT_POLICY_DESCRIPTION }).dim(),
                ])
                .wrap(Wrap { trim: false }),
            ),
            items,
            allow_cancel: false,
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn is_current_misalignment_review(&self, review: &Arc<MisalignmentReview>) -> bool {
        let current = self
            .misalignment_policy_violation
            .as_ref()
            .and_then(|violation| violation.review.as_ref());
        let turn_id = self.turn_lifecycle.last_turn_id.as_ref();
        self.thread_id == Some(review.thread_id)
            && turn_id.is_none_or(|id| id == &review.turn_id)
            && current.is_some_and(|current| Arc::ptr_eq(current, review))
    }

    pub(crate) fn show_misalignment_review_confirmation(
        &mut self,
        review: Arc<MisalignmentReview>,
    ) {
        if !self.is_current_misalignment_review(&review) {
            return;
        }
        let can_continue = review.continuation_message().is_some();
        self.bottom_pane.dismiss_view_by_id(PRECAUTION_VIEW);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            view_id: Some(PRECAUTION_VIEW),
            title: Some("Chat paused as a precaution".to_string()),
            items: vec![
                SelectionItem {
                    name: "Acknowledge findings and continue".to_string(),
                    is_disabled: !can_continue,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::ContinueMisalignment(Arc::clone(&review)));
                    })],
                    require_explicit_confirmation: true,
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Back".to_string(),
                    actions: vec![Box::new(|tx| tx.send(AppEvent::CloseMisalignmentReview))],
                    ..Default::default()
                },
            ],
            allow_cancel: false,
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn clear_misalignment_for_new_turn(
        &mut self,
        turn_id: &str,
        source: MisalignmentTurnSource,
    ) {
        if self
            .misalignment_policy_violation
            .as_ref()
            .is_some_and(|violation| {
                violation.turn_id.as_ref().is_some_and(|id| id != turn_id)
                    && (!violation.retired_voice
                        || source == MisalignmentTurnSource::AcknowledgedContinuation)
            })
        {
            self.misalignment_policy_violation = None;
            self.bottom_pane.dismiss_view_by_id(PRECAUTION_VIEW);
            self.bottom_pane
                .set_composer_input_enabled(/*enabled*/ true, /*placeholder*/ None);
        }
    }
}
