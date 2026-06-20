use super::*;
use crate::context::world_state::WorldState;
use anyhow::Result;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn renders_full_environment_state() -> Result<()> {
    let context = EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///repo", "zsh")?),
            (
                "devbox".to_string(),
                available("file:///workspace", "bash")?,
            ),
        ]
        .into_iter()
        .collect(),
        current_date: Some("2026-06-20".to_string()),
        timezone: Some("America/Los_Angeles".to_string()),
        network: Some(NetworkContext::new(
            vec!["api.example.com".to_string()],
            vec!["blocked.example.com".to_string()],
        )),
        filesystem: Some(FileSystemContext::from_permission_profile(
            &PermissionProfile::Disabled,
            &[],
        )),
        subagents: Some("task_1: running\ntask_2: completed".to_string()),
    };

    let mut world_state = WorldState::default();
    world_state.add_section(context);

    assert_eq!(
        vec![user_message(
            r#"<environment_context>
  <environments>
    <environment id="devbox">
      <cwd>/workspace</cwd>
      <shell>bash</shell>
    </environment>
    <environment id="laptop">
      <cwd>/repo</cwd>
      <shell>zsh</shell>
    </environment>
  </environments>
  <current_date>2026-06-20</current_date>
  <timezone>America/Los_Angeles</timezone>
  <network enabled="true"><allowed>api.example.com</allowed><denied>blocked.example.com</denied></network>
  <filesystem><permission_profile type="disabled"><file_system type="unrestricted" /></permission_profile></filesystem>
  <subagents>
    task_1: running
    task_2: completed
  </subagents>
</environment_context>"#,
        )],
        world_state.render_full(),
    );
    Ok(())
}

#[test]
fn renders_only_changed_environments() -> Result<()> {
    let mut previous = WorldState::default();
    previous.add_section(EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///repo", "bash")?),
            ("devbox".to_string(), starting("file:///workspace")?),
            ("old".to_string(), available("file:///old", "sh")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    });
    let mut current = WorldState::default();
    current.add_section(EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///new-repo", "zsh")?),
            (
                "devbox".to_string(),
                available("file:///workspace", "powershell")?,
            ),
            ("remote".to_string(), starting("file:///remote")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    });

    assert_eq!(
        vec![user_message(
            r#"<environment_context>
  <environments>
    <environment id="laptop">
      <cwd>/new-repo</cwd>
      <shell>zsh</shell>
    </environment>
    <environment id="old" status="unavailable" />
    <environment id="remote" status="starting">
      <cwd>/remote</cwd>
    </environment>
  </environments>
</environment_context>"#,
        )],
        current.render_diff(&previous),
    );
    Ok(())
}

#[test]
fn persisted_turn_context_values_render_a_diff() -> Result<()> {
    let environments = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "zsh")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let mut previous = WorldState::default();
    previous.add_section(EnvironmentsState {
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
    });
    let mut current = WorldState::default();
    current.add_section(EnvironmentsState {
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
    });

    assert_eq!(
        vec![user_message(
            r#"<environment_context>
  <current_date>2026-06-20</current_date>
  <timezone>America/Los_Angeles</timezone>
  <network enabled="true"><allowed>new.example.com</allowed><denied>blocked.example.com</denied></network>
  <filesystem><permission_profile type="external"><file_system type="external" /></permission_profile></filesystem>
</environment_context>"#,
        )],
        current.render_diff(&previous),
    );
    Ok(())
}

#[test]
fn single_environment_diff_ignores_shell() -> Result<()> {
    let previous = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "bash")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let current = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "zsh")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    assert_eq!(None, current.render_diff(&previous));
    Ok(())
}

#[test]
fn removed_legacy_environment_renders_unavailable() -> Result<()> {
    let previous = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "bash")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    assert_eq!(
        Some(user_message(
            r#"<environment_context>
  <environments>
    <environment id="local" status="unavailable" />
  </environments>
</environment_context>"#,
        )),
        EnvironmentsState::default().render_diff(&previous),
    );
    Ok(())
}

#[test]
fn loaded_environment_state_produces_no_diff_with_live_state() -> Result<()> {
    let live_state = EnvironmentsState {
        environments: [
            ("laptop".to_string(), available("file:///repo", "zsh")?),
            ("devbox".to_string(), starting("file:///workspace")?),
        ]
        .into_iter()
        .collect(),
        current_date: Some("2026-06-20".to_string()),
        timezone: Some("America/Los_Angeles".to_string()),
        network: Some(NetworkContext::new(
            vec!["api.example.com".to_string()],
            vec!["blocked.example.com".to_string()],
        )),
        filesystem: Some(FileSystemContext::from_permission_profile(
            &PermissionProfile::Disabled,
            &[],
        )),
        subagents: Some("task_1: running".to_string()),
    };

    let stored = serde_json::to_value(&live_state)?;
    assert_eq!(
        json!({
            "environments": {
                "devbox": {
                    "cwd": "file:///workspace",
                },
                "laptop": {
                    "cwd": "file:///repo",
                },
            },
            "current_date": "2026-06-20",
            "timezone": "America/Los_Angeles",
            "network": {
                "domains": {
                    "api.example.com": "allow",
                    "blocked.example.com": "deny",
                },
            },
            "filesystem": {
                "workspace_roots": {},
                "permission_profile": "disabled",
            },
        }),
        stored,
    );
    let loaded_state = serde_json::from_value::<EnvironmentsState>(stored)?;
    let mut live_world = WorldState::default();
    live_world.add_section(live_state);
    let mut loaded_world = WorldState::default();
    loaded_world.add_section(loaded_state);
    assert_eq!(
        Vec::<ResponseItem>::new(),
        live_world.render_diff(&loaded_world),
    );
    Ok(())
}

fn available(cwd: &str, shell: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        cwd: PathUri::parse(cwd)?,
        status: Some(EnvironmentStatus::Available),
        shell: Some(shell.to_string()),
    })
}

fn starting(cwd: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        cwd: PathUri::parse(cwd)?,
        status: Some(EnvironmentStatus::Starting),
        shell: None,
    })
}

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        metadata: None,
    }
}
