//! Continue only the displayed, acknowledged misalignment failure. Submission stays outside
//! the ordinary input queue, which must remain blocked until the server accepts the new turn.

use super::*;
use crate::app_server_session::turn_permissions_overrides;
use crate::chatwidget::MisalignmentReview;
use crate::chatwidget::MisalignmentTurnSource;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;

impl App {
    pub(super) fn open_misalignment_review(
        &mut self,
        tui: &mut tui::Tui,
        review: Arc<MisalignmentReview>,
    ) {
        if !self.chat_widget.is_current_misalignment_review(&review) {
            return;
        }
        self.chat_widget
            .show_misalignment_review_confirmation(Arc::clone(&review));
        let mut lines = Vec::new();
        if let Some(message) = review.continuation_message() {
            lines.push(Line::from("Continuation request (quoted)").bold());
            // Keep the submitted text ahead of potentially long findings, without interpreting it.
            lines.push(Line::from(format!("{message:?}")));
            lines.push(Line::default());
        }
        crate::markdown::append_markdown(
            review
                .details
                .detailed_explanation
                .as_deref()
                .unwrap_or_default(),
            /*width*/ None,
            Some(self.chat_widget.config_ref().cwd.as_path()),
            &mut lines,
        );
        let _ = tui.enter_alt_screen();
        self.overlay = Some(Overlay::new_static_with_lines(
            lines,
            "What we detected".to_string(),
            self.keymap.pager.clone(),
        ));
        tui.frame_requester().schedule_frame();
    }

    pub(super) async fn continue_misalignment(
        &mut self,
        app_server: &mut AppServerSession,
        review: Arc<MisalignmentReview>,
    ) {
        if self.chat_widget.is_current_misalignment_review(&review) {
            self.chat_widget.show_misalignment_policy_precaution();
        }
        // The event store can be newer than the widget while UI actions are queued.
        if let Some(channel) = self.thread_event_channels.get(&review.thread_id) {
            let store = channel.store.lock().await;
            if store.active_turn_id().is_some() {
                return;
            }
            let latest = store.buffer.iter().rev().find_map(|event| {
                let ThreadBufferedEvent::Notification(notification) = event else {
                    return None;
                };
                let (turn_id, error) = match notification.as_ref() {
                    ServerNotification::TurnStarted(_) => return Some(false),
                    ServerNotification::TurnCompleted(n) if n.turn.status == TurnStatus::Failed => {
                        (&n.turn.id, n.turn.error.as_ref())
                    }
                    ServerNotification::TurnCompleted(_) => return Some(false),
                    ServerNotification::Error(n)
                        if !n.will_retry
                            && (n.turn_id == review.turn_id
                                || n.error.codex_error_info
                                    == Some(
                                        AppServerCodexErrorInfo::MisalignmentPolicyViolation,
                                    )) =>
                    {
                        (&n.turn_id, Some(&n.error))
                    }
                    _ => return None,
                };
                let Some(error) = error else {
                    return Some(false);
                };
                if turn_id != &review.turn_id
                    || error.codex_error_info
                        != Some(AppServerCodexErrorInfo::MisalignmentPolicyViolation)
                {
                    return Some(false);
                }
                // Empty duplicates do not supersede the latest supplied findings.
                error
                    .misalignment
                    .as_ref()
                    .map(|details| details == &review.details)
            });
            if latest == Some(false) {
                return;
            }
        }
        if self.current_displayed_thread_id() != Some(review.thread_id)
            || self
                .active_thread_rx
                .as_ref()
                .is_some_and(|rx| !rx.is_empty())
            || !self.chat_widget.is_current_misalignment_review(&review)
        {
            return;
        }
        let Some(message) = review.continuation_message() else {
            return;
        };
        let config = self.chat_widget.config_ref();
        let explicit_profile =
            self.runtime_permission_profile_override
                .as_ref()
                .filter(|profile| {
                    profile.turn_override == RuntimePermissionProfileTurnOverride::LegacySandbox
                });
        let permissions_override = Self::turn_permissions_override_from_config(
            config,
            explicit_profile.and_then(|profile| profile.active_permission_profile.as_ref()),
            explicit_profile.and_then(RuntimePermissionProfileOverride::turn_permission_profile),
        );
        let Ok((sandbox_policy, permissions)) =
            turn_permissions_overrides(permissions_override, config.cwd.as_path())
        else {
            self.chat_widget.add_error_message(
                "Couldn’t continue this chat. Review its latest status before trying again."
                    .to_string(),
            );
            return;
        };
        let result = app_server
            .request_handle()
            .request_typed::<TurnStartResponse>(ClientRequest::TurnStart {
                request_id: app_server.next_request_id(),
                params: TurnStartParams {
                    thread_id: review.thread_id.to_string(),
                    cwd: Some(config.cwd.to_path_buf()),
                    runtime_workspace_roots: Some(
                        config.permissions.user_visible_workspace_roots().to_vec(),
                    ),
                    approval_policy: Some(config.permissions.approval_policy.value().into()),
                    approvals_reviewer: Some(config.approvals_reviewer.into()),
                    sandbox_policy,
                    permissions,
                    input: vec![UserInput::Text {
                        text: message.to_string(),
                        text_elements: Vec::new(),
                    }],
                    responsesapi_client_metadata: Some(HashMap::from([(
                        "misalignment_override".to_string(),
                        serde_json::json!({"timestamp": chrono::Utc::now().timestamp_millis()})
                            .to_string(),
                    )])),
                    ..Default::default()
                },
            })
            .await;
        match result {
            Ok(response) => {
                let store = &self.ensure_thread_channel(review.thread_id).store;
                store.lock().await.active_turn_id = Some(response.turn.id.clone());
                self.chat_widget.clear_misalignment_for_new_turn(
                    &response.turn.id,
                    MisalignmentTurnSource::AcknowledgedContinuation,
                );
                self.chat_widget.handle_server_notification(
                    ServerNotification::TurnStarted(
                        codex_app_server_protocol::TurnStartedNotification {
                            thread_id: review.thread_id.to_string(),
                            turn: response.turn,
                        },
                    ),
                    /*replay_kind*/ None,
                );
            }
            Err(_) => {
                // An RPC diagnostic can contain the submitted steer. Keep the review error generic.
                self.chat_widget.add_error_message(
                    "Couldn’t continue this chat. Review its latest status before trying again."
                        .to_string(),
                );
            }
        }
    }
}
