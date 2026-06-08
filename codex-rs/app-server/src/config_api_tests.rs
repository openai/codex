use super::*;
use codex_app_server_protocol::ApprovalsReviewer as ApiApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as ApiAskForApproval;
use codex_app_server_protocol::SandboxMode as ApiSandboxMode;
use codex_config::config_toml::ToolsToml;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::WebSearchToolConfig;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn converts_typed_fields_and_preserves_open_maps() {
    let writable_root =
        AbsolutePathBuf::try_from(PathBuf::from("/workspace")).expect("absolute path");
    let desktop = HashMap::from([(
        "customSetting".to_string(),
        serde_json::json!({"enabled": true}),
    )]);
    let config = ConfigToml {
        model: Some("gpt-test".to_string()),
        approval_policy: Some(AskForApproval::Never),
        approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        sandbox_workspace_write: Some(types::SandboxWorkspaceWrite {
            writable_roots: vec![writable_root],
            network_access: true,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: false,
        }),
        forced_chatgpt_workspace_id: Some(ForcedChatgptWorkspaceIds::Multiple(vec![
            "workspace-1".to_string(),
            "workspace-2".to_string(),
        ])),
        tools: Some(ToolsToml {
            web_search: Some(WebSearchToolConfig::default()),
            experimental_request_user_input: None,
        }),
        analytics: Some(types::AnalyticsConfigToml {
            enabled: Some(false),
        }),
        apps: Some(types::AppsConfigToml {
            default: Some(types::AppsDefaultConfig {
                enabled: true,
                destructive_enabled: false,
                open_world_enabled: true,
            }),
            apps: HashMap::from([(
                "drive".to_string(),
                types::AppConfig {
                    enabled: true,
                    approvals_reviewer: Some(ApprovalsReviewer::User),
                    default_tools_approval_mode: Some(types::AppToolApproval::Prompt),
                    tools: Some(types::AppToolsConfig {
                        tools: HashMap::from([(
                            "files/delete".to_string(),
                            types::AppToolConfig {
                                enabled: Some(false),
                                approval_mode: Some(types::AppToolApproval::Approve),
                            },
                        )]),
                    }),
                    ..Default::default()
                },
            )]),
        }),
        desktop: Some(desktop.clone()),
        ..Default::default()
    };
    let effective = toml::from_str(
        r#"
model = "raw-model-is-not-additional"

[mcp_servers.docs]
command = "docs-server"

[custom_extension]
enabled = true
values = [1, 2, 3]
"#,
    )
    .expect("valid TOML");

    let actual = config_toml_to_api(config, &effective).expect("conversion succeeds");

    assert_eq!(
        actual,
        ApiConfig {
            model: Some("gpt-test".to_string()),
            review_model: None,
            model_context_window: None,
            model_auto_compact_token_limit: None,
            model_auto_compact_token_limit_scope: None,
            model_provider: None,
            approval_policy: Some(ApiAskForApproval::Never),
            approvals_reviewer: Some(ApiApprovalsReviewer::AutoReview),
            sandbox_mode: Some(ApiSandboxMode::WorkspaceWrite),
            sandbox_workspace_write: Some(ApiSandboxWorkspaceWrite {
                writable_roots: vec![PathBuf::from("/workspace")],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: false,
            }),
            forced_chatgpt_workspace_id: Some(ApiForcedChatgptWorkspaceIds::Multiple(vec![
                "workspace-1".to_string(),
                "workspace-2".to_string(),
            ])),
            forced_login_method: None,
            web_search: None,
            tools: Some(ToolsV2 {
                web_search: Some(WebSearchToolConfig::default()),
            }),
            instructions: None,
            developer_instructions: None,
            compact_prompt: None,
            model_reasoning_effort: None,
            model_reasoning_summary: None,
            model_verbosity: None,
            service_tier: None,
            analytics: Some(AnalyticsConfig {
                enabled: Some(false),
                additional: HashMap::new(),
            }),
            apps: Some(AppsConfig {
                default: Some(AppsDefaultConfig {
                    enabled: true,
                    destructive_enabled: false,
                    open_world_enabled: true,
                }),
                apps: HashMap::from([(
                    "drive".to_string(),
                    AppConfig {
                        enabled: true,
                        approvals_reviewer: Some(ApiApprovalsReviewer::User),
                        destructive_enabled: None,
                        open_world_enabled: None,
                        default_tools_approval_mode: Some(AppToolApproval::Prompt),
                        default_tools_enabled: None,
                        tools: Some(AppToolsConfig {
                            tools: HashMap::from([(
                                "files/delete".to_string(),
                                AppToolConfig {
                                    enabled: Some(false),
                                    approval_mode: Some(AppToolApproval::Approve),
                                },
                            )]),
                        }),
                    },
                )]),
            }),
            desktop: Some(desktop),
            additional: HashMap::from([
                (
                    "mcp_servers".to_string(),
                    serde_json::json!({"docs": {"command": "docs-server"}}),
                ),
                (
                    "custom_extension".to_string(),
                    serde_json::json!({"enabled": true, "values": [1, 2, 3]}),
                ),
            ]),
        }
    );
}
