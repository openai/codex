//! Interactive tool request surfaces for `ChatWidget`.
//!
//! This module owns approval, permission, elicitation, and user-input prompts
//! that block on user decisions.

use super::*;

impl ChatWidget {
    pub(super) fn on_exec_approval_request(&mut self, _id: String, ev: ExecApprovalRequestEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_exec_approval(ev),
            |s| s.handle_exec_approval_now(ev2),
        );
    }

    pub(super) fn on_apply_patch_approval_request(
        &mut self,
        _id: String,
        ev: ApplyPatchApprovalRequestEvent,
    ) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_apply_patch_approval(ev),
            |s| s.handle_apply_patch_approval_now(ev2),
        );
    }

    /// Handle guardian review lifecycle events for the current thread.
    ///
    /// In-progress assessments temporarily own the live status footer so the
    /// user can see what is being reviewed, including parallel review
    /// aggregation. Terminal assessments clear or update that footer state and
    /// render the final approved/denied history cell when guardian returns a
    /// decision.
    pub(super) fn on_guardian_assessment(&mut self, ev: GuardianAssessmentEvent) {
        let permission_request_summary = |subject: &str, reason: &Option<String>| {
            reason
                .as_deref()
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .map(|reason| format!("{subject}: {reason}"))
                .unwrap_or_else(|| subject.to_string())
        };
        let guardian_action_summary = |action: &GuardianAssessmentAction| match action {
            GuardianAssessmentAction::Command { command, .. } => Some(command.clone()),
            GuardianAssessmentAction::Execve { program, argv, .. } => {
                let command = if argv.is_empty() {
                    vec![program.clone()]
                } else {
                    argv.clone()
                };
                shlex::try_join(command.iter().map(String::as_str))
                    .ok()
                    .or_else(|| Some(command.join(" ")))
            }
            GuardianAssessmentAction::ApplyPatch { files, .. } => Some(if files.len() == 1 {
                format!("apply_patch touching {}", files[0].display())
            } else {
                format!("apply_patch touching {} files", files.len())
            }),
            GuardianAssessmentAction::NetworkAccess { target, .. } => {
                Some(format!("network access to {target}"))
            }
            GuardianAssessmentAction::McpToolCall {
                server,
                tool_name,
                connector_name,
                ..
            } => {
                let label = connector_name.as_deref().unwrap_or(server.as_str());
                Some(format!("MCP {tool_name} on {label}"))
            }
            GuardianAssessmentAction::RequestPermissions { reason, .. } => {
                Some(permission_request_summary("permission request", reason))
            }
        };
        let guardian_command = |action: &GuardianAssessmentAction| match action {
            GuardianAssessmentAction::Command { command, .. } => shlex::split(command)
                .filter(|command| !command.is_empty())
                .or_else(|| Some(vec![command.clone()])),
            GuardianAssessmentAction::Execve { program, argv, .. } => Some(if argv.is_empty() {
                vec![program.clone()]
            } else {
                argv.clone()
            })
            .filter(|command| !command.is_empty()),
            GuardianAssessmentAction::ApplyPatch { .. }
            | GuardianAssessmentAction::NetworkAccess { .. }
            | GuardianAssessmentAction::McpToolCall { .. }
            | GuardianAssessmentAction::RequestPermissions { .. } => None,
        };

        if ev.status == GuardianAssessmentStatus::InProgress
            && let Some(detail) = guardian_action_summary(&ev.action)
        {
            // In-progress assessments own the live footer state while the
            // review is pending. Parallel reviews are aggregated into one
            // footer summary by `PendingGuardianReviewStatus`.
            self.bottom_pane.ensure_status_indicator();
            self.bottom_pane
                .set_interrupt_hint_visible(/*visible*/ true);
            self.status_state
                .pending_guardian_review_status
                .start_or_update(ev.id.clone(), detail);
            if let Some(status) = self
                .status_state
                .pending_guardian_review_status
                .status_indicator_state()
            {
                self.set_status(
                    status.header,
                    status.details,
                    StatusDetailsCapitalization::Preserve,
                    status.details_max_lines,
                );
            }
            self.request_redraw();
            return;
        }

        // Terminal assessments remove the matching pending footer entry first,
        // then render the final approved/denied history cell below.
        if self
            .status_state
            .pending_guardian_review_status
            .finish(&ev.id)
        {
            if let Some(status) = self
                .status_state
                .pending_guardian_review_status
                .status_indicator_state()
            {
                self.set_status(
                    status.header,
                    status.details,
                    StatusDetailsCapitalization::Preserve,
                    status.details_max_lines,
                );
            } else if self.status_state.current_status.is_guardian_review() {
                self.set_status_header(String::from("Working"));
            }
        } else if self.status_state.pending_guardian_review_status.is_empty()
            && self.status_state.current_status.is_guardian_review()
        {
            self.set_status_header(String::from("Working"));
        }

        if ev.status == GuardianAssessmentStatus::Approved {
            let cell = if let Some(command) = guardian_command(&ev.action) {
                history_cell::new_approval_decision_cell(
                    history_cell::ApprovalDecisionSubject::Command(command),
                    crate::history_cell::ReviewDecision::Approved,
                    history_cell::ApprovalDecisionActor::Guardian,
                )
            } else if let Some(summary) = guardian_action_summary(&ev.action) {
                history_cell::new_guardian_approved_action_request(summary)
            } else {
                let summary = serde_json::to_string(&ev.action)
                    .unwrap_or_else(|_| "<unrenderable guardian action>".to_string());
                history_cell::new_guardian_approved_action_request(summary)
            };

            self.add_boxed_history(cell);
            self.request_redraw();
            return;
        }

        if ev.status == GuardianAssessmentStatus::TimedOut {
            let cell = if let Some(command) = guardian_command(&ev.action) {
                history_cell::new_approval_decision_cell(
                    history_cell::ApprovalDecisionSubject::Command(command),
                    crate::history_cell::ReviewDecision::TimedOut,
                    history_cell::ApprovalDecisionActor::Guardian,
                )
            } else {
                match &ev.action {
                    GuardianAssessmentAction::ApplyPatch { files, .. } => {
                        let files = files
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>();
                        history_cell::new_guardian_timed_out_patch_request(files)
                    }
                    GuardianAssessmentAction::McpToolCall {
                        server, tool_name, ..
                    } => history_cell::new_guardian_timed_out_action_request(format!(
                        "codex could call MCP tool {server}.{tool_name}"
                    )),
                    GuardianAssessmentAction::NetworkAccess { target, .. } => {
                        history_cell::new_guardian_timed_out_action_request(format!(
                            "codex could access {target}"
                        ))
                    }
                    GuardianAssessmentAction::RequestPermissions { reason, .. } => {
                        history_cell::new_guardian_timed_out_action_request(
                            permission_request_summary("codex could request permissions", reason),
                        )
                    }
                    GuardianAssessmentAction::Command { .. } => unreachable!(),
                    GuardianAssessmentAction::Execve { .. } => unreachable!(),
                }
            };

            self.add_boxed_history(cell);
            self.request_redraw();
            return;
        }

        if ev.status != GuardianAssessmentStatus::Denied {
            return;
        }
        self.review.recent_auto_review_denials.push(ev.clone());
        let cell = if let Some(command) = guardian_command(&ev.action) {
            history_cell::new_approval_decision_cell(
                history_cell::ApprovalDecisionSubject::Command(command),
                crate::history_cell::ReviewDecision::Denied,
                history_cell::ApprovalDecisionActor::Guardian,
            )
        } else {
            match &ev.action {
                GuardianAssessmentAction::ApplyPatch { files, .. } => {
                    let files = files
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>();
                    history_cell::new_guardian_denied_patch_request(files)
                }
                GuardianAssessmentAction::McpToolCall {
                    server, tool_name, ..
                } => history_cell::new_guardian_denied_action_request(format!(
                    "codex to call MCP tool {server}.{tool_name}"
                )),
                GuardianAssessmentAction::NetworkAccess { target, .. } => {
                    history_cell::new_guardian_denied_action_request(format!(
                        "codex to access {target}"
                    ))
                }
                GuardianAssessmentAction::RequestPermissions { reason, .. } => {
                    history_cell::new_guardian_denied_action_request(permission_request_summary(
                        "codex to request permissions",
                        reason,
                    ))
                }
                GuardianAssessmentAction::Command { .. } => unreachable!(),
                GuardianAssessmentAction::Execve { .. } => unreachable!(),
            }
        };

        self.add_boxed_history(cell);
        self.request_redraw();
    }

    pub(super) fn on_elicitation_request(
        &mut self,
        request_id: AppServerRequestId,
        params: McpServerElicitationRequestParams,
    ) {
        let request_id2 = request_id.clone();
        let params2 = params.clone();
        self.defer_or_handle(
            |q| q.push_elicitation(request_id, params),
            |s| s.handle_elicitation_request_now(request_id2, params2),
        );
    }

    pub(super) fn on_request_user_input(&mut self, ev: ToolRequestUserInputParams) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_user_input(ev),
            |s| s.handle_request_user_input_now(ev2),
        );
    }

    pub(super) fn on_request_permissions(&mut self, ev: RequestPermissionsEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_request_permissions(ev),
            |s| s.handle_request_permissions_now(ev2),
        );
    }

    pub(crate) fn handle_exec_approval_now(&mut self, ev: ExecApprovalRequestEvent) {
        self.flush_answer_stream_with_separator();
        let command = shlex::try_join(ev.command.iter().map(String::as_str))
            .unwrap_or_else(|_| ev.command.join(" "));
        self.notify(Notification::ExecApprovalRequested { command });

        let available_decisions = ev.effective_available_decisions();
        let request = ApprovalRequest::Exec {
            thread_id: self.thread_id.unwrap_or_default(),
            thread_label: None,
            id: ev.effective_approval_id(),
            command: ev.command,
            reason: ev.reason,
            available_decisions,
            network_approval_context: ev.network_approval_context,
            additional_permissions: ev.additional_permissions,
        };
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
    }

    pub(crate) fn handle_apply_patch_approval_now(&mut self, ev: ApplyPatchApprovalRequestEvent) {
        self.flush_answer_stream_with_separator();

        let request = ApprovalRequest::ApplyPatch {
            thread_id: self.thread_id.unwrap_or_default(),
            thread_label: None,
            id: ev.call_id,
            reason: ev.reason,
            changes: ev.changes.clone(),
            cwd: self.config.cwd.clone(),
        };
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
        self.notify(Notification::EditApprovalRequested {
            cwd: self.config.cwd.to_path_buf(),
            changes: ev.changes.keys().cloned().collect(),
        });
    }

    pub(crate) fn handle_elicitation_request_now(
        &mut self,
        request_id: AppServerRequestId,
        params: McpServerElicitationRequestParams,
    ) {
        self.flush_answer_stream_with_separator();

        self.notify(Notification::ElicitationRequested {
            server_name: params.server_name.clone(),
        });

        let thread_id = self.thread_id.unwrap_or_default();
        if let Some(params) = crate::bottom_pane::AppLinkViewParams::from_url_app_server_request(
            thread_id,
            &params.server_name,
            request_id.clone(),
            &params.request,
        ) {
            self.open_app_link_view(params);
        } else if let Some(request) = McpServerElicitationFormRequest::from_app_server_request(
            thread_id,
            request_id.clone(),
            params.clone(),
        ) {
            self.bottom_pane
                .push_mcp_server_elicitation_request(request);
        } else {
            match params.request {
                McpServerElicitationRequest::Form { message, .. } => {
                    let request = ApprovalRequest::McpElicitation {
                        thread_id,
                        thread_label: None,
                        server_name: params.server_name,
                        request_id,
                        message,
                    };
                    self.bottom_pane
                        .push_approval_request(request, &self.config.features);
                }
                McpServerElicitationRequest::Url { .. } => {
                    self.app_event_tx.resolve_elicitation(
                        thread_id,
                        params.server_name,
                        request_id,
                        codex_app_server_protocol::McpServerElicitationAction::Decline,
                        /*content*/ None,
                        /*meta*/ None,
                    );
                }
            }
        }
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
    }

    pub(crate) fn push_approval_request(&mut self, request: ApprovalRequest) {
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
    }

    pub(crate) fn push_mcp_server_elicitation_request(
        &mut self,
        request: McpServerElicitationFormRequest,
    ) {
        self.bottom_pane
            .push_mcp_server_elicitation_request(request);
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
    }

    pub(crate) fn handle_request_user_input_now(&mut self, ev: ToolRequestUserInputParams) {
        self.flush_answer_stream_with_separator();
        let question_count = ev.questions.len();
        let summary = Notification::user_input_request_summary(&ev.questions);
        let title = match (question_count, summary.as_deref()) {
            (1, Some(summary)) => summary.to_string(),
            (1, None) => "Question requested".to_string(),
            (count, _) => format!("{count} questions requested"),
        };
        self.notify(Notification::PlanModePrompt { title });
        self.bottom_pane.push_user_input_request(ev);
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
    }

    pub(crate) fn handle_request_permissions_now(&mut self, ev: RequestPermissionsEvent) {
        self.flush_answer_stream_with_separator();
        let request = ApprovalRequest::Permissions {
            thread_id: self.thread_id.unwrap_or_default(),
            thread_label: None,
            call_id: ev.call_id,
            reason: ev.reason,
            permissions: ev.permissions,
        };
        self.bottom_pane
            .push_approval_request(request, &self.config.features);
        self.set_ambient_pet_notification(
            crate::pets::PetNotificationKind::Waiting,
            /*body*/ None,
        );
        self.request_redraw();
    }
}
