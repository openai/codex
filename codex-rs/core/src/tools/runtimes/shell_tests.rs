use super::*;
use crate::config::PermissionProfileSnapshot;
use crate::environment_selection::EnvironmentConfigOrigin;
use crate::tools::approvals::ApprovalCacheKey;
use codex_exec_server::Environment;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[tokio::test]
async fn approval_key_uses_path_uri_and_includes_environment_id() {
    let cwd = AbsolutePathBuf::try_from(std::env::current_dir().expect("read current dir"))
        .expect("current dir is absolute");
    let mut request = ShellRequest {
        command: vec!["echo".to_string(), "hello".to_string()],
        turn_environment: TurnEnvironment::new(
            TurnEnvironmentSelection {
                environment_id: "remote".to_string(),
                cwd: PathUri::from_abs_path(&cwd),
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::Ready(EnvironmentConfig {
                    allow_login_shell: true,
                    permission_profile: PermissionProfileSnapshot::legacy(
                        PermissionProfile::read_only(),
                    ),
                    shell_environment_policy: Default::default(),
                    exec_policy: None,
                    mcp_policy: None,
                    network_policy: None,
                    selected_capability_roots: Vec::new(),
                }),
            },
            EnvironmentConfigOrigin::Thread,
            Arc::new(Environment::default_for_tests()),
            /*shell*/ None,
        ),
        shell_type: None,
        hook_command: "echo hello".to_string(),
        cwd: cwd.clone(),
        timeout_ms: None,
        cancellation_token: CancellationToken::new(),
        env: HashMap::new(),
        explicit_env_overrides: HashMap::new(),
        network: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions: None,
        #[cfg(unix)]
        additional_permissions_preapproved: false,
        justification: None,
        exec_approval_requirement: ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    };
    let runtime = ShellRuntime::for_shell_command(ShellRuntimeBackend::ShellCommandClassic);
    let original_key = runtime
        .approval_action(&request, "call-1")
        .expect("build approval action")
        .cache_keys();
    assert_eq!(
        original_key,
        vec![ApprovalCacheKey::Shell(ApprovalKey {
            environment_id: "remote".to_string(),
            command: request.command.clone(),
            cwd: PathUri::from_abs_path(&cwd),
            sandbox_permissions: request.sandbox_permissions,
            additional_permissions: request.additional_permissions.clone(),
        })]
    );
    request.turn_environment.selection.environment_id = "other".to_string();
    let other_key = runtime
        .approval_action(&request, "call-1")
        .expect("build approval action")
        .cache_keys();

    assert_ne!(original_key, other_key);
}
