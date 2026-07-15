use super::super::PreviousSectionState;
use super::super::test_support::render_section_cases;
use super::*;
use anyhow::Result;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;

#[test]
fn snapshots() -> Result<()> {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let full = EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///repo", "zsh")?),
            (
                "devbox".to_string(),
                available("file:///workspace", "bash")?,
            ),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let before_environment_changes = EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///repo", "bash")?),
            ("devbox".to_string(), starting("file:///workspace")?),
            ("old".to_string(), available("file:///old", "sh")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let after_environment_changes = EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///repo", "zsh")?),
            (
                "devbox".to_string(),
                available("file:///workspace", "powershell")?,
            ),
            ("remote".to_string(), starting("file:///remote")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let environments = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "zsh")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let before_turn_context_changes = EnvironmentsState {
        current_date: Some("2026-06-19".to_string()),
        timezone: Some("UTC".to_string()),
        network: Some(NetworkContext::new(
            vec!["old.example.com".to_string()],
            vec![],
        )),
        filesystem: Some(FileSystemContext::from_permission_profile(
            &PermissionProfile::Disabled,
            &[],
        )),
        ..environments.clone()
    };
    let after_turn_context_changes = EnvironmentsState {
        current_date: Some("2026-06-20".to_string()),
        timezone: Some("America/Los_Angeles".to_string()),
        network: Some(NetworkContext::new(
            vec!["new.example.com".to_string()],
            vec!["blocked.example.com".to_string()],
        )),
        filesystem: Some(FileSystemContext::from_permission_profile(
            &PermissionProfile::External {
                network: NetworkSandboxPolicy::Restricted,
            },
            &[],
        )),
        ..environments
    };
    let foreign_windows = EnvironmentsState {
        environments: [(
            "remote".to_string(),
            available("file:///C:/windows", "powershell")?,
        )]
        .into_iter()
        .collect(),
        filesystem: Some(FileSystemContext::from_permission_profile(
            &PermissionProfile::Disabled,
            &[],
        )),
        ..Default::default()
    };
    let unknown_shell = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            EnvironmentState {
                cwd: PathUri::parse("file:///repo")?,
                status: EnvironmentStatus::Available,
                shell: None,
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let known_shell = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "zsh")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let legacy_environment = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "bash")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let empty = EnvironmentsState::default();

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&full)),
        (Unknown, Known(&full)),
        (
            Known(&before_environment_changes),
            Known(&after_environment_changes),
        ),
        (
            Known(&before_turn_context_changes),
            Known(&after_turn_context_changes),
        ),
        (Absent, Known(&foreign_windows)),
        (Known(&unknown_shell), Known(&known_shell)),
        (Known(&legacy_environment), Known(&empty)),
    ]));
    Ok(())
}

fn available(cwd: &str, shell: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        cwd: PathUri::parse(cwd)?,
        status: EnvironmentStatus::Available,
        shell: Some(shell.to_string()),
    })
}

fn starting(cwd: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        cwd: PathUri::parse(cwd)?,
        status: EnvironmentStatus::Starting,
        shell: None,
    })
}
