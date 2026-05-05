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
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_config::CloudRequirementsLoader;
    use codex_config::LoaderOverrides;
    use codex_config::SessionThreadConfig;
    use codex_config::StaticThreadConfigLoader;
    use codex_config::ThreadConfigSource;
    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::WireApi;
    use codex_protocol::ThreadId;
    use codex_protocol::openai_models::ReasoningEffort;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use codex_state::ThreadMetadataBuilder;
    use codex_thread_store::StoredThread;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn validate_dynamic_tools_rejects_unsupported_input_schema() {
        let tools = vec![ApiDynamicToolSpec {
            namespace: None,
            name: "my_tool".to_string(),
            description: "test".to_string(),
            input_schema: json!({"type": "null"}),
            defer_loading: false,
        }];
        let err = validate_dynamic_tools(&tools).expect_err("invalid schema");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_accepts_sanitizable_input_schema() {
        let tools = vec![ApiDynamicToolSpec {
            namespace: None,
            name: "my_tool".to_string(),
            description: "test".to_string(),
            // Missing `type` is common; core sanitizes these to a supported schema.
            input_schema: json!({"properties": {}}),
            defer_loading: false,
        }];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_accepts_nullable_field_schema() {
        let tools = vec![ApiDynamicToolSpec {
            namespace: None,
            name: "my_tool".to_string(),
            description: "test".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": ["string", "null"]}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            defer_loading: false,
        }];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_accepts_same_name_in_different_namespaces() {
        let tools = vec![
            ApiDynamicToolSpec {
                namespace: Some("codex_app".to_string()),
                name: "my_tool".to_string(),
                description: "test".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                defer_loading: true,
            },
            ApiDynamicToolSpec {
                namespace: Some("other_app".to_string()),
                name: "my_tool".to_string(),
                description: "test".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                defer_loading: true,
            },
        ];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn validate_dynamic_tools_rejects_duplicate_name_in_same_namespace() {
        let tools = vec![
            ApiDynamicToolSpec {
                namespace: Some("codex_app".to_string()),
                name: "my_tool".to_string(),
                description: "test".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                defer_loading: true,
            },
            ApiDynamicToolSpec {
                namespace: Some("codex_app".to_string()),
                name: "my_tool".to_string(),
                description: "test".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                defer_loading: true,
            },
        ];
        let err = validate_dynamic_tools(&tools).expect_err("duplicate name");
        assert!(err.contains("codex_app"), "unexpected error: {err}");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
    }

    #[test]
    fn thread_turns_list_merges_in_progress_active_turn_before_agent_status_running() {
        let persisted_items = vec![RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "persisted".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        ))];
        let active_turn = Turn {
            id: "live-turn".to_string(),
            items: vec![ThreadItem::UserMessage {
                id: "live-user-message".to_string(),
                content: vec![V2UserInput::Text {
                    text: "live".to_string(),
                    text_elements: Vec::new(),
                }],
            }],
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
        let tools = vec![ApiDynamicToolSpec {
            namespace: Some("".to_string()),
            name: "my_tool".to_string(),
            description: "test".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            defer_loading: false,
        }];
        let err = validate_dynamic_tools(&tools).expect_err("empty namespace");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
        assert!(err.contains("namespace"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_rejects_reserved_namespace() {
        let tools = vec![ApiDynamicToolSpec {
            namespace: Some("mcp__server__".to_string()),
            name: "my_tool".to_string(),
            description: "test".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            defer_loading: false,
        }];
        let err = validate_dynamic_tools(&tools).expect_err("reserved namespace");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
        assert!(err.contains("reserved"), "unexpected error: {err}");
    }

    #[test]
    fn summary_from_stored_thread_preserves_millisecond_precision() {
        let created_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:05.678Z").expect("valid timestamp");
        let updated_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:06.789Z").expect("valid timestamp");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread");
        let stored_thread = StoredThread {
            thread_id,
            rollout_path: Some(PathBuf::from("/tmp/thread.jsonl")),
            forked_from_id: None,
            preview: "preview".to_string(),
            name: None,
            model_provider: "openai".to_string(),
            model: None,
            reasoning_effort: None,
            created_at: created_at.with_timezone(&Utc),
            updated_at: updated_at.with_timezone(&Utc),
            archived_at: None,
            cwd: PathBuf::from("/tmp"),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::Cli,
            thread_source: Some(codex_protocol::protocol::ThreadSource::User),
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            git_info: None,
            approval_mode: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            token_usage: None,
            first_user_message: Some("first user message".to_string()),
            history: None,
        };

        let summary =
            summary_from_stored_thread(stored_thread, "fallback").expect("summary should exist");

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
    fn requested_permissions_trust_project_uses_permission_profile_intent() {
        let cwd = test_path_buf("/tmp/project").abs();
        let full_access_profile = codex_protocol::models::PermissionProfile::Disabled;
        let workspace_write_profile = codex_protocol::models::PermissionProfile::workspace_write();
        let read_only_profile = codex_protocol::models::PermissionProfile::read_only();
        let split_write_profile =
            codex_protocol::models::PermissionProfile::from_runtime_permissions(
                &FileSystemSandboxPolicy::restricted(vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Path { path: cwd.clone() },
                        access: FileSystemAccessMode::Write,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::GlobPattern {
                            pattern: "/tmp/project/**/*.env".to_string(),
                        },
                        access: FileSystemAccessMode::None,
                    },
                ]),
                NetworkSandboxPolicy::Restricted,
            );

        assert!(requested_permissions_trust_project(
            &ConfigOverrides {
                permission_profile: Some(full_access_profile),
                ..Default::default()
            },
            cwd.as_path()
        ));
        assert!(requested_permissions_trust_project(
            &ConfigOverrides {
                permission_profile: Some(workspace_write_profile),
                ..Default::default()
            },
            cwd.as_path()
        ));
        assert!(requested_permissions_trust_project(
            &ConfigOverrides {
                permission_profile: Some(split_write_profile),
                ..Default::default()
            },
            cwd.as_path()
        ));
        assert!(requested_permissions_trust_project(
            &ConfigOverrides {
                default_permissions: Some(":workspace".to_string()),
                ..Default::default()
            },
            cwd.as_path()
        ));
        assert!(requested_permissions_trust_project(
            &ConfigOverrides {
                default_permissions: Some(":danger-no-sandbox".to_string()),
                ..Default::default()
            },
            cwd.as_path()
        ));
        assert!(!requested_permissions_trust_project(
            &ConfigOverrides {
                permission_profile: Some(read_only_profile),
                ..Default::default()
            },
            cwd.as_path()
        ));
        assert!(!requested_permissions_trust_project(
            &ConfigOverrides {
                default_permissions: Some(":read-only".to_string()),
                ..Default::default()
            },
            cwd.as_path()
        ));
    }

    #[test]
    fn config_load_error_marks_cloud_requirements_failures_for_relogin() {
        let err = std::io::Error::other(CloudRequirementsLoadError::new(
            CloudRequirementsLoadErrorCode::Auth,
            Some(401),
            "Your authentication session could not be refreshed automatically. Please log out and sign in again.",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudRequirements",
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
    fn config_load_error_leaves_non_cloud_requirements_failures_unmarked() {
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
    fn config_load_error_marks_non_auth_cloud_requirements_failures_without_relogin() {
        let err = std::io::Error::other(CloudRequirementsLoadError::new(
            CloudRequirementsLoadErrorCode::RequestFailed,
            /*status_code*/ None,
            "Failed to load cloud requirements (workspace-managed policies).",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudRequirements",
                "errorCode": "RequestFailed",
                "detail": "Failed to load cloud requirements (workspace-managed policies).",
            }))
        );
    }

    #[tokio::test]
    async fn derive_config_from_params_uses_session_thread_config_model_provider() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let session_provider = ModelProviderInfo {
            name: "session".to_string(),
            base_url: Some("http://127.0.0.1:8061/api/codex".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
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
        };
        let config_manager = ConfigManager::new(
            temp_dir.path().to_path_buf(),
            Vec::new(),
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
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
            service_tier: Some(Some(codex_protocol::config_types::ServiceTier::Fast)),
            cwd: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox: None,
            permissions: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
            personality: None,
            exclude_turns: false,
            persist_extended_history: false,
        };
        let config_snapshot = ThreadConfigSnapshot {
            model: "gpt-5".to_string(),
            model_provider_id: "openai".to_string(),
            service_tier: Some(codex_protocol::config_types::ServiceTier::Flex),
            approval_policy: codex_protocol::protocol::AskForApproval::OnRequest,
            approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer::User,
            permission_profile: codex_protocol::models::PermissionProfile::Disabled,
            active_permission_profile: None,
            cwd,
            ephemeral: false,
            reasoning_effort: None,
            personality: None,
            session_source: SessionSource::Cli,
            thread_source: None,
        };

        assert_eq!(
            collect_resume_override_mismatches(&request, &config_snapshot),
            vec!["service_tier requested=Some(Fast) active=Some(Flex)".to_string()]
        );
    }

    fn test_thread_metadata(
        model: Option<&str>,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Result<ThreadMetadata> {
        let thread_id = ThreadId::from_string("3f941c35-29b3-493b-b0a4-e25800d9aeb0")?;
        let mut builder = ThreadMetadataBuilder::new(
            thread_id,
            PathBuf::from("/tmp/rollout.jsonl"),
            Utc::now(),
            codex_protocol::protocol::SessionSource::default(),
        );
        builder.model_provider = Some("mock_provider".to_string());
        let mut metadata = builder.build("mock_provider");
        metadata.model = model.map(ToString::to_string);
        metadata.reasoning_effort = reasoning_effort;
        Ok(metadata)
    }

    #[test]
    fn summary_from_thread_metadata_formats_protocol_timestamps_as_seconds() -> Result<()> {
        let mut metadata =
            test_thread_metadata(/*model*/ None, /*reasoning_effort*/ None)?;
        metadata.created_at =
            DateTime::parse_from_rfc3339("2025-09-05T16:53:11.123Z")?.with_timezone(&Utc);
        metadata.updated_at =
            DateTime::parse_from_rfc3339("2025-09-05T16:53:12.456Z")?.with_timezone(&Utc);

        let summary = summary_from_thread_metadata(&metadata);

        assert_eq!(summary.timestamp, Some("2025-09-05T16:53:11Z".to_string()));
        assert_eq!(summary.updated_at, Some("2025-09-05T16:53:12Z".to_string()));
        Ok(())
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
        assert_eq!(typesafe_overrides.model_provider, None);
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
    fn merge_persisted_resume_metadata_skips_persisted_values_when_model_overridden() -> Result<()>
    {
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
        assert_eq!(typesafe_overrides.model_provider, None);
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
    fn merge_persisted_resume_metadata_skips_persisted_values_when_reasoning_effort_overridden()
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
        assert_eq!(typesafe_overrides.model_provider, None);
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
    async fn read_summary_from_rollout_returns_empty_preview_when_no_user_message() -> Result<()> {
        use codex_protocol::protocol::RolloutItem;
        use codex_protocol::protocol::RolloutLine;
        use codex_protocol::protocol::SessionMetaLine;
        use std::fs;
        use std::fs::FileTimes;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            id: conversation_id,
            timestamp: timestamp.clone(),
            model_provider: None,
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp: timestamp.clone(),
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta.clone(),
                git: None,
            }),
        };

        fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;
        let parsed = chrono::DateTime::parse_from_rfc3339(&timestamp)?.with_timezone(&Utc);
        let times = FileTimes::new().set_modified(parsed.into());
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)?
            .set_times(times)?;

        let summary = read_summary_from_rollout(path.as_path(), "fallback").await?;

        let expected = ConversationSummary {
            conversation_id,
            timestamp: Some(timestamp.clone()),
            updated_at: Some(timestamp),
            path: path.clone(),
            preview: String::new(),
            model_provider: "fallback".to_string(),
            cwd: PathBuf::new(),
            cli_version: String::new(),
            source: SessionSource::VSCode,
            git_info: None,
        };

        assert_eq!(summary, expected);
        Ok(())
    }

    #[tokio::test]
    async fn read_summary_from_rollout_preserves_agent_nickname() -> Result<()> {
        use codex_protocol::protocol::RolloutItem;
        use codex_protocol::protocol::RolloutLine;
        use codex_protocol::protocol::SessionMetaLine;
        use std::fs;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let parent_thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            id: conversation_id,
            timestamp: timestamp.clone(),
            source: SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }),
            thread_source: Some(codex_protocol::protocol::ThreadSource::Subagent),
            agent_nickname: Some("atlas".to_string()),
            agent_role: Some("explorer".to_string()),
            model_provider: Some("test-provider".to_string()),
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp,
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta,
                git: None,
            }),
        };
        fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;

        let summary = read_summary_from_rollout(path.as_path(), "fallback").await?;
        let fallback_cwd = AbsolutePathBuf::from_absolute_path("/")?;
        let thread = summary_to_thread(summary, &fallback_cwd);

        assert_eq!(thread.agent_nickname, Some("atlas".to_string()));
        assert_eq!(thread.agent_role, Some("explorer".to_string()));
        assert_eq!(thread.thread_source, None);
        Ok(())
    }

    #[tokio::test]
    async fn read_summary_from_rollout_preserves_forked_from_id() -> Result<()> {
        use codex_protocol::protocol::RolloutItem;
        use codex_protocol::protocol::RolloutLine;
        use codex_protocol::protocol::SessionMetaLine;
        use std::fs;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let forked_from_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            id: conversation_id,
            forked_from_id: Some(forked_from_id),
            timestamp: timestamp.clone(),
            model_provider: Some("test-provider".to_string()),
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp,
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta,
                git: None,
            }),
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

    #[test]
    fn summary_from_state_db_metadata_preserves_agent_nickname() -> Result<()> {
        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let source =
            serde_json::to_string(&SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }))?;

        let summary = summary_from_state_db_metadata(
            conversation_id,
            PathBuf::from("/tmp/rollout.jsonl"),
            Some("hi".to_string()),
            "2025-09-05T16:53:11Z".to_string(),
            "2025-09-05T16:53:12Z".to_string(),
            "test-provider".to_string(),
            PathBuf::from("/"),
            "0.0.0".to_string(),
            source,
            Some(codex_protocol::protocol::ThreadSource::Subagent),
            Some("atlas".to_string()),
            Some("explorer".to_string()),
            /*git_sha*/ None,
            /*git_branch*/ None,
            /*git_origin_url*/ None,
        );

        let fallback_cwd = AbsolutePathBuf::from_absolute_path("/")?;
        let thread = summary_to_thread(summary, &fallback_cwd);

        assert_eq!(thread.agent_nickname, Some("atlas".to_string()));
        assert_eq!(thread.agent_role, Some("explorer".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn removing_thread_state_clears_listener_and_active_turn_history() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection = ConnectionId(1);
        let (cancel_tx, cancel_rx) = oneshot::channel();

        manager.connection_initialized(connection).await;
        manager
            .try_ensure_connection_subscribed(
                thread_id, connection, /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection should be live");
        {
            let state = manager.thread_state(thread_id).await;
            let mut state = state.lock().await;
            state.cancel_tx = Some(cancel_tx);
            state.track_current_turn_event(
                "turn-1",
                &EventMsg::TurnStarted(codex_protocol::protocol::TurnStartedEvent {
                    turn_id: "turn-1".to_string(),
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

        manager.connection_initialized(connection_a).await;
        manager.connection_initialized(connection_b).await;
        manager
            .try_ensure_connection_subscribed(
                thread_id,
                connection_a,
                /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection_a should be live");
        manager
            .try_ensure_connection_subscribed(
                thread_id,
                connection_b,
                /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection_b should be live");
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

        manager.connection_initialized(connection_a).await;
        manager.connection_initialized(connection_b).await;
        manager
            .try_ensure_connection_subscribed(
                thread_id,
                connection_a,
                /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection_a should be live");
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
    async fn closed_connection_cannot_be_reintroduced_by_auto_subscribe() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection = ConnectionId(1);

        manager.connection_initialized(connection).await;
        let threads_to_unload = manager.remove_connection(connection).await;
        assert_eq!(threads_to_unload, Vec::<ThreadId>::new());

        assert!(
            manager
                .try_ensure_connection_subscribed(
                    thread_id, connection, /*experimental_raw_events*/ false
                )
                .await
                .is_none()
        );
        assert!(!manager.has_subscribers(thread_id).await);
        Ok(())
    }
}
