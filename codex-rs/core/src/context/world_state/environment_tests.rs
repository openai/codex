use super::super::PreviousSectionState;
use super::super::test_support::render_section_cases;
use super::*;
use crate::context::world_state::test_support::FragmentSectionTestExt as _;
use anyhow::Result;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol_test_support::ContentItemTestExt;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn snapshots() -> Result<()> {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let full = EnvironmentsState {
        environments: [
            ("laptop".to_string(), primary("file:///repo", "zsh")?),
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
            ("laptop".to_string(), primary("file:///repo", "bash")?),
            ("devbox".to_string(), starting("file:///workspace")?),
            ("old".to_string(), available("file:///old", "sh")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let after_environment_changes = EnvironmentsState {
        environments: [
            ("laptop".to_string(), primary("file:///repo", "zsh")?),
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
            FileSystemFormat::Full,
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
            FileSystemFormat::Full,
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
            FileSystemFormat::Full,
        )),
        ..Default::default()
    };
    let unknown_shell = EnvironmentsState {
        environments: [(
            LOCAL_ENVIRONMENT_ID.to_string(),
            EnvironmentState {
                cwd: PathUri::parse("file:///repo")?,
                status: EnvironmentStatus::Available,
                error: None,
                shell: None,
                is_primary: false,
                filesystem: None,
                network_access: None,
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

#[test]
fn changing_primary_environment_updates_model_context_and_persisted_state() -> Result<()> {
    let before = EnvironmentsState {
        environments: [
            ("local".to_string(), primary("file:///local", "bash")?),
            ("remote".to_string(), available("file:///remote", "zsh")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let after = EnvironmentsState {
        environments: [
            ("local".to_string(), available("file:///local", "bash")?),
            ("remote".to_string(), primary("file:///remote", "zsh")?),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let previous = before
        .render_fragment_diff(PreviousSectionState::Absent)
        .0
        .unwrap();
    let rendered = after
        .render_fragment_diff(PreviousSectionState::Known(&previous))
        .1
        .expect("primary change should update the model")
        .render()
        .expect_text();

    assert_eq!(
        rendered,
        format!(
            "<environment_context>\n  <environments>\n    <environment id=\"local\" primary=\"false\">\n      <cwd>{}</cwd>\n      <shell>bash</shell>\n    </environment>\n    <environment id=\"remote\" primary=\"true\">\n      <cwd>{}</cwd>\n      <shell>zsh</shell>\n    </environment>\n  </environments>\n</environment_context>",
            PathUri::parse("file:///local")?.inferred_native_path_string(),
            PathUri::parse("file:///remote")?.inferred_native_path_string(),
        )
    );

    let mut previous_world_state = super::super::WorldState::default();
    previous_world_state.add_section(before);
    let mut current_world_state = super::super::WorldState::default();
    current_world_state.add_section(after);
    assert_eq!(
        current_world_state
            .render_full()
            .0
            .merge_patch_from(&previous_world_state.render_full().0)
            .map(serde_json::Value::Object),
        Some(json!({
            "environments": {
                "environments": {
                    "local": { "is_primary": null },
                    "remote": { "is_primary": true },
                },
            },
        }))
    );

    Ok(())
}

#[test]
fn legacy_single_environment_snapshot_preserves_environment_and_refreshes_date() -> Result<()> {
    let environment = EnvironmentsState {
        environments: [("local".to_string(), primary("file:///repo", "bash")?)]
            .into_iter()
            .collect(),
        ..Default::default()
    };
    let mut legacy_value = json!({
        "environments": {
            "local": {
                "cwd": PathUri::parse("file:///repo")?.inferred_native_path_string(),
                "status": "available",
                "shell": "bash",
            },
        },
    });
    let legacy_snapshot = serde_json::from_value::<EnvironmentsSnapshot>(legacy_value.clone())?;

    assert!(
        environment
            .render_fragment_diff(PreviousSectionState::Known(&legacy_snapshot))
            .1
            .is_none()
    );
    assert_eq!(
        serde_json::to_value(
            environment
                .render_fragment_diff(PreviousSectionState::Absent)
                .0
                .unwrap()
        )?["environments"]["local"],
        json!({
            "cwd": PathUri::parse("file:///repo")?.inferred_native_path_string(),
            "status": "available",
            "shell": "bash",
        })
    );

    let environment = EnvironmentsState {
        current_date: Some("2026-06-19".to_string()),
        ..environment
    };
    legacy_value["current_date"] = json!("2026-06-19");
    let legacy_snapshot = serde_json::from_value(legacy_value)?;
    assert!(
        environment
            .render_fragment_diff(PreviousSectionState::Known(&legacy_snapshot))
            .1
            .is_none()
    );
    let environment = EnvironmentsState {
        current_date_is_utc: true,
        ..environment
    };
    let (snapshot, update) =
        environment.render_fragment_diff(PreviousSectionState::Known(&legacy_snapshot));
    assert_eq!(
        update
            .expect("legacy date should be refreshed")
            .render()
            .expect_text(),
        "<environment_context>\n  <current_date_utc>2026-06-19</current_date_utc>\n</environment_context>"
    );
    let snapshot = serde_json::from_value(serde_json::to_value(snapshot.unwrap())?)?;
    assert!(
        environment
            .render_fragment_diff(PreviousSectionState::Known(&snapshot))
            .1
            .is_none()
    );

    Ok(())
}

#[test]
fn crossing_single_environment_boundary_restates_current_environments() -> Result<()> {
    let single = EnvironmentsState {
        environments: [("local".to_string(), primary("file:///local", "bash")?)]
            .into_iter()
            .collect(),
        ..Default::default()
    };
    let local_cwd = PathUri::parse("file:///local")?.inferred_native_path_string();
    let remote_cwd = PathUri::parse("file:///remote")?.inferred_native_path_string();

    for (local_is_primary, remote_is_primary) in [(true, false), (false, true)] {
        let multiple = EnvironmentsState {
            environments: [
                (
                    "local".to_string(),
                    EnvironmentState {
                        is_primary: local_is_primary,
                        ..available("file:///local", "bash")?
                    },
                ),
                (
                    "remote".to_string(),
                    EnvironmentState {
                        is_primary: remote_is_primary,
                        ..available("file:///remote", "zsh")?
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let expanded = multiple
            .render_fragment_diff(PreviousSectionState::Known(
                &single
                    .render_fragment_diff(PreviousSectionState::Absent)
                    .0
                    .unwrap(),
            ))
            .1
            .expect("adding an environment should update the model")
            .render()
            .expect_text();
        assert_eq!(
            expanded,
            format!(
                "<environment_context>\n  <environments>\n    <environment id=\"local\" primary=\"{local_is_primary}\">\n      <cwd>{local_cwd}</cwd>\n      <shell>bash</shell>\n    </environment>\n    <environment id=\"remote\" primary=\"{remote_is_primary}\">\n      <cwd>{remote_cwd}</cwd>\n      <shell>zsh</shell>\n    </environment>\n  </environments>\n</environment_context>"
            )
        );

        let reduced = single
            .render_fragment_diff(PreviousSectionState::Known(
                &multiple
                    .render_fragment_diff(PreviousSectionState::Absent)
                    .0
                    .unwrap(),
            ))
            .1
            .expect("removing an environment should update the model")
            .render()
            .expect_text();
        assert_eq!(
            reduced,
            format!(
                "<environment_context>\n  <environments>\n    <environment id=\"local\" primary=\"true\">\n      <cwd>{local_cwd}</cwd>\n      <shell>bash</shell>\n    </environment>\n    <environment id=\"remote\" status=\"unavailable\" />\n  </environments>\n</environment_context>"
            )
        );
    }

    Ok(())
}

fn available(cwd: &str, shell: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        cwd: PathUri::parse(cwd)?,
        status: EnvironmentStatus::Available,
        error: None,
        shell: Some(shell.to_string()),
        is_primary: false,
        filesystem: None,
        network_access: None,
    })
}

fn primary(cwd: &str, shell: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        is_primary: true,
        ..available(cwd, shell)?
    })
}

fn starting(cwd: &str) -> Result<EnvironmentState> {
    Ok(EnvironmentState {
        cwd: PathUri::parse(cwd)?,
        status: EnvironmentStatus::Starting,
        error: None,
        shell: None,
        is_primary: false,
        filesystem: None,
        network_access: None,
    })
}

#[test]
fn failure_context_is_escaped_incremental_and_cleared_on_recovery() -> Result<()> {
    let failed = EnvironmentsState {
        environments: [(
            "remote".to_string(),
            EnvironmentState {
                cwd: PathUri::parse("file:///workspace")?,
                status: EnvironmentStatus::Failed,
                error: Some("Repository <empty> & unavailable".to_string()),
                shell: None,
                is_primary: false,
                filesystem: None,
                network_access: None,
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    insta::assert_snapshot!(failed.body().expect_text(), @r#"

      <environments>
        <environment id="remote">
          <cwd>/workspace</cwd>
          <status>failed</status>
          <error>Repository &lt;empty&gt; &amp; unavailable</error>
        </environment>
      </environments>
    "#);
    assert!(
        failed
            .render_fragment_diff(PreviousSectionState::Known(
                &failed
                    .render_fragment_diff(PreviousSectionState::Absent)
                    .0
                    .unwrap()
            ))
            .1
            .is_none()
    );
    let recovered = EnvironmentsState {
        environments: [(
            "remote".to_string(),
            available("file:///workspace", "bash")?,
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let update = recovered
        .render_fragment_diff(PreviousSectionState::Known(
            &failed
                .render_fragment_diff(PreviousSectionState::Absent)
                .0
                .unwrap(),
        ))
        .1
        .expect("recovery must be visible to the model");
    assert!(!update.body().expect_text().contains("<error>"));
    assert!(update.body().expect_text().contains("<shell>bash</shell>"));
    Ok(())
}

#[test]
fn failure_context_limits_total_detail_bytes_at_utf8_boundaries() {
    use codex_protocol::protocol::EnvironmentConfigState;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    let long_error = "界".repeat(300);
    let snapshot = TurnEnvironmentSnapshot {
        environments: [long_error.as_str(), &long_error, &long_error, "abc", "d"]
            .into_iter()
            .enumerate()
            .map(|(index, error)| TurnEnvironmentState::Failed {
                required_skills: Vec::new(),
                selection: TurnEnvironmentSelection {
                    selected_capability_roots: Default::default(),
                    environment_id: format!("remote-{index}"),
                    cwd: PathUri::parse("file:///workspace").unwrap(),
                    workspace_roots: Vec::new(),
                    config: EnvironmentConfigState::FromThread,
                },
                error: error.to_owned(),
            })
            .collect(),
    };
    let states = environment_states(
        &snapshot,
        /*per_environment*/ false,
        FileSystemFormat::Full,
    );
    let details: Vec<_> = states
        .values()
        .map(|state| state.error.as_deref())
        .collect();
    let truncated = "界".repeat(85);
    assert_eq!(
        details,
        vec![
            Some(truncated.as_str()),
            Some(truncated.as_str()),
            None,
            Some("ab"),
            None
        ]
    );
}
