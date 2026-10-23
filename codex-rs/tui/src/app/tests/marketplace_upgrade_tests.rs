//! Marketplace startup failures reach the warning notice and its details view.

use super::*;
use crate::app::owned_transcript::tests::buffer_text;
use crate::chatwidget::tests::helpers::normalize_snapshot_paths;
use crate::custom_terminal::test_support::last_rendered_buffer;
use codex_app_server_client::AppServerEvent;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;

#[tokio::test]
async fn automatic_marketplace_upgrade_failure_shows_warning() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let source = tempdir()?;
    std::fs::create_dir_all(source.path().join(".agents/plugins"))?;
    std::fs::write(
        source.path().join(".agents/plugins/marketplace.json"),
        r#"{"name":"other","plugins":[]}"#,
    )?;
    for args in [
        vec!["init"],
        vec!["add", "."],
        vec!["commit", "-m", "initial marketplace"],
    ] {
        let output = std::process::Command::new("git")
            .args(["-c", "commit.gpgSign=false"])
            .args(args)
            .current_dir(source.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // Both checkouts succeed, but their names do not match the configured marketplaces.
    let marketplace_names = ["team", "work"];
    for marketplace_name in marketplace_names {
        record_user_marketplace(
            app.config.codex_home.as_path(),
            marketplace_name,
            &MarketplaceConfigUpdate {
                source_type: "git",
                source: &source.path().display().to_string(),
                ref_name: None,
                sparse_paths: &[],
            },
        )?;
    }
    app.config = ConfigBuilder::default()
        .codex_home(app.config.codex_home.to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .harness_overrides(ConfigOverrides {
            cwd: Some(source.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await?;
    app.local_settings.tui.animations = false;
    app.local_settings.tui.show_tooltips = false;
    while events.try_recv().is_ok() {}

    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut warnings_received = 0;
    while warnings_received < marketplace_names.len() {
        let event = server.next_event().await.expect("app-server notification");
        let is_warning = matches!(
            &event.event,
            AppServerEvent::ServerNotification(notification)
                if matches!(notification.as_ref(), ServerNotification::Warning(_))
        );
        app.handle_backend_event(&server, event).await;
        if is_warning {
            warnings_received += 1;
        }
    }

    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    while let Ok(event) = events.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    let size = Size::new(/*width*/ 80, /*height*/ 16);
    tui.screen_size_for_event(&TuiEvent::Resize(size))?;
    app.render_owned_transcript(&mut tui, size)?;
    let notice = buffer_text(last_rendered_buffer(&tui.terminal));

    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(2).into())))
        .await?;
    app.render_owned_transcript(&mut tui, size)?;
    let first_details = buffer_text(last_rendered_buffer(&tui.terminal));

    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Right.into())))
        .await?;
    app.render_owned_transcript(&mut tui, size)?;
    let second_details = buffer_text(last_rendered_buffer(&tui.terminal));
    server.shutdown().await?;

    insta::assert_snapshot!(
        "marketplace_upgrade_warning",
        normalize_snapshot_paths(format!(
            "Warning notice:\n{notice}\n\nFirst warning details:\n{first_details}\n\nSecond warning details:\n{second_details}"
        ))
    );
    Ok(())
}
