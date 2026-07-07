use super::*;
use crate::app::test_support::make_test_app_with_channels;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::test_support::PathBufExt;
use assert_matches::assert_matches;
use codex_protocol::permissions::NetworkSandboxPolicy;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

fn auto_review_selection() -> AutoReviewPresetSelection {
    AutoReviewPresetSelection {
        approval_policy: AskForApproval::OnRequest,
        profile_update: PermissionPresetProfileUpdate::Preserve,
        display_label: "Approve for me".to_string(),
    }
}

fn next_history_message(rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> String {
    let cell = std::iter::from_fn(|| rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .expect("expected permission transition history");
    cell.display_lines(/*width*/ 120)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn terminal_browser_detects_user_network_restriction_over_managed_enabled_default()
-> color_eyre::eyre::Result<()> {
    let config_dir = tempdir()?;
    let user_config = config_dir.path().join("user-config.toml");
    let system_config = config_dir.path().join("system-config.toml");
    let response: ConfigReadResponse = serde_json::from_value(serde_json::json!({
        "config": {
            "sandbox_workspace_write": { "network_access": false },
        },
        "origins": {
            WORKSPACE_NETWORK_ACCESS_KEY: {
                "name": {
                    "type": "user",
                    "file": user_config,
                    "profile": null,
                },
                "version": "user-v1",
            },
        },
        "layers": [
            {
                "name": {
                    "type": "user",
                    "file": user_config,
                    "profile": null,
                },
                "version": "user-v1",
                "config": {
                    "sandbox_workspace_write": { "network_access": false },
                },
            },
            {
                "name": {
                    "type": "system",
                    "file": system_config,
                },
                "version": "system-v1",
                "config": {
                    "sandbox_workspace_write": { "network_access": true },
                },
            },
        ],
    }))?;

    assert!(user_network_restriction_overrides_managed_enabled(
        &response
    ));
    Ok(())
}

#[tokio::test]
async fn terminal_browser_auto_review_restores_managed_network_override_before_reporting_success()
-> color_eyre::eyre::Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let config_dir = tempdir()?;
    let user_config_path = config_dir.path().join("config.toml").abs();
    let system_config_path = config_dir.path().join("system-config.toml");
    std::fs::write(
        &system_config_path,
        r#"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
network_access = true

[features]
guardian_approval = true
terminal_browser = true
"#,
    )?;
    std::fs::write(
        user_config_path.as_path(),
        r#"
approvals_reviewer = "auto_review"
approval_policy = "on-request"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
network_access = false

[features]
guardian_approval = true
terminal_browser = true
"#,
    )?;
    app.config.codex_home = config_dir.path().to_path_buf().abs();
    app.loader_overrides.user_config_path = Some(user_config_path.clone());
    app.loader_overrides.system_config_path = Some(system_config_path);
    let rebuilt = app
        .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
        .await?;
    assert_eq!(
        rebuilt.permissions.network_sandbox_policy(),
        NetworkSandboxPolicy::Restricted
    );
    app.config = rebuilt.clone();
    app.chat_widget
        .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);
    app.chat_widget
        .set_permission_profile_from_session_snapshot(
            PermissionProfileSnapshot::from_session_snapshot(
                rebuilt.permissions.permission_profile().clone(),
                rebuilt.permissions.active_permission_profile(),
            ),
        )?;
    app.chat_widget
        .set_permission_network(rebuilt.permissions.network.clone());
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.apply_auto_review_preset(
        &mut app_server,
        auto_review_selection(),
        ManagedNetworkChoice::Detect,
    )
    .await;

    let popup = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(popup.contains("Enable managed network access?"), "{popup}");
    app.chat_widget
        .handle_key_event(KeyEvent::from(KeyCode::Enter));
    let (selection, network_choice) = std::iter::from_fn(|| app_event_rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::ApplyAutoReviewPreset {
                selection,
                network_choice,
            } => Some((selection, network_choice)),
            _ => None,
        })
        .expect("restore confirmation should emit an Auto Review transition");
    assert_eq!(network_choice, ManagedNetworkChoice::RestoreManaged);

    app.apply_auto_review_preset(&mut app_server, selection, network_choice)
        .await;

    assert!(app.config.permissions.network_sandbox_policy().is_enabled());
    assert!(
        app.chat_widget
            .config_ref()
            .permissions
            .network_sandbox_policy()
            .is_enabled()
    );
    assert_matches!(
        op_rx.try_recv().expect("expected permission context update"),
        AppCommand::OverrideTurnContext {
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            permission_profile: Some(profile),
            ..
        } if profile.network_sandbox_policy().is_enabled()
    );
    let rendered = next_history_message(&mut app_event_rx);
    assert!(
        rendered.contains("Permissions updated to Approve for me; managed network access enabled"),
        "{rendered}"
    );
    let user_config = std::fs::read_to_string(user_config_path.as_path())?;
    assert!(!user_config.contains("network_access = false"));
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn terminal_browser_auto_review_keep_restricted_reports_review_only()
-> color_eyre::eyre::Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config
        .features
        .enable(Feature::TerminalBrowser)
        .expect("terminal browser feature should be mutable");
    app.chat_widget
        .set_feature_enabled(Feature::TerminalBrowser, /*enabled*/ true);
    assert_eq!(
        app.config.permissions.network_sandbox_policy(),
        NetworkSandboxPolicy::Restricted
    );
    let permission_profile = app.config.permissions.permission_profile().clone();
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.apply_auto_review_preset(
        &mut app_server,
        auto_review_selection(),
        ManagedNetworkChoice::KeepRestricted,
    )
    .await;

    assert_eq!(
        app.config.permissions.permission_profile(),
        &permission_profile
    );
    assert_eq!(
        op_rx.try_recv(),
        Ok(AppCommand::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            permission_profile: None,
            active_permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );
    let rendered = next_history_message(&mut app_event_rx);
    assert!(
        rendered.contains("Review mode updated to Approve for me"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Network access remains disabled"),
        "{rendered}"
    );
    app_server.shutdown().await?;
    Ok(())
}
