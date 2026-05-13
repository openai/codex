use super::*;
use codex_protocol::openai_models::ReasoningEffort;

#[derive(Clone)]
pub(crate) struct TurnRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    analytics_events_client: AnalyticsEventsClient,
    arg0_paths: Arg0DispatchPaths,
    config: Arc<Config>,
    config_manager: ConfigManager,
    pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
    thread_state_manager: ThreadStateManager,
    thread_watch_manager: ThreadWatchManager,
    thread_list_state_permit: Arc<Semaphore>,
    skills_watcher: Arc<SkillsWatcher>,
}

struct TurnContextOverrideRequest {
    cwd: Option<PathBuf>,
    approval_policy: Option<AskForApproval>,
    approvals_reviewer: Option<codex_app_server_protocol::ApprovalsReviewer>,
    sandbox_policy: Option<codex_app_server_protocol::SandboxPolicy>,
    permissions: Option<PermissionProfileSelectionParams>,
    model: Option<String>,
    service_tier: Option<Option<String>>,
    effort: Option<Option<ReasoningEffort>>,
    summary: Option<codex_protocol::config_types::ReasoningSummary>,
    collaboration_mode: Option<CollaborationMode>,
    personality: Option<Personality>,
}

impl TurnContextOverrideRequest {
    fn has_any_overrides(&self) -> bool {
        self.cwd.is_some()
            || self.approval_policy.is_some()
            || self.approvals_reviewer.is_some()
            || self.sandbox_policy.is_some()
            || self.permissions.is_some()
            || self.model.is_some()
            || self.service_tier.is_some()
            || self.effort.is_some()
            || self.summary.is_some()
            || self.collaboration_mode.is_some()
            || self.personality.is_some()
    }
}

struct ResolvedTurnContextOverrides {
    has_any_overrides: bool,
    overrides: CodexThreadTurnContextOverrides,
}

impl TurnRequestProcessor {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        analytics_events_client: AnalyticsEventsClient,
        arg0_paths: Arg0DispatchPaths,
        config: Arc<Config>,
        config_manager: ConfigManager,
        pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
        thread_state_manager: ThreadStateManager,
        thread_watch_manager: ThreadWatchManager,
        thread_list_state_permit: Arc<Semaphore>,
        skills_watcher: Arc<SkillsWatcher>,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            outgoing,
            analytics_events_client,
            arg0_paths,
            config,
            config_manager,
            pending_thread_unloads,
            thread_state_manager,
            thread_watch_manager,
            thread_list_state_permit,
            skills_watcher,
        }
    }

    pub(crate) async fn turn_start(
        &self,
        request_id: ConnectionRequestId,
        params: TurnStartParams,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.turn_start_inner(
            request_id,
            params,
            app_server_client_name,
            app_server_client_version,
        )
        .await
        .map(|response| Some(response.into()))
    }

    pub(crate) async fn thread_inject_items(
        &self,
        params: ThreadInjectItemsParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_inject_items_response_inner(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn thread_turn_context_update(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadTurnContextUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_turn_context_update_inner(request_id, params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn turn_steer(
        &self,
        request_id: &ConnectionRequestId,
        params: TurnSteerParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.turn_steer_inner(request_id, params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn turn_interrupt(
        &self,
        request_id: &ConnectionRequestId,
        params: TurnInterruptParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.turn_interrupt_inner(request_id, params)
            .await
            .map(|response| response.map(Into::into))
    }

    pub(crate) async fn thread_realtime_start(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_realtime_start_inner(request_id, params)
            .await
            .map(|response| response.map(Into::into))
    }

    pub(crate) async fn thread_realtime_append_audio(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeAppendAudioParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_realtime_append_audio_inner(request_id, params)
            .await
            .map(|response| response.map(Into::into))
    }

    pub(crate) async fn thread_realtime_append_text(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeAppendTextParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_realtime_append_text_inner(request_id, params)
            .await
            .map(|response| response.map(Into::into))
    }

    pub(crate) async fn thread_realtime_stop(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeStopParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_realtime_stop_inner(request_id, params)
            .await
            .map(|response| response.map(Into::into))
    }

    pub(crate) async fn thread_realtime_list_voices(
        &self,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        Ok(Some(
            ThreadRealtimeListVoicesResponse {
                voices: RealtimeVoicesList::builtin(),
            }
            .into(),
        ))
    }

    pub(crate) async fn review_start(
        &self,
        request_id: &ConnectionRequestId,
        params: ReviewStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.review_start_inner(request_id, params)
            .await
            .map(|()| None)
    }

    fn track_error_response(
        &self,
        request_id: &ConnectionRequestId,
        error: &JSONRPCErrorError,
        error_type: Option<AnalyticsJsonRpcError>,
    ) {
        self.analytics_events_client.track_error_response(
            request_id.connection_id.0,
            request_id.request_id.clone(),
            error.clone(),
            error_type,
        );
    }

    async fn load_thread(
        &self,
        thread_id: &str,
    ) -> Result<(ThreadId, Arc<CodexThread>), JSONRPCErrorError> {
        // Resolve the core conversation handle from a v2 thread id string.
        let thread_id = ThreadId::from_string(thread_id)
            .map_err(|err| invalid_request(format!("invalid thread id: {err}")))?;

        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| invalid_request(format!("thread not found: {thread_id}")))?;

        Ok((thread_id, thread))
    }
    fn normalize_turn_start_collaboration_mode(
        &self,
        mut collaboration_mode: CollaborationMode,
    ) -> CollaborationMode {
        if collaboration_mode.settings.developer_instructions.is_none()
            && let Some(instructions) = builtin_collaboration_mode_presets()
                .into_iter()
                .find(|preset| preset.mode == Some(collaboration_mode.mode))
                .and_then(|preset| preset.developer_instructions.flatten())
                .filter(|instructions| !instructions.is_empty())
        {
            collaboration_mode.settings.developer_instructions = Some(instructions);
        }

        collaboration_mode
    }

    fn review_request_from_target(
        target: ApiReviewTarget,
    ) -> Result<(ReviewRequest, String), JSONRPCErrorError> {
        let cleaned_target = match target {
            ApiReviewTarget::UncommittedChanges => ApiReviewTarget::UncommittedChanges,
            ApiReviewTarget::BaseBranch { branch } => {
                let branch = branch.trim().to_string();
                if branch.is_empty() {
                    return Err(invalid_request("branch must not be empty".to_string()));
                }
                ApiReviewTarget::BaseBranch { branch }
            }
            ApiReviewTarget::Commit { sha, title } => {
                let sha = sha.trim().to_string();
                if sha.is_empty() {
                    return Err(invalid_request("sha must not be empty".to_string()));
                }
                let title = title
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                ApiReviewTarget::Commit { sha, title }
            }
            ApiReviewTarget::Custom { instructions } => {
                let trimmed = instructions.trim().to_string();
                if trimmed.is_empty() {
                    return Err(invalid_request(
                        "instructions must not be empty".to_string(),
                    ));
                }
                ApiReviewTarget::Custom {
                    instructions: trimmed,
                }
            }
        };

        let core_target = match cleaned_target {
            ApiReviewTarget::UncommittedChanges => CoreReviewTarget::UncommittedChanges,
            ApiReviewTarget::BaseBranch { branch } => CoreReviewTarget::BaseBranch { branch },
            ApiReviewTarget::Commit { sha, title } => CoreReviewTarget::Commit { sha, title },
            ApiReviewTarget::Custom { instructions } => CoreReviewTarget::Custom { instructions },
        };

        let hint = codex_core::review_prompts::user_facing_hint(&core_target);
        let review_request = ReviewRequest {
            target: core_target,
            user_facing_hint: Some(hint.clone()),
        };

        Ok((review_request, hint))
    }

    fn parse_environment_selections(
        &self,
        environments: Option<Vec<TurnEnvironmentParams>>,
    ) -> Result<Option<Vec<TurnEnvironmentSelection>>, JSONRPCErrorError> {
        let environment_selections = environments.map(|environments| {
            environments
                .into_iter()
                .map(|environment| TurnEnvironmentSelection {
                    environment_id: environment.environment_id,
                    cwd: environment.cwd,
                })
                .collect::<Vec<_>>()
        });
        if let Some(environment_selections) = environment_selections.as_ref() {
            self.thread_manager
                .validate_environment_selections(environment_selections)
                .map_err(|err| invalid_request(environment_selection_error_message(err)))?;
        }
        Ok(environment_selections)
    }

    async fn request_trace_context(
        &self,
        request_id: &ConnectionRequestId,
    ) -> Option<codex_protocol::protocol::W3cTraceContext> {
        self.outgoing.request_trace_context(request_id).await
    }

    async fn submit_core_op(
        &self,
        request_id: &ConnectionRequestId,
        thread: &CodexThread,
        op: Op,
    ) -> CodexResult<String> {
        thread
            .submit_with_trace(op, self.request_trace_context(request_id).await)
            .await
    }

    fn input_too_large_error(actual_chars: usize) -> JSONRPCErrorError {
        let mut error = invalid_params(format!(
            "Input exceeds the maximum length of {MAX_USER_INPUT_TEXT_CHARS} characters."
        ));
        error.data = Some(serde_json::json!({
            "input_error_code": INPUT_TOO_LARGE_ERROR_CODE,
            "max_chars": MAX_USER_INPUT_TEXT_CHARS,
            "actual_chars": actual_chars,
        }));
        error
    }

    fn validate_v2_input_limit(items: &[V2UserInput]) -> Result<(), JSONRPCErrorError> {
        let actual_chars: usize = items.iter().map(V2UserInput::text_char_count).sum();
        if actual_chars > MAX_USER_INPUT_TEXT_CHARS {
            return Err(Self::input_too_large_error(actual_chars));
        }
        Ok(())
    }

    async fn resolve_turn_context_overrides(
        &self,
        base_snapshot: &ThreadConfigSnapshot,
        request: TurnContextOverrideRequest,
    ) -> Result<ResolvedTurnContextOverrides, JSONRPCErrorError> {
        if request.sandbox_policy.is_some() && request.permissions.is_some() {
            return Err(invalid_request(
                "`permissions` cannot be combined with `sandboxPolicy`",
            ));
        }

        let has_any_overrides = request.has_any_overrides();
        let cwd = request.cwd;
        let approval_policy = request.approval_policy.map(AskForApproval::to_core);
        let approvals_reviewer = request
            .approvals_reviewer
            .map(codex_app_server_protocol::ApprovalsReviewer::to_core);
        let sandbox_policy = request.sandbox_policy.map(|p| p.to_core());
        let (permission_profile, active_permission_profile) =
            if let Some(permissions) = request.permissions {
                let mut overrides = ConfigOverrides {
                    cwd: cwd.clone(),
                    codex_linux_sandbox_exe: self.arg0_paths.codex_linux_sandbox_exe.clone(),
                    main_execve_wrapper_exe: self.arg0_paths.main_execve_wrapper_exe.clone(),
                    ..Default::default()
                };
                apply_permission_profile_selection_to_config_overrides(
                    &mut overrides,
                    Some(permissions),
                );
                let config = self
                    .config_manager
                    .load_for_cwd(
                        /*request_overrides*/ None,
                        overrides,
                        Some(base_snapshot.cwd.to_path_buf()),
                    )
                    .await
                    .map_err(|err| config_load_error(&err))?;
                // Startup config is allowed to fall back when requirements
                // disallow a configured profile. An explicit turn context
                // update is different: reject it before accepting the request.
                if let Some(warning) = config.startup_warnings.iter().find(|warning| {
                    warning.contains("Configured value for `permission_profile` is disallowed")
                }) {
                    return Err(invalid_request(format!(
                        "invalid turn context override: {warning}"
                    )));
                }
                (
                    Some(config.permissions.permission_profile()),
                    config.permissions.active_permission_profile(),
                )
            } else {
                (None, None)
            };

        Ok(ResolvedTurnContextOverrides {
            has_any_overrides,
            overrides: CodexThreadTurnContextOverrides {
                cwd,
                approval_policy,
                approvals_reviewer,
                sandbox_policy,
                permission_profile,
                active_permission_profile,
                windows_sandbox_level: None,
                model: request.model,
                effort: request.effort,
                summary: request.summary,
                service_tier: request.service_tier,
                collaboration_mode: request
                    .collaboration_mode
                    .map(|mode| self.normalize_turn_start_collaboration_mode(mode)),
                personality: request.personality,
            },
        })
    }

    async fn maybe_emit_turn_context_updated(
        &self,
        thread_id: &str,
        before: &ThreadTurnContext,
        after: ThreadTurnContext,
    ) {
        if before == &after {
            return;
        }

        self.outgoing
            .send_server_notification(ServerNotification::ThreadTurnContextUpdated(
                ThreadTurnContextUpdatedNotification {
                    thread_id: thread_id.to_string(),
                    turn_context: after,
                },
            ))
            .await;
    }

    async fn turn_start_inner(
        &self,
        request_id: ConnectionRequestId,
        params: TurnStartParams,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) -> Result<TurnStartResponse, JSONRPCErrorError> {
        if let Err(error) = Self::validate_v2_input_limit(&params.input) {
            self.track_error_response(
                &request_id,
                &error,
                Some(AnalyticsJsonRpcError::Input(InputError::TooLarge)),
            );
            return Err(error);
        }
        let (thread_id, thread) =
            self.load_thread(&params.thread_id)
                .await
                .inspect_err(|error| {
                    self.track_error_response(&request_id, error, /*error_type*/ None);
                })?;
        Self::set_app_server_client_info(
            thread.as_ref(),
            app_server_client_name,
            app_server_client_version,
        )
        .await
        .inspect_err(|error| {
            self.track_error_response(&request_id, error, /*error_type*/ None);
        })?;

        let before_snapshot = thread.config_snapshot().await;
        let before_turn_context = thread_turn_context_from_snapshot(&before_snapshot);
        let environment_selections = self.parse_environment_selections(params.environments)?;

        // Map v2 input items to core input items.
        let mapped_items: Vec<CoreInputItem> = params
            .input
            .into_iter()
            .map(V2UserInput::into_core)
            .collect();
        let turn_has_input = !mapped_items.is_empty();
        let resolved_overrides = self
            .resolve_turn_context_overrides(
                &before_snapshot,
                TurnContextOverrideRequest {
                    cwd: params.cwd,
                    approval_policy: params.approval_policy,
                    approvals_reviewer: params.approvals_reviewer,
                    sandbox_policy: params.sandbox_policy,
                    permissions: params.permissions,
                    model: params.model,
                    service_tier: params.service_tier,
                    effort: params.effort.map(Some),
                    summary: params.summary,
                    collaboration_mode: params.collaboration_mode,
                    personality: params.personality,
                },
            )
            .await?;
        let after_turn_context = if resolved_overrides.has_any_overrides {
            Some(thread_turn_context_from_snapshot(
                &thread
                    .preview_turn_context_overrides(resolved_overrides.overrides.clone())
                    .await
                    .map_err(|err| {
                        invalid_request(format!("invalid turn context override: {err}"))
                    })?,
            ))
        } else {
            None
        };

        // Start the turn by submitting the user input. Return its submission id as turn_id.
        let turn_op = if resolved_overrides.has_any_overrides {
            let overrides = resolved_overrides.overrides;
            Op::UserInputWithTurnContext {
                items: mapped_items,
                environments: environment_selections,
                final_output_json_schema: params.output_schema,
                responsesapi_client_metadata: params.responsesapi_client_metadata,
                cwd: overrides.cwd,
                approval_policy: overrides.approval_policy,
                approvals_reviewer: overrides.approvals_reviewer,
                sandbox_policy: overrides.sandbox_policy,
                permission_profile: overrides.permission_profile,
                active_permission_profile: overrides.active_permission_profile,
                windows_sandbox_level: overrides.windows_sandbox_level,
                model: overrides.model,
                effort: overrides.effort,
                summary: overrides.summary,
                service_tier: overrides.service_tier,
                collaboration_mode: overrides.collaboration_mode,
                personality: overrides.personality,
            }
        } else {
            Op::UserInput {
                items: mapped_items,
                environments: environment_selections,
                final_output_json_schema: params.output_schema,
                responsesapi_client_metadata: params.responsesapi_client_metadata,
            }
        };
        let turn_id = self
            .submit_core_op(&request_id, thread.as_ref(), turn_op)
            .await
            .map_err(|err| {
                let error = internal_error(format!("failed to start turn: {err}"));
                self.track_error_response(&request_id, &error, /*error_type*/ None);
                error
            })?;

        if turn_has_input {
            let config_snapshot = thread.config_snapshot().await;
            codex_memories_write::start_memories_startup_task(
                Arc::clone(&self.thread_manager),
                Arc::clone(&self.auth_manager),
                thread_id,
                Arc::clone(&thread),
                thread.config().await,
                &config_snapshot.session_source,
            );
        }
        if let Some(after_turn_context) = after_turn_context {
            self.maybe_emit_turn_context_updated(
                &params.thread_id,
                &before_turn_context,
                after_turn_context,
            )
            .await;
        }

        self.outgoing
            .record_request_turn_id(&request_id, &turn_id)
            .await;
        let turn = Turn {
            id: turn_id,
            items: vec![],
            items_view: TurnItemsView::NotLoaded,
            error: None,
            status: TurnStatus::InProgress,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        Ok(TurnStartResponse { turn })
    }

    async fn thread_turn_context_update_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadTurnContextUpdateParams,
    ) -> Result<ThreadTurnContextUpdateResponse, JSONRPCErrorError> {
        let (_, thread) = self
            .load_thread(&params.thread_id)
            .await
            .inspect_err(|error| {
                self.track_error_response(request_id, error, /*error_type*/ None);
            })?;
        let before_snapshot = thread.config_snapshot().await;
        let before_turn_context = thread_turn_context_from_snapshot(&before_snapshot);
        let resolved_overrides = self
            .resolve_turn_context_overrides(
                &before_snapshot,
                TurnContextOverrideRequest {
                    cwd: params.cwd,
                    approval_policy: params.approval_policy,
                    approvals_reviewer: params.approvals_reviewer,
                    sandbox_policy: params.sandbox_policy,
                    permissions: params.permissions,
                    model: params.model,
                    service_tier: params.service_tier,
                    effort: params.effort,
                    summary: params.summary,
                    collaboration_mode: params.collaboration_mode,
                    personality: params.personality,
                },
            )
            .await?;

        let after_snapshot = if resolved_overrides.has_any_overrides {
            thread
                .update_turn_context_overrides(resolved_overrides.overrides)
                .await
                .map_err(|err| invalid_request(format!("invalid turn context override: {err}")))?
        } else {
            before_snapshot
        };
        let after_turn_context = thread_turn_context_from_snapshot(&after_snapshot);
        self.maybe_emit_turn_context_updated(
            &params.thread_id,
            &before_turn_context,
            after_turn_context.clone(),
        )
        .await;

        Ok(ThreadTurnContextUpdateResponse {
            turn_context: after_turn_context,
        })
    }

    async fn thread_inject_items_response_inner(
        &self,
        params: ThreadInjectItemsParams,
    ) -> Result<ThreadInjectItemsResponse, JSONRPCErrorError> {
        let (_, thread) = self.load_thread(&params.thread_id).await?;

        let items = params
            .items
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                serde_json::from_value::<ResponseItem>(value)
                    .map_err(|err| format!("items[{index}] is not a valid response item: {err}"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(invalid_request)?;

        thread
            .inject_response_items(items)
            .await
            .map_err(|err| match err {
                CodexErr::InvalidRequest(message) => invalid_request(message),
                err => internal_error(format!("failed to inject response items: {err}")),
            })?;
        Ok(ThreadInjectItemsResponse {})
    }

    async fn set_app_server_client_info(
        thread: &CodexThread,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) -> Result<(), JSONRPCErrorError> {
        let mcp_elicitations_auto_deny = xcode_26_4_mcp_elicitations_auto_deny(
            app_server_client_name.as_deref(),
            app_server_client_version.as_deref(),
        );
        thread
            .set_app_server_client_info(
                app_server_client_name,
                app_server_client_version,
                mcp_elicitations_auto_deny,
            )
            .await
            .map_err(|err| internal_error(format!("failed to set app server client info: {err}")))
    }

    async fn turn_steer_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: TurnSteerParams,
    ) -> Result<TurnSteerResponse, JSONRPCErrorError> {
        let (_, thread) = self
            .load_thread(&params.thread_id)
            .await
            .inspect_err(|error| {
                self.track_error_response(request_id, error, /*error_type*/ None);
            })?;

        if params.expected_turn_id.is_empty() {
            return Err(invalid_request("expectedTurnId must not be empty"));
        }
        self.outgoing
            .record_request_turn_id(request_id, &params.expected_turn_id)
            .await;
        if let Err(error) = Self::validate_v2_input_limit(&params.input) {
            self.track_error_response(
                request_id,
                &error,
                Some(AnalyticsJsonRpcError::Input(InputError::TooLarge)),
            );
            return Err(error);
        }

        let mapped_items: Vec<CoreInputItem> = params
            .input
            .into_iter()
            .map(V2UserInput::into_core)
            .collect();

        let turn_id = thread
            .steer_input(
                mapped_items,
                Some(&params.expected_turn_id),
                params.responsesapi_client_metadata,
            )
            .await
            .map_err(|err| {
                let (message, data, error_type) = match err {
                    SteerInputError::NoActiveTurn(_) => (
                        "no active turn to steer".to_string(),
                        None,
                        Some(AnalyticsJsonRpcError::TurnSteer(
                            TurnSteerRequestError::NoActiveTurn,
                        )),
                    ),
                    SteerInputError::ExpectedTurnMismatch { expected, actual } => (
                        format!("expected active turn id `{expected}` but found `{actual}`"),
                        None,
                        Some(AnalyticsJsonRpcError::TurnSteer(
                            TurnSteerRequestError::ExpectedTurnMismatch,
                        )),
                    ),
                    SteerInputError::ActiveTurnNotSteerable { turn_kind } => {
                        let (message, turn_steer_error) = match turn_kind {
                            codex_protocol::protocol::NonSteerableTurnKind::Review => (
                                "cannot steer a review turn".to_string(),
                                TurnSteerRequestError::NonSteerableReview,
                            ),
                            codex_protocol::protocol::NonSteerableTurnKind::Compact => (
                                "cannot steer a compact turn".to_string(),
                                TurnSteerRequestError::NonSteerableCompact,
                            ),
                        };
                        let error = TurnError {
                            message: message.clone(),
                            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                                turn_kind: turn_kind.into(),
                            }),
                            additional_details: None,
                        };
                        let data = match serde_json::to_value(error) {
                            Ok(data) => Some(data),
                            Err(error) => {
                                tracing::error!(
                                    ?error,
                                    "failed to serialize active-turn-not-steerable turn error"
                                );
                                None
                            }
                        };
                        (
                            message,
                            data,
                            Some(AnalyticsJsonRpcError::TurnSteer(turn_steer_error)),
                        )
                    }
                    SteerInputError::EmptyInput => (
                        "input must not be empty".to_string(),
                        None,
                        Some(AnalyticsJsonRpcError::Input(InputError::Empty)),
                    ),
                };
                let mut error = invalid_request(message);
                error.data = data;
                self.track_error_response(request_id, &error, error_type);
                error
            })?;
        Ok(TurnSteerResponse { turn_id })
    }

    async fn prepare_realtime_conversation_thread(
        &self,
        request_id: &ConnectionRequestId,
        thread_id: &str,
    ) -> Result<Option<(ThreadId, Arc<CodexThread>)>, JSONRPCErrorError> {
        let (thread_id, thread) = self.load_thread(thread_id).await?;

        match self
            .ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                /*raw_events_enabled*/ false,
            )
            .await
        {
            Ok(EnsureConversationListenerResult::Attached) => {}
            Ok(EnsureConversationListenerResult::ConnectionClosed) => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        }

        if !thread.enabled(Feature::RealtimeConversation) {
            return Err(invalid_request(format!(
                "thread {thread_id} does not support realtime conversation"
            )));
        }

        Ok(Some((thread_id, thread)))
    }

    async fn thread_realtime_start_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeStartParams,
    ) -> Result<Option<ThreadRealtimeStartResponse>, JSONRPCErrorError> {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id, &params.thread_id)
            .await?
        else {
            return Ok(None);
        };
        self.submit_core_op(
            request_id,
            thread.as_ref(),
            Op::RealtimeConversationStart(ConversationStartParams {
                output_modality: params.output_modality,
                prompt: params.prompt,
                realtime_session_id: params.realtime_session_id,
                transport: params.transport.map(|transport| match transport {
                    ThreadRealtimeStartTransport::Websocket => {
                        ConversationStartTransport::Websocket
                    }
                    ThreadRealtimeStartTransport::Webrtc { sdp } => {
                        ConversationStartTransport::Webrtc { sdp }
                    }
                }),
                voice: params.voice,
            }),
        )
        .await
        .map_err(|err| internal_error(format!("failed to start realtime conversation: {err}")))?;
        Ok(Some(ThreadRealtimeStartResponse::default()))
    }

    async fn thread_realtime_append_audio_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeAppendAudioParams,
    ) -> Result<Option<ThreadRealtimeAppendAudioResponse>, JSONRPCErrorError> {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id, &params.thread_id)
            .await?
        else {
            return Ok(None);
        };
        self.submit_core_op(
            request_id,
            thread.as_ref(),
            Op::RealtimeConversationAudio(ConversationAudioParams {
                frame: params.audio.into(),
            }),
        )
        .await
        .map_err(|err| {
            internal_error(format!(
                "failed to append realtime conversation audio: {err}"
            ))
        })?;
        Ok(Some(ThreadRealtimeAppendAudioResponse::default()))
    }

    async fn thread_realtime_append_text_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeAppendTextParams,
    ) -> Result<Option<ThreadRealtimeAppendTextResponse>, JSONRPCErrorError> {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id, &params.thread_id)
            .await?
        else {
            return Ok(None);
        };
        self.submit_core_op(
            request_id,
            thread.as_ref(),
            Op::RealtimeConversationText(ConversationTextParams { text: params.text }),
        )
        .await
        .map_err(|err| {
            internal_error(format!(
                "failed to append realtime conversation text: {err}"
            ))
        })?;
        Ok(Some(ThreadRealtimeAppendTextResponse::default()))
    }

    async fn thread_realtime_stop_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadRealtimeStopParams,
    ) -> Result<Option<ThreadRealtimeStopResponse>, JSONRPCErrorError> {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id, &params.thread_id)
            .await?
        else {
            return Ok(None);
        };
        self.submit_core_op(request_id, thread.as_ref(), Op::RealtimeConversationClose)
            .await
            .map_err(|err| {
                internal_error(format!("failed to stop realtime conversation: {err}"))
            })?;
        Ok(Some(ThreadRealtimeStopResponse::default()))
    }

    fn build_review_turn(turn_id: String, display_text: &str) -> Turn {
        let items = if display_text.is_empty() {
            Vec::new()
        } else {
            vec![ThreadItem::UserMessage {
                id: turn_id.clone(),
                content: vec![V2UserInput::Text {
                    text: display_text.to_string(),
                    // Review prompt display text is synthesized; no UI element ranges to preserve.
                    text_elements: Vec::new(),
                }],
            }]
        };

        Turn {
            id: turn_id,
            items,
            items_view: TurnItemsView::NotLoaded,
            error: None,
            status: TurnStatus::InProgress,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }

    async fn emit_review_started(
        &self,
        request_id: &ConnectionRequestId,
        turn: Turn,
        review_thread_id: String,
    ) {
        let response = ReviewStartResponse {
            turn,
            review_thread_id,
        };
        self.outgoing
            .send_response(request_id.clone(), response)
            .await;
    }

    async fn start_inline_review(
        &self,
        request_id: &ConnectionRequestId,
        parent_thread: Arc<CodexThread>,
        review_request: ReviewRequest,
        display_text: &str,
        parent_thread_id: String,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let turn_id = self
            .submit_core_op(
                request_id,
                parent_thread.as_ref(),
                Op::Review { review_request },
            )
            .await
            .map_err(|err| internal_error(format!("failed to start review: {err}")))?;
        let turn = Self::build_review_turn(turn_id, display_text);
        self.emit_review_started(request_id, turn, parent_thread_id)
            .await;
        Ok(())
    }

    async fn start_detached_review(
        &self,
        request_id: &ConnectionRequestId,
        parent_thread_id: ThreadId,
        parent_thread: Arc<CodexThread>,
        review_request: ReviewRequest,
        display_text: &str,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        parent_thread.ensure_rollout_materialized().await;
        parent_thread.flush_rollout().await.map_err(|err| {
            internal_error(format!(
                "failed to flush parent thread {parent_thread_id}: {err}"
            ))
        })?;
        let parent_history = parent_thread
            .load_history(/*include_archived*/ true)
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to load parent thread {parent_thread_id}: {err}"
                ))
            })?;

        let mut config = self.config.as_ref().clone();
        if let Some(review_model) = &config.review_model {
            config.model = Some(review_model.clone());
        }

        let NewThread {
            thread_id,
            thread: review_thread,
            ..
        } = self
            .thread_manager
            .fork_thread_from_history(
                ForkSnapshot::Interrupted,
                config.clone(),
                InitialHistory::Resumed(ResumedHistory {
                    conversation_id: parent_thread_id,
                    history: parent_history.items,
                    rollout_path: parent_thread.rollout_path(),
                }),
                /*thread_source*/ None,
                /*persist_extended_history*/ false,
                self.request_trace_context(request_id).await,
            )
            .await
            .map_err(|err| {
                internal_error(format!("error creating detached review thread: {err}"))
            })?;

        log_listener_attach_result(
            self.ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                /*raw_events_enabled*/ false,
            )
            .await,
            thread_id,
            request_id.connection_id,
            "review thread",
        );

        let fallback_provider = self.config.model_provider_id.as_str();
        match review_thread
            .read_thread(
                /*include_archived*/ true, /*include_history*/ false,
            )
            .await
        {
            Ok(stored_thread) => {
                let (mut thread, _) =
                    thread_from_stored_thread(stored_thread, fallback_provider, &self.config.cwd);
                thread.session_id = review_thread.session_configured().session_id.to_string();
                self.thread_watch_manager
                    .upsert_thread_silently(thread.clone())
                    .await;
                thread.status = resolve_thread_status(
                    self.thread_watch_manager
                        .loaded_status_for_thread(&thread.id)
                        .await,
                    /*has_in_progress_turn*/ false,
                );
                let notif = thread_started_notification(thread);
                self.outgoing
                    .send_server_notification(ServerNotification::ThreadStarted(notif))
                    .await;
            }
            Err(err) => {
                tracing::warn!("failed to load summary for review thread {thread_id}: {err}");
            }
        }

        let turn_id = self
            .submit_core_op(
                request_id,
                review_thread.as_ref(),
                Op::Review { review_request },
            )
            .await
            .map_err(|err| {
                internal_error(format!("failed to start detached review turn: {err}"))
            })?;

        let turn = Self::build_review_turn(turn_id, display_text);
        let review_thread_id = thread_id.to_string();
        self.emit_review_started(request_id, turn, review_thread_id)
            .await;

        Ok(())
    }

    async fn review_start_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: ReviewStartParams,
    ) -> Result<(), JSONRPCErrorError> {
        let ReviewStartParams {
            thread_id,
            target,
            delivery,
        } = params;

        let (parent_thread_id, parent_thread) = self.load_thread(&thread_id).await?;
        let (review_request, display_text) = Self::review_request_from_target(target)?;
        match delivery.unwrap_or(ApiReviewDelivery::Inline).to_core() {
            CoreReviewDelivery::Inline => {
                self.start_inline_review(
                    request_id,
                    parent_thread,
                    review_request,
                    &display_text,
                    thread_id,
                )
                .await?;
            }
            CoreReviewDelivery::Detached => {
                self.start_detached_review(
                    request_id,
                    parent_thread_id,
                    parent_thread,
                    review_request,
                    &display_text,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn turn_interrupt_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: TurnInterruptParams,
    ) -> Result<Option<TurnInterruptResponse>, JSONRPCErrorError> {
        let TurnInterruptParams { thread_id, turn_id } = params;
        let is_startup_interrupt = turn_id.is_empty();

        let (thread_uuid, thread) = self.load_thread(&thread_id).await?;

        // Record turn interrupts so we can reply when TurnAborted arrives. Startup
        // interrupts do not have a turn and are acknowledged after submission.
        if !is_startup_interrupt {
            let thread_state = self.thread_state_manager.thread_state(thread_uuid).await;
            let is_running = matches!(thread.agent_status().await, AgentStatus::Running);
            {
                let mut thread_state = thread_state.lock().await;
                if let Some(active_turn) = thread_state.active_turn_snapshot() {
                    if active_turn.id != turn_id {
                        return Err(invalid_request(format!(
                            "expected active turn id {turn_id} but found {}",
                            active_turn.id
                        )));
                    }
                } else if thread_state.last_terminal_turn_id.as_deref() == Some(turn_id.as_str())
                    || !is_running
                {
                    return Err(invalid_request("no active turn to interrupt"));
                }
                thread_state.pending_interrupts.push(request_id.clone());
            }

            self.outgoing
                .record_request_turn_id(request_id, &turn_id)
                .await;
        }

        // Submit the interrupt. Turn interrupts respond upon TurnAborted; startup
        // interrupts respond here because startup cancellation has no turn event.
        match self
            .submit_core_op(request_id, thread.as_ref(), Op::Interrupt)
            .await
        {
            Ok(_) if is_startup_interrupt => Ok(Some(TurnInterruptResponse {})),
            Ok(_) => Ok(None),
            Err(err) => {
                if !is_startup_interrupt {
                    let thread_state = self.thread_state_manager.thread_state(thread_uuid).await;
                    let mut thread_state = thread_state.lock().await;
                    thread_state
                        .pending_interrupts
                        .retain(|pending_request_id| pending_request_id != request_id);
                }
                let interrupt_target = if is_startup_interrupt {
                    "startup"
                } else {
                    "turn"
                };
                Err(internal_error(format!(
                    "failed to interrupt {interrupt_target}: {err}"
                )))
            }
        }
    }

    fn listener_task_context(&self) -> ListenerTaskContext {
        ListenerTaskContext {
            thread_manager: Arc::clone(&self.thread_manager),
            thread_state_manager: self.thread_state_manager.clone(),
            outgoing: Arc::clone(&self.outgoing),
            pending_thread_unloads: Arc::clone(&self.pending_thread_unloads),
            thread_watch_manager: self.thread_watch_manager.clone(),
            thread_list_state_permit: self.thread_list_state_permit.clone(),
            fallback_model_provider: self.config.model_provider_id.clone(),
            codex_home: self.config.codex_home.to_path_buf(),
            skills_watcher: Arc::clone(&self.skills_watcher),
        }
    }

    async fn ensure_conversation_listener(
        &self,
        conversation_id: ThreadId,
        connection_id: ConnectionId,
        raw_events_enabled: bool,
    ) -> Result<EnsureConversationListenerResult, JSONRPCErrorError> {
        super::thread_lifecycle::ensure_conversation_listener(
            self.listener_task_context(),
            conversation_id,
            connection_id,
            raw_events_enabled,
        )
        .await
    }
}

fn thread_turn_context_from_snapshot(config_snapshot: &ThreadConfigSnapshot) -> ThreadTurnContext {
    ThreadTurnContext {
        model: config_snapshot.model.clone(),
        model_provider: config_snapshot.model_provider_id.clone(),
        service_tier: config_snapshot.service_tier.clone(),
        cwd: config_snapshot.cwd.clone(),
        approval_policy: config_snapshot.approval_policy.into(),
        approvals_reviewer: config_snapshot.approvals_reviewer.into(),
        sandbox_policy: thread_response_sandbox_policy(
            &config_snapshot.permission_profile,
            config_snapshot.cwd.as_path(),
        ),
        permission_profile: config_snapshot.permission_profile.clone().into(),
        active_permission_profile: thread_response_active_permission_profile(
            config_snapshot.active_permission_profile.clone(),
        ),
        effort: config_snapshot.reasoning_effort,
        summary: config_snapshot.reasoning_summary,
        personality: config_snapshot.personality,
        collaboration_mode: config_snapshot.collaboration_mode.clone(),
    }
}

fn xcode_26_4_mcp_elicitations_auto_deny(
    client_name: Option<&str>,
    client_version: Option<&str>,
) -> bool {
    // Xcode 26.4 shipped before app-server MCP elicitation requests were
    // client-visible. Keep elicitations auto-denied for that client line.
    // TODO: Remove this compatibility hack once Xcode 26.4 ages out.
    client_name == Some("Xcode")
        && client_version.is_some_and(|version| version.starts_with("26.4"))
}
