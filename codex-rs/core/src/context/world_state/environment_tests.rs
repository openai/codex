use super::*;
use crate::context::ContextualUserFragment;
use crate::context::world_state::WorldState;
use anyhow::Result;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
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
        ..Default::default()
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
</environment_context>"#,
        )],
        render_fragments(world_state.render_full()),
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
    });

    assert_eq!(
        vec![user_message(
            r#"<environment_context>
  <environments>
    <environment id="devbox">
      <cwd>/workspace</cwd>
      <shell>powershell</shell>
    </environment>
    <environment id="laptop">
      <cwd>/repo</cwd>
      <shell>zsh</shell>
    </environment>
    <environment id="old" status="unavailable" />
    <environment id="remote">
      <cwd>/remote</cwd>
      <status>starting</status>
    </environment>
  </environments>
</environment_context>"#,
        )],
        render_fragments(current.render_diff(&previous.snapshot())),
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
            /*enabled*/ true,
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
            /*enabled*/ true,
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
        render_fragments(current.render_diff(&previous.snapshot())),
    );
    Ok(())
}

#[test]
fn persisted_snapshot_uses_model_visible_path_and_context_values() -> Result<()> {
    let mut world_state = WorldState::default();
    world_state.add_section(EnvironmentsState {
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
    });

    assert_eq!(
        serde_json::to_value(world_state.snapshot())?,
        json!({
            "environments": {
                "environments": {
                    "remote": {
                        "cwd": "C:\\windows",
                        "status": "available",
                        "shell": "powershell"
                    }
                },
                "filesystem": "<filesystem><permission_profile type=\"disabled\"><file_system type=\"unrestricted\" /></permission_profile></filesystem>"
            }
        }),
    );
    Ok(())
}

#[test]
fn single_environment_diff_ignores_unknown_shell() -> Result<()> {
    let previous = EnvironmentsState {
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
    let current = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            available("file:///repo", "zsh")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let previous = WorldStateSection::snapshot(&previous);

    assert_eq!(
        None,
        render_fragment(WorldStateSection::render_diff(
            &current,
            PreviousSectionState::Known(&previous),
        ))
    );
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
    let previous = WorldStateSection::snapshot(&previous);

    assert_eq!(
        Some(user_message(
            r#"<environment_context>
  <environments>
    <environment id="local" status="unavailable" />
  </environments>
</environment_context>"#,
        )),
        render_fragment(WorldStateSection::render_diff(
            &EnvironmentsState::default(),
            PreviousSectionState::Known(&previous),
        )),
    );
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

fn render_fragments(fragments: Vec<Box<dyn ContextualUserFragment>>) -> Vec<ResponseItem> {
    fragments
        .into_iter()
        .map(ContextualUserFragment::into_boxed_response_item)
        .collect()
}

fn render_fragment(fragment: Option<Box<dyn ContextualUserFragment>>) -> Option<ResponseItem> {
    fragment.map(ContextualUserFragment::into_boxed_response_item)
}

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}
