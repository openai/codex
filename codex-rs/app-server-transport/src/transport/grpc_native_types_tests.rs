use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fmt::Debug;

fn assert_round_trip<T>(value: T)
where
    T: NativeProto + Clone + Debug + PartialEq,
{
    let encoded = encode_native(value.clone()).expect("encode native protobuf");
    let decoded = decode_native::<T>(encoded).expect("decode native protobuf");
    assert_eq!(decoded, value);
}

#[test]
fn account_login_round_trips_without_serde() {
    assert_round_trip(LoginAccountParams::ChatgptAuthTokens {
        access_token: "access-token".to_string(),
        chatgpt_account_id: "workspace-1".to_string(),
        chatgpt_plan_type: Some("pro".to_string()),
    });
    assert_round_trip(LoginAccountResponse::ChatgptDeviceCode {
        login_id: "login-1".to_string(),
        verification_url: "https://example.test/device".to_string(),
        user_code: "ABCD-EFGH".to_string(),
    });
}

#[test]
fn command_exec_round_trips_native_sandbox_and_presence() {
    assert_round_trip(CommandExecParams {
        command: vec![
            "sh".to_string(),
            "-lc".to_string(),
            "printf test".to_string(),
        ],
        process_id: Some("process-1".to_string()),
        tty: true,
        stream_stdin: true,
        stream_stdout_stderr: true,
        output_bytes_cap: Some(4096),
        disable_output_cap: false,
        disable_timeout: false,
        timeout_ms: Some(12_000),
        cwd: Some(PathBuf::from("/tmp")),
        env: Some(HashMap::from([
            ("SET".to_string(), Some("value".to_string())),
            ("UNSET".to_string(), None),
        ])),
        size: Some(CommandExecTerminalSize {
            rows: 40,
            cols: 120,
        }),
        sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
            writable_roots: vec!["/tmp".to_string().try_into().expect("absolute path")],
            network_access: true,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        }),
        permission_profile: Some("workspace-write".to_string()),
    });
}

#[test]
fn process_spawn_preserves_explicit_null_limits() {
    assert_round_trip(ProcessSpawnParams {
        command: vec!["echo".to_string(), "test".to_string()],
        process_handle: "process-1".to_string(),
        cwd: "/tmp".to_string().try_into().expect("absolute path"),
        tty: false,
        stream_stdin: true,
        stream_stdout_stderr: true,
        output_bytes_cap: Some(None),
        timeout_ms: Some(Some(1_500)),
        env: Some(HashMap::from([("REMOVE_ME".to_string(), None)])),
        size: None,
    });
}

#[test]
fn thread_goal_round_trips_optional_updates() {
    assert_round_trip(ThreadGoalSetParams {
        thread_id: "thread-1".to_string(),
        objective: Some("ship it".to_string()),
        status: Some(ThreadGoalStatus::Active),
        token_budget: Some(None),
    });
    assert_round_trip(ThreadGoalSetResponse {
        goal: ThreadGoal {
            thread_id: "thread-1".to_string(),
            objective: "ship it".to_string(),
            status: ThreadGoalStatus::Complete,
            token_budget: Some(2_000),
            tokens_used: 1_500,
            time_used_seconds: 42,
            created_at: 10,
            updated_at: 20,
        },
    });
}

#[test]
fn filesystem_paths_and_stream_enums_round_trip() {
    assert_round_trip(FsChangedNotification {
        watch_id: "watch-1".to_string(),
        changed_paths: vec![
            "/tmp/a".to_string().try_into().expect("absolute path"),
            "/tmp/b".to_string().try_into().expect("absolute path"),
        ],
    });
    assert_round_trip(CommandExecOutputDeltaNotification {
        process_id: "process-1".to_string(),
        stream: CommandExecOutputStream::Stderr,
        delta_base64: "dGVzdA==".to_string(),
        cap_reached: true,
    });
}

#[test]
fn generated_turn_start_preserves_typed_and_open_fields() {
    assert_round_trip(TurnStartParams {
        thread_id: "thread-1".to_string(),
        input: vec![UserInput::Text {
            text: "hello".to_string(),
            text_elements: Vec::new(),
        }],
        approval_policy: Some(AskForApproval::Granular {
            sandbox_approval: true,
            rules: false,
            skill_approval: false,
            request_permissions: false,
            mcp_elicitations: true,
        }),
        output_schema: Some(json!({
            "type": "object",
            "properties": {
                "answer": {"type": "string"}
            }
        })),
        responsesapi_client_metadata: Some(HashMap::from([(
            "surface".to_string(),
            "grpc".to_string(),
        )])),
        ..Default::default()
    });
    assert_round_trip(TurnStartParams {
        thread_id: "thread-1".to_string(),
        service_tier: Some(None),
        ..Default::default()
    });
    assert_round_trip(ThreadMetadataUpdateParams {
        thread_id: "thread-1".to_string(),
        git_info: Some(ThreadMetadataGitInfoUpdateParams {
            branch: Some(None),
            sha: Some(Some("abc123".to_string())),
            origin_url: None,
        }),
    });
}

#[test]
fn generated_thread_item_union_round_trips() {
    assert_round_trip(ItemStartedNotification {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        started_at_ms: 1234,
        item: ThreadItem::AgentMessage {
            id: "item-1".to_string(),
            text: "done".to_string(),
            phase: Some(codex_protocol::models::MessagePhase::FinalAnswer),
            memory_citation: None,
        },
    });
}

#[test]
fn generated_legacy_union_round_trips() {
    assert_round_trip(GetConversationSummaryParams::ThreadId {
        conversation_id: codex_protocol::ThreadId::try_from("01976f64-5377-7f23-a7ca-8297b464c200")
            .expect("thread id"),
    });
}
