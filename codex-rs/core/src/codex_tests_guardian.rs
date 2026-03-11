use super::*;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::ConfigRequirementsToml;
use crate::exec::ExecParams;
use crate::exec_policy::ExecPolicyManager;
use crate::features::Feature;
use crate::guardian::GUARDIAN_SUBAGENT_NAME;
use crate::protocol::AskForApproval;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::FunctionToolOutput;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_app_server_protocol::ConfigLayerSource;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::RuleMatch;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::codex_linux_sandbox_exe_or_skip;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

fn expect_text_output(output: &FunctionToolOutput) -> String {
    function_call_output_content_items_to_text(&output.body).unwrap_or_default()
}

#[tokio::test]
async fn guardian_allows_shell_additional_permissions_requests_past_policy_validation() {
    let server = start_mock_server().await;
    let _request_log = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-guardian"),
            ev_assistant_message(
                "msg-guardian",
                &serde_json::json!({
                    "risk_level": "low",
                    "risk_score": 5,
                    "rationale": "The request only widens permissions for a benign local echo command.",
                    "evidence": [{
                        "message": "The planned command is an `echo hi` smoke test.",
                        "why": "This is low-risk and does not attempt destructive or exfiltrating behavior.",
                    }],
                })
                .to_string(),
            ),
            ev_completed("resp-guardian"),
        ]),
    )
    .await;

    let (mut session, mut turn_context_raw) = make_session_and_context().await;
    turn_context_raw.codex_linux_sandbox_exe = codex_linux_sandbox_exe_or_skip!();
    turn_context_raw
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("test setup should allow updating approval policy");
    turn_context_raw
        .features
        .enable(Feature::GuardianApproval)
        .expect("test setup should allow enabling guardian approvals");
    session
        .features
        .enable(Feature::RequestPermissions)
        .expect("test setup should allow enabling request permissions");
    turn_context_raw
        .sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");
    // This test is about request-permissions validation, not managed sandbox
    // policy enforcement. Widen the derived sandbox policies directly so the
    // command runs without depending on a platform sandbox binary.
    turn_context_raw.file_system_sandbox_policy =
        FileSystemSandboxPolicy::from(turn_context_raw.sandbox_policy.get());
    turn_context_raw.network_sandbox_policy =
        NetworkSandboxPolicy::from(turn_context_raw.sandbox_policy.get());
    let mut config = (*turn_context_raw.config).clone();
    config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    let config = Arc::new(config);
    let models_manager = Arc::new(crate::test_support::models_manager_with_provider(
        config.codex_home.clone(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    session.services.models_manager = models_manager;
    turn_context_raw.config = Arc::clone(&config);
    turn_context_raw.provider = config.model_provider.clone();
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context_raw);
    let expiration_ms: u64 = if cfg!(windows) { 2_500 } else { 1_000 };

    let params = ExecParams {
        command: if cfg!(windows) {
            vec![
                "cmd.exe".to_string(),
                "/Q".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "echo hi".to_string(),
            ]
        } else {
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo hi".to_string(),
            ]
        },
        cwd: turn_context.cwd.clone(),
        expiration: expiration_ms.into(),
        env: HashMap::new(),
        network: None,
        sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
        windows_sandbox_level: turn_context.windows_sandbox_level,
        justification: Some("test".to_string()),
        arg0: None,
    };

    let handler = ShellHandler;
    let resp = handler
        .handle(ToolInvocation {
            session: Arc::clone(&session),
            turn: Arc::clone(&turn_context),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "test-call".to_string(),
            tool_name: "shell".to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::json!({
                    "command": params.command.clone(),
                    "workdir": Some(turn_context.cwd.to_string_lossy().to_string()),
                    "timeout_ms": params.expiration.timeout_ms(),
                    "sandbox_permissions": params.sandbox_permissions,
                    "additional_permissions": PermissionProfile {
                        network: Some(NetworkPermissions {
                            enabled: Some(true),
                        }),
                        file_system: None,
                        macos: None,
                    },
                    "justification": params.justification.clone(),
                })
                .to_string(),
            },
        })
        .await;

    let output = expect_text_output(&resp.expect("expected Ok result"));

    #[derive(Deserialize, PartialEq, Eq, Debug)]
    struct ResponseExecMetadata {
        exit_code: i32,
    }

    #[derive(Deserialize)]
    struct ResponseExecOutput {
        output: String,
        metadata: ResponseExecMetadata,
    }

    let exec_output: ResponseExecOutput =
        serde_json::from_str(&output).expect("valid exec output json");

    assert_eq!(exec_output.metadata, ResponseExecMetadata { exit_code: 0 });
    assert!(exec_output.output.contains("hi"));
}

#[tokio::test]
async fn guardian_allows_unified_exec_additional_permissions_requests_past_policy_validation() {
    let (mut session, mut turn_context_raw) = make_session_and_context().await;
    turn_context_raw
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("test setup should allow updating approval policy");
    turn_context_raw
        .features
        .enable(Feature::GuardianApproval)
        .expect("test setup should allow enabling guardian approvals");
    session
        .features
        .enable(Feature::RequestPermissions)
        .expect("test setup should allow enabling request permissions");
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context_raw);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));

    let handler = UnifiedExecHandler;
    let resp = handler
        .handle(ToolInvocation {
            session: Arc::clone(&session),
            turn: Arc::clone(&turn_context),
            tracker: Arc::clone(&tracker),
            call_id: "exec-call".to_string(),
            tool_name: "exec_command".to_string(),
            payload: ToolPayload::Function {
                arguments: serde_json::json!({
                    "cmd": "echo hi",
                    "sandbox_permissions": SandboxPermissions::WithAdditionalPermissions,
                    "justification": "need additional sandbox permissions",
                })
                .to_string(),
            },
        })
        .await;

    let Err(FunctionCallError::RespondToModel(output)) = resp else {
        panic!("expected validation error result");
    };

    assert_eq!(
        output,
        "missing `additional_permissions`; provide at least one of `network`, `file_system`, or `macos` when using `with_additional_permissions`"
    );
}

#[tokio::test]
async fn guardian_subagent_does_not_inherit_parent_exec_policy_rules() {
    let codex_home = tempdir().expect("create codex home");
    let project_dir = tempdir().expect("create project dir");
    let rules_dir = project_dir.path().join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(
        rules_dir.join("deny.rules"),
        r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
    )
    .expect("write policy file");

    let mut config = build_test_config(codex_home.path()).await;
    config.cwd = project_dir.path().to_path_buf();
    config.config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::Project {
                dot_codex_folder: AbsolutePathBuf::from_absolute_path(project_dir.path())
                    .expect("absolute project path"),
            },
            toml::Value::Table(Default::default()),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");

    let command = [vec!["rm".to_string()]];
    let parent_exec_policy = ExecPolicyManager::load(&config.config_layer_stack)
        .await
        .expect("load parent exec policy");
    assert_eq!(
        parent_exec_policy
            .current()
            .check_multiple(command.iter(), &|_| Decision::Allow),
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec!["rm".to_string()],
                decision: Decision::Forbidden,
                resolved_program: None,
                justification: None,
            }],
        }
    );

    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let models_manager = Arc::new(ModelsManager::new(
        config.codex_home.clone(),
        auth_manager.clone(),
        None,
        CollaborationModesConfig::default(),
    ));
    let plugins_manager = Arc::new(PluginsManager::new(config.codex_home.clone()));
    let skills_manager = Arc::new(SkillsManager::new(
        config.codex_home.clone(),
        Arc::clone(&plugins_manager),
        true,
    ));
    let mcp_manager = Arc::new(McpManager::new(Arc::clone(&plugins_manager)));
    let file_watcher = Arc::new(FileWatcher::noop());

    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        auth_manager,
        models_manager,
        skills_manager,
        plugins_manager,
        mcp_manager,
        file_watcher,
        InitialHistory::New,
        SessionSource::SubAgent(SubAgentSource::Other(GUARDIAN_SUBAGENT_NAME.to_string())),
        AgentControl::default(),
        None,
        false,
        Vec::new(),
        false,
        None,
        None,
    )
    .await
    .expect("spawn guardian subagent");

    assert_eq!(
        codex
            .session
            .services
            .exec_policy
            .current()
            .check_multiple(command.iter(), &|_| Decision::Allow),
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: vec!["rm".to_string()],
                decision: Decision::Allow,
            }],
        }
    );

    drop(codex);
}
