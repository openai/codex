use super::*;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::ConfigRequirementsToml;
use crate::exec_policy::ExecPolicyManager;
use crate::features::Feature;
use crate::guardian::GUARDIAN_SUBAGENT_NAME;
use crate::protocol::AskForApproval;
use crate::sandboxing::SandboxPermissions;
use crate::tools::handlers::normalize_and_validate_additional_permissions;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_app_server_protocol::ConfigLayerSource;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::RuleMatch;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn guardian_allows_shell_additional_permissions_requests_past_policy_validation() {
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
    let additional_permissions = PermissionProfile {
        network: Some(NetworkPermissions {
            enabled: Some(true),
        }),
        file_system: None,
        macos: None,
    };
    let normalized = normalize_and_validate_additional_permissions(
        session.features().enabled(Feature::RequestPermissions),
        turn_context_raw.approval_policy.value(),
        SandboxPermissions::WithAdditionalPermissions,
        Some(additional_permissions.clone()),
        &turn_context_raw.cwd,
    )
    .expect("shell additional permissions should pass policy validation");

    assert_eq!(normalized, Some(additional_permissions));
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
