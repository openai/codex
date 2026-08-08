use super::AllowPrefixRules;
use super::ExecPolicyManager;
use crate::exec_policy::ExecApprovalRequest;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;
use codex_execpolicy::Decision;
use codex_execpolicy::MatchOptions;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use std::sync::Arc;

fn policy_with_broad_allow_prefix() -> (Arc<Policy>, String) {
    let program_name = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let program_path = std::env::temp_dir()
        .join(program_name)
        .to_string_lossy()
        .into_owned();
    let escaped_program_path = program_path.replace('\\', "\\\\");
    let source = format!(
        r#"
host_executable(name="cargo", paths=["{escaped_program_path}"])
prefix_rule(pattern=["cargo"], decision="allow")
prefix_rule(pattern=["cargo", "publish"], decision="prompt")
prefix_rule(pattern=["rm"], decision="forbidden")
network_rule(host="example.com", protocol="https", decision="allow")
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &source).expect("parse policy");
    (Arc::new(parser.build()), program_path)
}

#[test]
fn cyber_policy_filters_allow_prefixes_but_preserves_restrictive_and_network_rules() {
    let (policy, program_path) = policy_with_broad_allow_prefix();
    let manager = ExecPolicyManager::new(Arc::clone(&policy));

    let standard_policy = manager.current_for_prefix_rules(AllowPrefixRules::Honor);
    assert!(Arc::ptr_eq(&standard_policy, &policy));

    let cyber_policy = manager.current_for_prefix_rules(AllowPrefixRules::IgnoreForCyberModel);
    assert!(cyber_policy.get_allowed_prefixes().is_empty());
    assert_eq!(cyber_policy.network_rules(), policy.network_rules());
    assert_eq!(cyber_policy.host_executables(), policy.host_executables());

    let cargo_install = vec!["cargo".to_string(), "install".to_string()];
    assert_eq!(
        cyber_policy
            .check(&cargo_install, &|_| Decision::Prompt)
            .decision,
        Decision::Prompt,
    );

    let cargo_publish = vec!["cargo".to_string(), "publish".to_string()];
    assert_eq!(
        cyber_policy
            .check(&cargo_publish, &|_| Decision::Allow)
            .decision,
        Decision::Prompt,
    );

    let resolved_cargo_publish = vec![program_path, "publish".to_string()];
    assert_eq!(
        cyber_policy
            .check_with_options(
                &resolved_cargo_publish,
                &|_| Decision::Allow,
                &MatchOptions {
                    resolve_host_executables: true,
                },
            )
            .decision,
        Decision::Prompt,
    );

    let forbidden_command = vec!["rm".to_string(), "target".to_string()];
    assert_eq!(
        cyber_policy
            .check(&forbidden_command, &|_| Decision::Allow)
            .decision,
        Decision::Forbidden,
    );
}

#[tokio::test]
async fn cyber_policy_requires_approval_for_broad_wrapped_and_resolved_prefixes() {
    let (policy, program_path) = policy_with_broad_allow_prefix();
    let manager = ExecPolicyManager::new(policy);
    let commands = [
        vec![
            "cargo".to_string(),
            "install".to_string(),
            "example".to_string(),
        ],
        vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cargo install example".to_string(),
        ],
        vec![program_path, "install".to_string(), "example".to_string()],
    ];

    for command in commands {
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                permission_profile: PermissionProfile::read_only(),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::RequireEscalated,
                prefix_rule: Some(vec!["cargo".to_string(), "install".to_string()]),
                allow_prefix_rules: AllowPrefixRules::IgnoreForCyberModel,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: None,
            },
            "command {command:?} must not inherit a saved prefix approval",
        );
    }
}

#[tokio::test]
async fn cyber_policy_keeps_heuristically_safe_commands_inside_the_sandbox() {
    let mut policy = Policy::empty();
    policy
        .add_prefix_rule(&["echo".to_string()], Decision::Allow)
        .expect("add broad allow prefix");
    let manager = ExecPolicyManager::new(Arc::new(policy));
    let command = vec!["echo".to_string(), "hello".to_string()];

    let cyber_requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
            allow_prefix_rules: AllowPrefixRules::IgnoreForCyberModel,
        })
        .await;
    assert_eq!(
        cyber_requirement,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    );

    let standard_requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
            allow_prefix_rules: AllowPrefixRules::Honor,
        })
        .await;
    assert_eq!(
        standard_requirement,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );
}
