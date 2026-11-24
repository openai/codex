use codex_app_server_protocol::JSONRPCErrorError;
use codex_thread_store::ThreadStoreError;
use pretty_assertions::assert_eq;

#[cfg(unix)]
#[test]
fn thread_cwd_rejects_invalid_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let cwd = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(OsStr::from_bytes(
        b"/workspace-\xFF",
    ))
    .expect("absolute native path");
    assert_eq!(
        super::thread_cwd_for_api(&cwd).expect_err("cwd cannot be represented as a wire string"),
        JSONRPCErrorError {
            code: crate::error_code::INTERNAL_ERROR_CODE,
            message: "cannot encode thread cwd: path contains invalid UTF-8".into(),
            data: None,
        }
    );
}

#[test]
fn runtime_workspace_roots_reject_home_relative_paths() {
    use codex_utils_absolute_path::AbsolutePathBufGuard;
    use codex_utils_path_uri::LegacyAppPathString;

    let home = std::env::current_dir().expect("current directory");
    let result = AbsolutePathBufGuard::with_home_directory(&home, || {
        super::native_runtime_workspace_roots(vec![LegacyAppPathString::from_string("~/extra")])
    });
    assert_eq!(
        result.expect_err("runtime workspace root must be absolute"),
        JSONRPCErrorError {
            code: crate::error_code::INVALID_REQUEST_ERROR_CODE,
            message: "invalid runtimeWorkspaceRoots: path is not absolute: ~/extra".into(),
            data: None,
        }
    );
}

#[test]
fn storage_failure_classification_does_not_change_mutation_error_policy() {
    for error in [
        ThreadStoreError::Internal {
            message: "retryable conflict; outcome unknown".to_owned(),
        },
        ThreadStoreError::Retryable {
            message: "store temporarily unavailable".to_owned(),
        },
        ThreadStoreError::OutcomeUnknown {
            message: "connection lost after write".to_owned(),
        },
    ] {
        let mapped = super::thread_store_mutation_error("archive", error.clone());
        let expected = JSONRPCErrorError {
            code: -32603,
            message: format!("failed to archive session: {error}"),
            data: None,
        };
        assert_eq!(mapped, expected);
    }

    assert_eq!(
        super::thread_store_mutation_error(
            "archive",
            ThreadStoreError::Conflict {
                message: "writer already active".to_owned(),
            },
        ),
        JSONRPCErrorError {
            code: -32600,
            message: "writer already active".to_owned(),
            data: None,
        },
    );
}

mod thread_list_cwd_filter_tests {
    use super::super::normalize_thread_list_cwd_filters;
    use codex_app_server_protocol::ThreadListCwdFilter;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn normalize_thread_list_cwd_filter_preserves_absolute_paths() {
        let cwd = if cfg!(windows) {
            String::from(r"C:\srv\repo-b")
        } else {
            String::from("/srv/repo-b")
        };

        assert_eq!(
            normalize_thread_list_cwd_filters(Some(ThreadListCwdFilter::One(cwd.clone())))
                .expect("cwd filter should parse"),
            Some(vec![PathBuf::from(cwd)])
        );
    }

    #[test]
    fn normalize_thread_list_cwd_filter_resolves_relative_paths_against_server_cwd()
    -> std::io::Result<()> {
        let expected = AbsolutePathBuf::relative_to_current_dir("repo-b")?.to_path_buf();

        assert_eq!(
            normalize_thread_list_cwd_filters(Some(ThreadListCwdFilter::Many(vec![String::from(
                "repo-b"
            ),])))
            .expect("cwd filter should parse"),
            Some(vec![expected])
        );
        Ok(())
    }
}

mod background_terminal_pagination_tests {
    use super::super::paginate_background_terminals;
    use codex_app_server_protocol::ThreadBackgroundTerminal;
    use codex_utils_path_uri::LegacyAppPathString;
    use pretty_assertions::assert_eq;

    fn terminal(process_id: &str) -> ThreadBackgroundTerminal {
        let cwd = if cfg!(windows) { r"C:\tmp" } else { "/tmp" };

        ThreadBackgroundTerminal {
            item_id: format!("item-{process_id}"),
            process_id: process_id.to_string(),
            command: format!("command-{process_id}"),
            cwd: LegacyAppPathString::from_string(cwd),
            os_pid: None,
            cpu_percent: None,
            rss_kb: None,
        }
    }

    #[test]
    fn paginates_with_process_id_cursor() {
        let terminals = vec![
            terminal("1"),
            terminal("2"),
            terminal("3"),
            terminal("4"),
            terminal("5"),
        ];

        let (data, next_cursor) =
            paginate_background_terminals(&terminals, /*cursor*/ None, Some(2))
                .expect("valid page");

        assert_eq!(data, vec![terminal("1"), terminal("2")]);
        assert_eq!(next_cursor, Some("2".to_string()));
        let first_cursor = next_cursor;

        let terminals_without_anchor = vec![terminal("1"), terminal("3"), terminal("4")];
        let (data, next_cursor) =
            paginate_background_terminals(&terminals_without_anchor, first_cursor.clone(), Some(2))
                .expect("valid page");

        assert_eq!(data, vec![terminal("3"), terminal("4")]);
        assert_eq!(next_cursor, None);

        let (data, next_cursor) =
            paginate_background_terminals(&terminals, first_cursor, Some(2)).expect("valid page");

        assert_eq!(data, vec![terminal("3"), terminal("4")]);
        assert_eq!(next_cursor, Some("4".to_string()));

        assert!(
            paginate_background_terminals(&terminals, Some("missing".to_string()), Some(1))
                .is_err()
        );
    }
}

mod thread_processor_behavior_tests {
    async fn forked_from_id_from_rollout(path: &Path) -> Option<String> {
        codex_core::read_session_meta_line(path)
            .await
            .ok()
            .and_then(|meta_line| meta_line.meta.forked_from_id)
            .map(|thread_id| thread_id.to_string())
    }

    use super::super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use anyhow::Result;
    use chrono::DateTime;
    use chrono::Utc;
    use codex_app_server_protocol::ServerRequestPayload;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::ThreadItemPayload;
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_config::CloudConfigBundleLoader;
    use codex_config::LoaderOverrides;
    use codex_config::SessionThreadConfig;
    use codex_config::StaticThreadConfigLoader;
    use codex_config::ThreadConfigSource;
    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::WireApi;
    use codex_protocol::ThreadId;
    use codex_protocol::config_types::CollaborationMode;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::config_types::Settings;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::openai_models::ReasoningEffort;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use codex_protocol::protocol::TurnEnvironmentSelections;
    use codex_state::ThreadMetadataBuilder;
    use codex_thread_store::StoredThread;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn full_resume_restores_only_recoverable_mcp_results() -> Result<()> {
        use codex_protocol::protocol::HasLegacyEvent;
        use core_test_support::responses;

        let server = responses::start_mock_server().await;
        responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("seed"),
                responses::ev_completed("seed"),
            ]),
        )
        .await;
        let test = core_test_support::test_codex::test_codex()
            .with_history_mode(ThreadHistoryMode::Paginated)
            .build_with_auto_env(&server)
            .await?;
        test.submit_turn("seed history").await?;
        let store = test.thread_store.as_ref();
        let thread_id = test.session_configured.thread_id;
        let started: EventMsg =
            serde_json::from_value(json!({"type": "turn_started", "turn_id": "turn"}))?;
        let mut live = ThreadHistoryBuilder::new();
        live.handle_live_paginated_event(&started);
        let mut saved = vec![RolloutItem::EventMsg(started)];
        let completion = |id: &str, text: &str, view: &str| -> serde_json::Result<EventMsg> {
            serde_json::from_value(json!({
                "type": "item_completed", "thread_id": thread_id, "turn_id": "turn",
                "started_at_ms": 2000, "completed_at_ms": 2007,
                "item": {
                    "type": "McpToolCall", "id": id, "server": "docs", "tool": "read",
                    "arguments": {}, "status": if id == "failed" { "failed" } else { "completed" },
                    "duration": {"secs": 0, "nanos": 7000000},
                    "result": {"content": [{"type": "text", "text": text}], "isError": id == "failed",
                               "structuredContent": {"id": id}, "_meta": {"view": view}}
                }
            }))
        };
        let deliver = |live: &mut ThreadHistoryBuilder, event: &EventMsg| {
            live.handle_live_paginated_event(event);
            for legacy in event.as_legacy_events(/*show_raw_agent_reasoning*/ false) {
                live.handle_live_paginated_event(&legacy);
            }
        };

        // The prepared full page predates a correction to this same call ID.
        let old = completion("exact", "old result", "detail")?;
        deliver(&mut live, &old);
        saved.push(RolloutItem::EventMsg(old));
        let mut page = live.active_turn_snapshot().expect("prepared turn");
        let ids = ["exact", "preview", "metadata", "missing", "failed"];
        let oversized = "x".repeat(200_000);
        for id in ids {
            let text = if id == "preview" {
                &oversized
            } else {
                "full result"
            };
            let event = completion(id, text, "detail")?;
            deliver(&mut live, &event);
            match id {
                "missing" => {}
                "metadata" => saved.push(RolloutItem::EventMsg(completion(id, text, "summary")?)),
                _ => saved.push(RolloutItem::EventMsg(event)),
            }
        }
        store
            .append_items(codex_thread_store::AppendThreadItemsParams {
                thread_id,
                items: saved,
            })
            .await?;
        let rows = ThreadRequestProcessor::read_thread_items(
            store,
            ThreadItemsReadParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn".to_string(),
                item_ids: ids.map(str::to_string).into(),
            },
        )
        .await
        .expect("read saved results");
        assert_eq!(
            rows.data
                .iter()
                .map(|entry| entry.item.id())
                .collect::<Vec<_>>(),
            ["exact", "preview", "metadata", "failed"]
        );

        let expected = live.active_turn_snapshot().expect("full turn");
        assert_eq!(
            expected
                .items
                .iter()
                .map(ThreadItem::id)
                .collect::<Vec<_>>(),
            ids
        );
        live.release_saved_mcp_results(&rows.data);
        let mut resumed = live.active_turn_snapshot().expect("evicted turn");
        let mut evicted = expected.clone();
        let ThreadItemPayload::McpToolCall { result, .. } = &mut evicted.items[0].payload else {
            panic!("expected MCP call")
        };
        assert!(result.take().is_some());
        assert_eq!(resumed, evicted);

        crate::request_processors::thread_lifecycle::restore_evicted_payloads(
            store,
            thread_id,
            &mut resumed,
            std::iter::once(&mut page),
        )
        .await
        .expect("full reconnect");
        assert_eq!(resumed, expected);
        Ok(())
    }

    fn dynamic_tool(
        namespace: Option<&str>,
        name: impl Into<String>,
        input_schema: Value,
        defer_loading: bool,
    ) -> DynamicToolSpec {
        let function = DynamicToolFunctionSpec {
            name: name.into(),
            description: "test".to_string(),
            input_schema,
            defer_loading,
        };
        match namespace {
            Some(namespace) => {
                DynamicToolSpec::Namespace(codex_app_server_protocol::DynamicToolNamespaceSpec {
                    name: namespace.to_string(),
                    description: "test namespace".to_string(),
                    tools: vec![DynamicToolNamespaceTool::Function(function)],
                })
            }
            None => DynamicToolSpec::Function(function),
        }
    }

    #[test]
    fn validate_dynamic_tools_rejects_unsupported_input_schema() {
        let tools = vec![dynamic_tool(
            /*namespace*/ None,
            "my_tool",
            json!({"type": "null"}),
            /*defer_loading*/ false,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("invalid schema");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_accepts_sanitizable_input_schema() {
        let tools = vec![dynamic_tool(
            /*namespace*/ None,
            "my_tool",
            // Missing `type` is common; core sanitizes these to a supported schema.
            json!({"properties": {}}),
            /*defer_loading*/ false,
        )];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_accepts_nullable_field_schema() {
        let tools = vec![dynamic_tool(
            /*namespace*/ None,
            "my_tool",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": ["string", "null"]}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            /*defer_loading*/ false,
        )];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_accepts_same_name_in_different_namespaces() {
        let tools = vec![
            dynamic_tool(
                Some("codex_app"),
                "my_tool",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                /*defer_loading*/ true,
            ),
            dynamic_tool(
                Some("other_app"),
                "my_tool",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                /*defer_loading*/ true,
            ),
        ];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_accepts_responses_compatible_identifiers() {
        let tools = vec![dynamic_tool(
            Some("Codex-App_2"),
            "lookup-ticket_2",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ true,
        )];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_rejects_duplicate_name_in_same_namespace() {
        let function = || DynamicToolFunctionSpec {
            name: "my_tool".to_string(),
            description: "test".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            defer_loading: true,
        };
        let tools = vec![DynamicToolSpec::Namespace(
            codex_app_server_protocol::DynamicToolNamespaceSpec {
                name: "codex_app".to_string(),
                description: "test namespace".to_string(),
                tools: vec![
                    DynamicToolNamespaceTool::Function(function()),
                    DynamicToolNamespaceTool::Function(function()),
                ],
            },
        )];
        let err = validate_dynamic_tools(&tools).expect_err("duplicate name");
        assert!(err.contains("codex_app"), "unexpected error: {err}");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
    }

    #[test]
    fn thread_turns_list_merges_in_progress_active_turn_before_agent_status_running() {
        let persisted_items = vec![RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                client_id: None,
                message: "persisted".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
                ..Default::default()
            },
        ))];
        let active_turn = Turn {
            id: "live-turn".to_string(),
            root_turn_id: None,
            items: vec![ThreadItem::untimed(ThreadItemPayload::UserMessage {
                id: "live-user-message".to_string(),
                client_id: None,
                content: vec![V2UserInput::Text {
                    text: "live".to_string(),
                    text_elements: Vec::new(),
                }],
            })],
            items_view: TurnItemsView::Full,
            error: None,
            status: TurnStatus::InProgress,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        let turns = reconstruct_thread_turns_for_turns_list(
            &persisted_items,
            ThreadStatus::Idle,
            /*has_live_running_thread*/ false,
            Some(active_turn.clone()),
        );

        assert_eq!(turns.last(), Some(&active_turn));
    }

    #[test]
    fn validate_dynamic_tools_rejects_empty_namespace() {
        let tools = vec![dynamic_tool(
            Some(""),
            "my_tool",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ false,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("empty namespace");
        assert!(err.contains("namespace"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_rejects_reserved_namespace() {
        let tools = vec![dynamic_tool(
            Some("mcp__server__"),
            "my_tool",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ false,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("reserved namespace");
        assert!(err.contains("reserved"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_rejects_name_not_supported_by_responses() {
        let tools = vec![dynamic_tool(
            /*namespace*/ None,
            "lookup.ticket",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ false,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("invalid name");
        assert!(err.contains("lookup.ticket"), "unexpected error: {err}");
        assert!(
            err.contains("Responses API") && err.contains("^[a-zA-Z0-9_-]+$"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_dynamic_tools_rejects_namespace_not_supported_by_responses() {
        let tools = vec![dynamic_tool(
            Some("codex.app"),
            "lookup_ticket",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ true,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("invalid namespace");
        assert!(err.contains("codex.app"), "unexpected error: {err}");
        assert!(
            err.contains("Responses API") && err.contains("^[a-zA-Z0-9_-]+$"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_dynamic_tools_rejects_name_longer_than_responses_limit() {
        let long_name = "a".repeat(129);
        let tools = vec![dynamic_tool(
            /*namespace*/ None,
            long_name.clone(),
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ false,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("name too long");
        assert!(err.contains("at most 128"), "unexpected error: {err}");
        assert!(err.contains(&long_name), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_rejects_namespace_fields_over_limits() {
        let long_namespace = "a".repeat(65);
        let mut tools = vec![dynamic_tool(
            Some(&long_namespace),
            "lookup_ticket",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ true,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("namespace too long");
        assert!(err.contains("at most 64"), "unexpected error: {err}");
        assert!(err.contains(&long_namespace), "unexpected error: {err}");

        let DynamicToolSpec::Namespace(namespace) = &mut tools[0] else {
            unreachable!("expected namespace")
        };
        namespace.name = "tickets".to_string();
        namespace.description = "a".repeat(1025);
        let err = validate_dynamic_tools(&tools).expect_err("namespace description too long");
        assert!(err.contains("at most 1024"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_rejects_reserved_responses_namespace() {
        let tools = vec![dynamic_tool(
            Some("functions"),
            "lookup_ticket",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            /*defer_loading*/ true,
        )];
        let err = validate_dynamic_tools(&tools).expect_err("reserved Responses namespace");
        assert!(err.contains("functions"), "unexpected error: {err}");
        assert!(err.contains("Responses API"), "unexpected error: {err}");
    }

    #[test]
    fn stored_thread_conversions_preserve_timestamps_and_agent_metadata() {
        let created_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:05.678Z").expect("valid timestamp");
        let updated_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:06.789Z").expect("valid timestamp");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread");
        let stored_thread = StoredThread {
            creator_user_id: None,
            creator_account_id: None,
            originator: None,
            thread_id,
            extra_config: None,
            rollout_path: Some(PathBuf::from("/tmp/thread.jsonl")),
            forked_from_id: None,
            attached_to_thread_id: None,
            parent_thread_id: None,
            preview: "preview".to_string(),
            name: None,
            model_provider: "openai".to_string(),
            model: None,
            reasoning_effort: None,
            created_at: created_at.with_timezone(&Utc),
            updated_at: updated_at.with_timezone(&Utc),
            recency_at: updated_at.with_timezone(&Utc),
            archived_at: None,
            section: None,
            section_position: None,
            section_entered_at: None,
            project_id: None,
            daybreak_enabled: None,
            cwd: PathBuf::from("/tmp"),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: ThreadId::new(),
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }),
            history_mode: Default::default(),
            thread_source: Some(codex_protocol::protocol::ThreadSource::Subagent),
            agent_nickname: Some("atlas".to_string()),
            agent_role: Some("explorer".to_string()),
            agent_path: None,
            git_info: None,
            approval_mode: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            token_usage: None,
            first_user_message: Some("first user message".to_string()),
            history: None,
        };

        let fallback_cwd = AbsolutePathBuf::current_dir().expect("current directory");
        let (thread, _) =
            thread_from_stored_thread(stored_thread.clone(), "fallback", &fallback_cwd)
                .expect("stored thread should convert");
        let summary = summary_from_stored_thread(stored_thread, "fallback");
        assert_eq!(
            (
                thread.agent_nickname.as_deref(),
                thread.agent_role.as_deref()
            ),
            (Some("atlas"), Some("explorer")),
        );
        assert_eq!(
            (
                summary.source.get_nickname(),
                summary.source.get_agent_role()
            ),
            (Some("atlas".to_string()), Some("explorer".to_string())),
        );

        assert_eq!(
            summary.timestamp.as_deref(),
            Some("2025-01-02T03:04:05.678Z")
        );
        assert_eq!(
            summary.updated_at.as_deref(),
            Some("2025-01-02T03:04:06.789Z")
        );
    }

    #[test]
    fn config_load_error_marks_cloud_config_bundle_failures_for_relogin() {
        let err = std::io::Error::other(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Auth,
            Some(401),
            "Your authentication session could not be refreshed automatically. Please log out and sign in again.",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudConfigBundle",
                "errorCode": "Auth",
                "action": "relogin",
                "statusCode": 401,
                "detail": "Your authentication session could not be refreshed automatically. Please log out and sign in again.",
            }))
        );
        assert!(
            error.message.contains("failed to load configuration"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn config_load_error_leaves_non_cloud_config_bundle_failures_unmarked() {
        let err = std::io::Error::other("required MCP servers failed to initialize");

        let error = config_load_error(&err);

        assert_eq!(error.data, None);
        assert!(
            error.message.contains("failed to load configuration"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn config_load_error_marks_non_auth_cloud_config_bundle_failures_without_relogin() {
        let err = std::io::Error::other(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::RequestFailed,
            /*status_code*/ None,
            "Failed to load cloud config bundle (workspace-managed policies).",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudConfigBundle",
                "errorCode": "RequestFailed",
                "detail": "Failed to load cloud config bundle (workspace-managed policies).",
            }))
        );
    }

    #[test]
    fn config_load_error_marks_invalid_cloud_config_bundle_failures_without_relogin() {
        let err = std::io::Error::other(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::InvalidBundle,
            /*status_code*/ None,
            "invalid cloud config bundle: invalid cloud config fragment Base policy (cfg_123)",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudConfigBundle",
                "errorCode": "InvalidBundle",
                "detail": "invalid cloud config bundle: invalid cloud config fragment Base policy (cfg_123)",
            }))
        );
    }

    #[tokio::test]
    async fn derive_config_from_params_uses_session_thread_config_model_provider() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let session_provider = ModelProviderInfo {
            name: "session".to_string(),
            base_url: Some("http://127.0.0.1:8061/api/codex".to_string()),
            model_catalog_url: None,
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            gateway_oauth: None,
            aws: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_openai_auth: false,
            supports_websockets: true,
            supports_standalone_web_search: false,
            capabilities: None,
            project_id: None,
            location: None,
            credential_file: None,
            include_internal_metadata: false,
        };
        let config_manager = ConfigManager::new(
            temp_dir.path().to_path_buf(),
            Vec::new(),
            LoaderOverrides::default(),
            /*strict_config*/ false,
            CloudConfigBundleLoader::default(),
            Arg0DispatchPaths::default(),
            Arc::new(StaticThreadConfigLoader::new(vec![
                ThreadConfigSource::Session(SessionThreadConfig {
                    model_provider: Some("session".to_string()),
                    model_providers: HashMap::from([(
                        "session".to_string(),
                        session_provider.clone(),
                    )]),
                    features: BTreeMap::from([("plugins".to_string(), false)]),
                }),
            ])),
        );
        let config = config_manager
            .load_with_overrides(
                Some(HashMap::from([
                    ("model_provider".to_string(), json!("request")),
                    ("features.plugins".to_string(), json!(true)),
                    ("bypass_hook_trust".to_string(), json!(true)),
                    (
                        "model_providers.session".to_string(),
                        json!({
                            "name": "request",
                            "base_url": "http://127.0.0.1:9999/api/codex",
                            "wire_api": "responses",
                        }),
                    ),
                ])),
                ConfigOverrides::default(),
            )
            .await?;

        assert_eq!(config.model_provider_id, "session");
        assert_eq!(config.model_provider, session_provider);
        assert!(!config.features.enabled(Feature::Plugins));
        assert!(config.bypass_hook_trust);
        Ok(())
    }

    #[test]
    fn collect_resume_override_mismatches_includes_service_tier() {
        let cwd = test_path_buf("/tmp").abs();
        let request = ThreadResumeParams {
            thread_id: "thread-1".to_string(),
            history: None,
            path: None,
            model: None,
            model_provider: None,
            service_tier: Some(Some("priority".to_string())),
            cwd: None,
            runtime_workspace_roots: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox: None,
            permissions: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
            personality: None,
            exclude_turns: false,
            initial_turns_page: None,
        };
        let config_snapshot = ThreadConfigSnapshot {
            request_country: Default::default(),
            turn_extension_init: Default::default(),
            disabled_plugin_ids: Vec::new(),
            model: "gpt-5".to_string(),
            model_provider_id: "openai".to_string(),
            service_tier: Some("flex".to_string()),
            approval_policy: codex_protocol::protocol::AskForApproval::OnRequest,
            approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer::User,
            permission_profile: codex_protocol::models::PermissionProfile::Disabled,
            full_access: false,
            active_permission_profile: None,
            environments: TurnEnvironmentSelections::new(cwd, Vec::new()),
            workspace_roots: Vec::new(),
            profile_workspace_roots: Vec::new(),
            ephemeral: false,
            reasoning_effort: None,
            reasoning_summary: None,
            personality: None,
            collaboration_mode: CollaborationMode {
                mode: ModeKind::Default,
                settings: Settings {
                    model: "gpt-5".to_string(),
                    reasoning_effort: None,
                    developer_instructions: None,
                },
            },
            session_source: SessionSource::Cli,
            history_mode: Default::default(),
            forked_from_thread_id: None,
            parent_thread_id: None,
            thread_source: None,
            originator: "test_originator".to_string(),
        };

        assert_eq!(
            collect_resume_override_mismatches(
                &request,
                &config_snapshot,
                /* selected_service_tier */ Some("flex"),
            )
            .unwrap(),
            vec!["service_tier requested=Some(\"priority\") active=Some(\"flex\")".to_string()]
        );
    }

    fn test_thread_metadata(
        model: Option<&str>,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Result<ThreadMetadata> {
        let thread_id = ThreadId::from_string("3f941c35-29b3-493b-b0a4-e25800d9aeb0")?;
        let builder = ThreadMetadataBuilder::new(
            thread_id,
            PathBuf::from("/tmp/rollout.jsonl"),
            Utc::now(),
            codex_protocol::protocol::SessionSource::default(),
        )
        .with_model_provider(Some("mock_provider".to_string()));
        let mut metadata = builder.build("mock_provider");
        metadata.model = model.map(ToString::to_string);
        metadata.reasoning_effort = reasoning_effort;
        Ok(metadata)
    }

    #[test]
    fn merge_persisted_resume_metadata_prefers_persisted_model_and_reasoning_effort() -> Result<()>
    {
        let mut request_overrides = None;
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(
            typesafe_overrides.model,
            Some("gpt-5.1-codex-max".to_string())
        );
        assert_eq!(
            typesafe_overrides.model_provider,
            Some("mock_provider".to_string())
        );
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::Value::String("high".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_preserves_explicit_overrides() -> Result<()> {
        let mut request_overrides = Some(HashMap::from([(
            "model_reasoning_effort".to_string(),
            serde_json::Value::String("low".to_string()),
        )]));
        let mut typesafe_overrides = ConfigOverrides {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        };
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, Some("gpt-5.2-codex".to_string()));
        assert_eq!(
            typesafe_overrides.model_provider,
            Some("mock_provider".to_string())
        );
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::Value::String("low".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_preserves_provider_when_model_overridden() -> Result<()> {
        let mut request_overrides = Some(HashMap::from([(
            "model".to_string(),
            serde_json::Value::String("gpt-5.2-codex".to_string()),
        )]));
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(
            typesafe_overrides.model_provider,
            Some("mock_provider".to_string())
        );
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model".to_string(),
                serde_json::Value::String("gpt-5.2-codex".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_skips_persisted_values_when_provider_overridden()
    -> Result<()> {
        let mut request_overrides = None;
        let mut typesafe_overrides = ConfigOverrides {
            model_provider: Some("oss".to_string()),
            ..Default::default()
        };
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(typesafe_overrides.model_provider, Some("oss".to_string()));
        assert_eq!(request_overrides, None);
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_preserves_provider_when_reasoning_effort_overridden()
    -> Result<()> {
        let mut request_overrides = Some(HashMap::from([(
            "model_reasoning_effort".to_string(),
            serde_json::Value::String("low".to_string()),
        )]));
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(
            typesafe_overrides.model_provider,
            Some("mock_provider".to_string())
        );
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::Value::String("low".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_skips_missing_values() -> Result<()> {
        let mut request_overrides = None;
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(/*model*/ None, /*reasoning_effort*/ None)?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(
            typesafe_overrides.model_provider,
            Some("mock_provider".to_string())
        );
        assert_eq!(request_overrides, None);
        Ok(())
    }

    #[tokio::test]
    async fn read_summary_from_rollout_preserves_forked_from_id() -> Result<()> {
        use codex_protocol::protocol::SessionMetaLine;
        use codex_rollout::RolloutItem;
        use codex_rollout::RolloutLine;
        use std::fs;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let forked_from_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            session_id: conversation_id.into(),
            id: conversation_id,
            forked_from_id: Some(forked_from_id),
            timestamp: timestamp.clone(),
            model_provider: Some("test-provider".to_string()),
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp,
            ordinal: None,
            item: RolloutItem::SessionMeta(Box::new(SessionMetaLine {
                meta: session_meta,
                git: None,
            })),
        };
        fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;

        assert_eq!(
            forked_from_id_from_rollout(path.as_path()).await,
            Some(forked_from_id.to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn aborting_pending_request_clears_pending_state() -> Result<()> {
        let thread_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let connection_id = ConnectionId(7);

        let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            codex_analytics::AnalyticsEventsClient::disabled(),
        ));
        let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing.clone(),
            vec![connection_id],
            thread_id,
        );

        let (request_id, client_request_rx) = thread_outgoing
            .send_request(ServerRequestPayload::ToolRequestUserInput(
                ToolRequestUserInputParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "call-1".to_string(),
                    questions: vec![],
                    is_blocking: true,
                    auto_resolution_ms: None,
                },
            ))
            .await;
        thread_outgoing.abort_pending_server_requests().await;

        let request_message = outgoing_rx.recv().await.expect("request should be sent");
        let OutgoingEnvelope::ToConnection {
            connection_id: request_connection_id,
            message:
                OutgoingMessage::Request(ServerRequest::ToolRequestUserInput {
                    request_id: sent_request_id,
                    ..
                }),
            ..
        } = request_message
        else {
            panic!("expected tool request to be sent to the subscribed connection");
        };
        assert_eq!(request_connection_id, connection_id);
        assert_eq!(sent_request_id, request_id);

        let response = client_request_rx
            .await
            .expect("callback should be resolved");
        let error = response.expect_err("request should be aborted during cleanup");
        assert_eq!(
            error.message,
            "client request resolved because the turn state was changed"
        );
        assert_eq!(error.data, Some(json!({ "reason": "turnTransition" })));
        assert!(
            outgoing
                .pending_requests_for_thread(thread_id)
                .await
                .is_empty()
        );
        assert!(outgoing_rx.try_recv().is_err());
        Ok(())
    }

    #[tokio::test]
    async fn late_turn_completion_keeps_the_next_command_approval_answerable() -> Result<()> {
        use crate::request_processors::thread_lifecycle::ListenerTaskContext;
        use crate::request_processors::thread_lifecycle::ensure_conversation_listener;
        use codex_app_server_protocol::ThreadActiveFlag;
        use codex_extension_api::ExtensionFuture;
        use codex_extension_api::ExtensionRegistryBuilder;
        use codex_extension_api::TurnLifecycleContributor;
        use codex_extension_api::TurnStopInput;
        use core_test_support::responses;
        use core_test_support::test_codex::test_codex;

        #[derive(Default)]
        struct StopGate {
            entered: CancellationToken,
            release: CancellationToken,
        }

        impl TurnLifecycleContributor for StopGate {
            fn on_turn_stop<'a>(&'a self, _: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
                Box::pin(async {
                    if !self.entered.is_cancelled() {
                        self.entered.cancel();
                        self.release.cancelled().await;
                    }
                })
            }
        }

        let gate = Arc::new(StopGate::default());
        let _release_on_drop = gate.release.clone().drop_guard();
        let mut extensions = ExtensionRegistryBuilder::new();
        extensions.turn_lifecycle_contributor(gate.clone());
        let server = responses::start_mock_server().await;
        let model = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse_completed("first"),
                responses::sse(vec![
                    responses::ev_exec_command_call_with_args(
                        "command",
                        &json!({
                            "cmd": "echo approval-delivered",
                            "sandbox_permissions": "require_escalated",
                            "justification": "test approval"
                        }),
                    ),
                    responses::ev_completed("command-request"),
                ]),
                responses::sse_completed("second"),
            ],
        )
        .await;
        let test = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_config(|config| {
                config.permissions.approval_policy =
                    codex_config::Constrained::allow_any(AskForApproval::OnRequest);
                config
                    .permissions
                    .set_permission_profile(PermissionProfile::workspace_write())
                    .expect("test allows workspace-write");
                config.approvals_reviewer = codex_protocol::config_types::ApprovalsReviewer::User;
            })
            .build_with_auto_env(&server)
            .await?;
        let thread_id = test.session_configured.thread_id;
        let connection_id = ConnectionId(7);
        let (tx, mut rx) = tokio::sync::mpsc::channel(crate::CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            tx,
            codex_analytics::AnalyticsEventsClient::disabled(),
        ));
        let thread_state_manager = ThreadStateManager::new();
        thread_state_manager
            .connection_initialized(connection_id, ConnectionCapabilities::default())
            .await;
        let skills_watcher = SkillsWatcher::new(
            test.thread_manager.skills_service(),
            &test.config.codex_home,
            outgoing.clone(),
        );
        let thread_watch_manager = ThreadWatchManager::new();
        let thread_status = thread_watch_manager.subscribe(thread_id).await;
        ensure_conversation_listener(
            ListenerTaskContext {
                thread_manager: test.thread_manager.clone(),
                thread_store: Arc::clone(&test.thread_store),
                thread_state_manager,
                outgoing: outgoing.clone(),
                pending_thread_unloads: Default::default(),
                thread_watch_manager,
                codex_home: test.config.codex_home.to_path_buf(),
                skills_watcher: skills_watcher.clone(),
                turn_cost_worker: None,
            },
            thread_id,
            connection_id,
            /*raw_events_enabled*/ false,
        )
        .await
        .expect("listener should attach to the test thread");

        let start = async |text: &str| -> Result<String> {
            let TurnInputSubmission::Started { turn_id, .. } = test
                .codex
                .start_or_steer_turn(TurnInputRequest::user_input(vec![CoreInputItem::Text {
                    text: text.to_owned(),
                    text_elements: vec![],
                }]))
                .await?
            else {
                anyhow::bail!("expected a new turn")
            };
            Ok(turn_id)
        };
        let first = start("first").await?;
        tokio::time::timeout(std::time::Duration::from_secs(20), gate.entered.cancelled())
            .await
            .expect("A must reach its stop hook");
        let second = start("second").await?;
        let mut command_events = Vec::new();
        let mut next_message = async |stage: &str| loop {
            if let OutgoingEnvelope::ToConnection { message, .. } =
                tokio::time::timeout(std::time::Duration::from_secs(20), rx.recv())
                    .await
                    .expect(stage)
                    .expect("listener closed")
            {
                if let OutgoingMessage::AppServerNotification(envelope) = &message {
                    match &envelope.notification {
                        ServerNotification::ItemStarted(n)
                            if n.turn_id == second && n.item.id() == "command" =>
                        {
                            command_events.push("started");
                        }
                        ServerNotification::ItemCompleted(n)
                            if n.turn_id == second && n.item.id() == "command" =>
                        {
                            command_events.push("completed");
                        }
                        _ => {}
                    }
                }
                break message;
            }
        };
        let approval_id = loop {
            match next_message("waiting for B to request approval").await {
                OutgoingMessage::Request(ServerRequest::CommandExecutionRequestApproval {
                    request_id,
                    params,
                }) => {
                    assert_eq!(params.turn_id, second);
                    break request_id;
                }
                OutgoingMessage::AppServerNotification(envelope)
                    if matches!(envelope.notification, ServerNotification::TurnCompleted(_)) =>
                {
                    anyhow::bail!("turn completed before B requested approval");
                }
                _ => {}
            }
        };

        gate.release.cancel();
        let mut wait_for_completion = async |expected: &str| {
            loop {
                if let OutgoingMessage::AppServerNotification(envelope) =
                    next_message(expected).await
                    && let ServerNotification::TurnCompleted(completed) = envelope.notification
                {
                    assert_eq!(completed.turn.id, expected);
                    assert!(completed.turn.started_at.is_some());
                    break;
                }
            }
        };
        wait_for_completion(&first).await;
        assert_eq!(
            *thread_status.borrow(),
            ThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
            }
        );
        assert!(
            outgoing
                .pending_requests_for_thread(thread_id)
                .await
                .iter()
                .any(|request| request.id() == &approval_id),
            "A's completion removed B's approval"
        );
        outgoing
            .notify_client_response(connection_id, approval_id, json!({"decision": "accept"}))
            .await;
        wait_for_completion(&second).await;
        assert_eq!(command_events, ["started", "completed"]);
        let output = model
            .function_call_output_text("command")
            .expect("command output");
        assert!(
            output
                .lines()
                .any(|line| line.trim() == "approval-delivered"),
            "{output}"
        );
        test.codex.shutdown_and_wait().await?;
        skills_watcher.shutdown();
        Ok(())
    }

    #[tokio::test]
    async fn removing_thread_state_clears_listener_and_active_turn_history() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection = ConnectionId(1);
        let (cancel_tx, cancel_rx) = oneshot::channel();

        manager
            .connection_initialized(connection, ConnectionCapabilities::default())
            .await;
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, connection)
                .await,
            "connection should be live"
        );
        {
            let state = manager.thread_state(thread_id).await;
            let mut state = state.lock().await;
            state.cancel_tx = Some(cancel_tx);
            state.track_current_turn_event(
                "turn-1",
                &EventMsg::TurnStarted(codex_protocol::protocol::TurnStartedEvent {
                    turn_attribution: None,
                    turn_id: "turn-1".to_string(),
                    root_turn_id: None,
                    trace_id: None,
                    started_at: None,
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                }),
            );
        }

        manager.remove_thread_state(thread_id).await;
        assert_eq!(cancel_rx.await, Ok(()));

        let state = manager.thread_state(thread_id).await;
        let subscribed_connection_ids = manager.subscribed_connection_ids(thread_id).await;
        assert!(subscribed_connection_ids.is_empty());
        let state = state.lock().await;
        assert!(state.cancel_tx.is_none());
        assert!(state.active_turn_snapshot().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn removing_auto_attached_connection_preserves_listener_for_other_connections()
    -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);
        let (cancel_tx, mut cancel_rx) = oneshot::channel();

        manager
            .connection_initialized(connection_a, ConnectionCapabilities::default())
            .await;
        manager
            .connection_initialized(connection_b, ConnectionCapabilities::default())
            .await;
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, connection_a)
                .await,
            "connection_a should be live"
        );
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, connection_b)
                .await,
            "connection_b should be live"
        );
        {
            let state = manager.thread_state(thread_id).await;
            state.lock().await.cancel_tx = Some(cancel_tx);
        }

        let threads_to_unload = manager.remove_connection(connection_a).await;
        assert_eq!(threads_to_unload, Vec::<ThreadId>::new());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut cancel_rx)
                .await
                .is_err()
        );

        assert_eq!(
            manager.subscribed_connection_ids(thread_id).await,
            vec![connection_b]
        );
        Ok(())
    }

    #[tokio::test]
    async fn adding_connection_to_thread_updates_has_connections_watcher() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);

        manager
            .connection_initialized(connection_a, ConnectionCapabilities::default())
            .await;
        manager
            .connection_initialized(connection_b, ConnectionCapabilities::default())
            .await;
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, connection_a)
                .await,
            "connection_a should be live"
        );
        let mut has_connections = manager
            .subscribe_to_has_connections(thread_id)
            .await
            .expect("thread should have a has-connections watcher");
        assert!(*has_connections.borrow());

        assert!(
            manager
                .unsubscribe_connection_from_thread(thread_id, connection_a)
                .await
        );
        tokio::time::timeout(Duration::from_secs(1), has_connections.changed())
            .await
            .expect("timed out waiting for no-subscriber update")
            .expect("has-connections watcher should remain open");
        assert!(!*has_connections.borrow());

        assert!(
            manager
                .try_add_connection_to_thread(thread_id, connection_b)
                .await
        );
        tokio::time::timeout(Duration::from_secs(1), has_connections.changed())
            .await
            .expect("timed out waiting for subscriber update")
            .expect("has-connections watcher should remain open");
        assert!(*has_connections.borrow());
        Ok(())
    }

    #[tokio::test]
    async fn wait_for_thread_subscriber_unblocks_after_connection_attaches() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ba62fd70-2ec2-4b1b-9d94-355694332dd2")?;
        let connection = ConnectionId(1);
        manager
            .connection_initialized(connection, ConnectionCapabilities::default())
            .await;

        let wait_for_subscriber = manager.wait_for_thread_subscriber(thread_id);
        let attach_connection = async {
            tokio::task::yield_now().await;
            manager
                .try_add_connection_to_thread(thread_id, connection)
                .await
        };
        let ((), attached) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(wait_for_subscriber, attach_connection)
        })
        .await?;

        assert!(attached);
        Ok(())
    }

    #[tokio::test]
    async fn closed_connection_cannot_be_reintroduced_by_auto_subscribe() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection = ConnectionId(1);

        manager
            .connection_initialized(connection, ConnectionCapabilities::default())
            .await;
        let threads_to_unload = manager.remove_connection(connection).await;
        assert_eq!(threads_to_unload, Vec::<ThreadId>::new());

        assert!(
            !manager
                .try_add_connection_to_thread(thread_id, connection)
                .await
        );
        assert!(!manager.has_subscribers(thread_id).await);
        Ok(())
    }

    #[tokio::test]
    async fn first_attestation_capable_connection_for_thread_only_uses_thread_subscribers()
    -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("dfbd9a95-2f44-470a-8bd8-1cfc04efc243")?;
        let other_thread_id = ThreadId::from_string("6c9a74e4-5e59-479e-90bf-5c5798bb50aa")?;
        let unrelated_supported_connection = ConnectionId(1);
        let earlier_supported_connection = ConnectionId(2);
        let later_supported_connection = ConnectionId(3);
        let unsupported_connection = ConnectionId(4);

        manager
            .connection_initialized(
                unrelated_supported_connection,
                ConnectionCapabilities {
                    request_attestation: true,
                },
            )
            .await;
        manager
            .connection_initialized(
                earlier_supported_connection,
                ConnectionCapabilities {
                    request_attestation: true,
                },
            )
            .await;
        manager
            .connection_initialized(
                later_supported_connection,
                ConnectionCapabilities {
                    request_attestation: true,
                },
            )
            .await;
        manager
            .connection_initialized(unsupported_connection, ConnectionCapabilities::default())
            .await;

        assert!(
            manager
                .try_add_connection_to_thread(other_thread_id, unrelated_supported_connection)
                .await
        );
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, later_supported_connection)
                .await
        );
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, earlier_supported_connection)
                .await
        );
        assert!(
            manager
                .try_add_connection_to_thread(thread_id, unsupported_connection)
                .await
        );

        assert_eq!(
            manager
                .first_attestation_capable_connection_for_thread(thread_id)
                .await,
            Some(earlier_supported_connection)
        );
        assert_eq!(
            manager
                .first_attestation_capable_connection_for_thread(other_thread_id)
                .await,
            Some(unrelated_supported_connection)
        );
        Ok(())
    }
}

#[test]
fn preview_from_rollout_items_prefers_plain_user_messages() {
    use codex_protocol::protocol::USER_MESSAGE_BEGIN;
    use codex_rollout::RolloutItem;
    use core_test_support::responses::user_message_item;

    let items = [
        "# AGENTS.md instructions for project\n\n<INSTRUCTIONS>\n<AGENTS.md contents>\n</INSTRUCTIONS>",
        &format!("<prior context> {USER_MESSAGE_BEGIN}Count to 5"),
    ]
    .map(|text| RolloutItem::ResponseItem(user_message_item(text).into()));

    assert_eq!(super::preview_from_rollout_items(&items), "Count to 5");
}
