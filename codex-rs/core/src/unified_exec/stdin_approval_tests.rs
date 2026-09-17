//! Direct policy comparisons, launch capture, and private approval text.

use super::*;
use crate::session::tests::make_session_and_context;
use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_sandboxing::SandboxType;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

fn terminal_permissions(profile: &PermissionProfile) -> TerminalPermissions {
    TerminalPermissions {
        policy: TerminalPolicy {
            sandbox: FileSystemSandboxContext::from_permission_profile(
                effective_permission_profile(profile, /*additional_permissions*/ None),
                PathUri::from_host_native_path(std::env::temp_dir()).expect("local temporary cwd"),
            ),
            environment_network: None,
            controller_network: None,
            controller_proxy: false,
        },
        sandbox_source: TerminalSandboxSource::Native,
        launch_permissions: SandboxPermissions::UseDefault,
        additional_permissions: None,
        internal_permissions: None,
    }
}

#[test]
fn reduced_permissions_require_review() -> anyhow::Result<()> {
    let permissions = terminal_permissions(&PermissionProfile::Disabled);
    let baseline = PermissionProfile::read_only();
    let current = terminal_permissions(&baseline);
    let expected = SandboxPermissions::RequireEscalated;
    assert_eq!(
        permissions.review_requirement(&current.policy, &baseline),
        Ok(expected)
    );
    insta::assert_snapshot!(
        "reduced_permissions",
        permissions.approval_reason(expected)?
    );
    Ok(())
}

#[test]
fn proxy_bypass_requires_review_even_when_permissions_match() -> anyhow::Result<()> {
    let baseline = PermissionProfile::Disabled;
    let mut permissions = terminal_permissions(&baseline);
    permissions.launch_permissions = SandboxPermissions::RequireEscalated;
    let expected = SandboxPermissions::RequireEscalated;
    assert_eq!(
        permissions.review_requirement(&permissions.policy, &baseline),
        Ok(expected)
    );
    insta::assert_snapshot!("proxy_bypass", permissions.approval_reason(expected)?);
    Ok(())
}

#[test]
fn denied_reads_reject_file_system_changes_but_only_review_network_changes() {
    let mut file_system = FileSystemSandboxPolicy::read_only();
    file_system.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: "**/.env".into(),
        },
        FileSystemAccessMode::Deny,
    ));
    let baseline =
        PermissionProfile::from_runtime_permissions(&file_system, NetworkSandboxPolicy::Restricted);
    let permissions = terminal_permissions(&PermissionProfile::Disabled);
    let current = terminal_permissions(&baseline);
    assert_eq!(
        permissions.review_requirement(&current.policy, &baseline),
        Err(
            "this terminal cannot enforce the current denied-read restrictions; start a new terminal"
        )
    );

    let permissions = current;
    let baseline =
        PermissionProfile::from_runtime_permissions(&file_system, NetworkSandboxPolicy::Enabled);
    let current = terminal_permissions(&baseline);
    assert_eq!(
        permissions.review_requirement(&current.policy, &baseline),
        Ok(SandboxPermissions::RequireEscalated)
    );
}

#[tokio::test]
async fn captured_network_changes_require_review_or_a_new_terminal() -> anyhow::Result<()> {
    let (_session, mut turn) = make_session_and_context().await;
    let mut environment = turn.environments.primary().expect("environment").clone();
    environment.config_mut().permission_profile =
        PermissionProfileSnapshot::legacy(PermissionProfile::read_only());
    let mut proxy = NetworkProxyConfig {
        enabled: true,
        ..Default::default()
    };
    Arc::make_mut(&mut turn.config).permissions.network =
        Some(NetworkProxySpec::from_config_and_constraints(
            proxy.clone(),
            /*requirements*/ None,
            &turn.permission_profile(),
        )?);
    let permissions = TerminalPermissions::for_launch(
        &environment,
        &turn,
        TerminalSandboxSource::Native,
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
        /*internal_permissions*/ None,
    );

    proxy.mode = NetworkMode::Limited;
    Arc::make_mut(&mut turn.config).permissions.network =
        Some(NetworkProxySpec::from_config_and_constraints(
            proxy.clone(),
            /*requirements*/ None,
            &turn.permission_profile(),
        )?);
    let current = TerminalPolicy::capture(
        &environment,
        &turn,
        TerminalSandboxSource::Native,
        /*additional_permissions*/ None,
    );
    let expected = SandboxPermissions::RequireEscalated;
    assert_eq!(
        permissions.review_requirement(&current, environment.permission_profile()),
        Ok(expected)
    );
    insta::assert_snapshot!("network_mode", permissions.approval_reason(expected)?);

    environment.config_mut().network_policy = Some(EnvironmentNetworkPolicy::from_config(
        &proxy, /*managed_allowed_domains_only*/ false,
    ));
    let current = TerminalPolicy::capture(
        &environment,
        &turn,
        TerminalSandboxSource::Native,
        /*additional_permissions*/ None,
    );
    assert_eq!(
        permissions.review_requirement(&current, environment.permission_profile()),
        Err(
            "this terminal cannot enforce the current environment-owned network restrictions; start a new terminal"
        )
    );
    Ok(())
}

#[tokio::test]
async fn internal_grants_require_review_without_exposing_paths() -> anyhow::Result<()> {
    let (_session, turn) = make_session_and_context().await;
    let mut environment = turn.environments.primary().expect("environment").clone();
    environment.config_mut().permission_profile =
        PermissionProfileSnapshot::legacy(PermissionProfile::read_only());
    let grants = serde_json::from_value(json!({
        "file_system": {
            "read": [turn.config.cwd.join("private-snapshot")],
            "write": [turn.config.cwd.join("private-metrics")]
        }
    }))?;
    let permissions = TerminalPermissions::for_launch(
        &environment,
        &turn,
        TerminalSandboxSource::Native,
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
        Some(&grants),
    );
    let current = TerminalPolicy::capture(
        &environment,
        &turn,
        TerminalSandboxSource::Native,
        Some(grants),
    );
    let expected = SandboxPermissions::WithAdditionalPermissions;
    assert_eq!(
        permissions.review_requirement(&current, environment.permission_profile()),
        Ok(expected)
    );
    assert_eq!(permissions.additional_permissions, None);
    insta::assert_snapshot!("internal_grant", permissions.approval_reason(expected)?);
    Ok(())
}

#[tokio::test]
async fn readable_snapshot_does_not_require_stdin_approval() -> anyhow::Result<()> {
    let (_session, turn) = make_session_and_context().await;
    let mut environment = turn.environments.primary().expect("environment").clone();
    environment.config_mut().permission_profile =
        PermissionProfileSnapshot::legacy(PermissionProfile::read_only());
    let snapshot = turn.config.cwd.join("shell-snapshot.sh");
    let grant = crate::shell_snapshot::snapshot_read_permissions(
        &snapshot,
        &environment.permission_profile_with_workspace_roots(),
        environment.cwd(),
    );
    assert_eq!(grant, None);
    let permissions = TerminalPermissions::for_launch(
        &environment,
        &turn,
        TerminalSandboxSource::Native,
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
        grant.as_ref(),
    );
    assert_eq!(
        permissions.review_requirement(&permissions.policy, environment.permission_profile()),
        Ok(SandboxPermissions::UseDefault)
    );
    Ok(())
}

#[test_case::test_case(TerminalSandboxSource::Native, SandboxType::None, SandboxPermissions::RequireEscalated; "native_disabled_sandbox_needs_review_when_enabled")]
#[test_case::test_case(TerminalSandboxSource::Executor, SandboxType::None, SandboxPermissions::UseDefault; "executor_keeps_its_restricted_token_default")]
#[test_case::test_case(TerminalSandboxSource::Native, SandboxType::WindowsMxc, SandboxPermissions::UseDefault; "native_mxc_ignores_legacy_level_changes")]
#[tokio::test]
async fn enabling_windows_sandbox_respects_the_launch_backend(
    source: TerminalSandboxSource,
    sandbox_type: SandboxType,
    expected: SandboxPermissions,
) -> anyhow::Result<()> {
    let (_session, turn) = make_session_and_context().await;
    let mut environment = turn.environments.primary().expect("environment").clone();
    environment.selection.cwd = PathUri::parse("file:///C:/workspace")?;
    environment.config_mut().permission_profile =
        PermissionProfileSnapshot::legacy(PermissionProfile::read_only());
    environment.config_mut().windows_sandbox_type = sandbox_type;
    environment.config_mut().windows_sandbox_level = WindowsSandboxLevel::Disabled;
    assert_eq!(
        environment.windows_sandbox_selection_for_turn_metadata(),
        if sandbox_type == SandboxType::WindowsMxc {
            codex_file_system::WindowsSandboxSelection::Mxc
        } else {
            codex_file_system::WindowsSandboxSelection::Disabled
        }
    );
    let permissions = TerminalPermissions::for_launch(
        &environment,
        &turn,
        source,
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
        /*internal_permissions*/ None,
    );
    let current = TerminalPolicy::capture(
        &environment,
        &turn,
        source,
        /*additional_permissions*/ None,
    );
    assert_eq!(
        permissions.review_requirement(&current, environment.permission_profile()),
        Ok(SandboxPermissions::UseDefault)
    );
    environment.config_mut().windows_sandbox_level = WindowsSandboxLevel::RestrictedToken;
    let current = TerminalPolicy::capture(
        &environment,
        &turn,
        source,
        /*additional_permissions*/ None,
    );
    assert_eq!(
        permissions.review_requirement(&current, environment.permission_profile()),
        Ok(expected)
    );
    Ok(())
}
