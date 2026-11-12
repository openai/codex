#![cfg(test)]

use crate::AppServerTarget;
use crate::app::test_support::make_test_app;
use codex_config::types::BuiltinStatusLine;
use codex_config::types::StatusLineProviderId;
use codex_config::types::StatusLineSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn remote_session_saves_to_the_active_local_profile() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let base = app.local_settings.user_config_path.clone();
    let base_text = "[tui]\nstatus_line_provider = { server = 'personal', tool = 'usage' }\n";
    std::fs::write(&base, base_text)?;
    let profile = app.local_settings.codex_home.join("work.config.toml");
    std::fs::write(&profile, "[tui]\ntheme = 'nord'\n")?;
    app.loader_overrides.user_config_path = Some(profile.clone());
    app.loader_overrides.user_config_profile = Some("work".parse()?);

    app.cli_kv_overrides = vec![("tui.status_line_provider".into(), "default".into())];
    app.refresh_in_memory_config_from_disk()
        .await
        .expect("load the initial provider from -c");
    assert_eq!(
        app.local_settings.tui.status_line_provider.as_deref(),
        Some(&StatusLineSelection::Builtin(BuiltinStatusLine::Default)),
    );

    // The running thread's config and cwd belong to the remote app server.
    let remote = tempfile::tempdir()?;
    let remote_file = remote.path().join("config.toml");
    std::fs::write(&remote_file, "[tui]\ntheme = 'remote'\n")?;
    app.config.codex_home = AbsolutePathBuf::from_absolute_path(remote.path())?;
    app.config.cwd = app.config.codex_home.clone();
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:8765").expect("test endpoint"),
    };
    for (selection, expected) in [
        (
            StatusLineSelection::Builtin(BuiltinStatusLine::Default),
            toml::toml! {
                [tui]
                theme = "nord"
                status_line_provider = "default"
            },
        ),
        (
            StatusLineSelection::Mcp(StatusLineProviderId {
                server: "work".into(),
                tool: "usage".into(),
            }),
            toml::toml! {
                [tui]
                theme = "nord"
                status_line_provider = { server = "work", tool = "usage" }
            },
        ),
    ] {
        app.save_status_line_provider(selection.clone()).await?;
        let saved: toml::Table = toml::from_str(&std::fs::read_to_string(&profile)?)?;
        assert_eq!(saved, expected);
        assert_eq!(
            (
                app.local_settings.tui.status_line_provider.as_deref(),
                app.chat_widget
                    .local_settings
                    .tui
                    .status_line_provider
                    .as_deref()
            ),
            (Some(&selection), Some(&selection)),
        );
    }
    app.refresh_in_memory_config_from_disk()
        .await
        .expect("reload the CLI override");
    let default = StatusLineSelection::Builtin(BuiltinStatusLine::Default);
    assert_eq!(
        (
            app.local_settings.tui.status_line_provider.as_deref(),
            app.chat_widget
                .local_settings
                .tui
                .status_line_provider
                .as_deref(),
        ),
        (Some(&default), Some(&default)),
    );
    assert_eq!(std::fs::read_to_string(base)?, base_text);
    assert_eq!(
        std::fs::read_to_string(remote_file)?,
        "[tui]\ntheme = 'remote'\n"
    );
    Ok(())
}

#[tokio::test]
async fn failed_write_preserves_the_file_and_the_live_selection() -> anyhow::Result<()> {
    let mut app = make_test_app().await;
    let path = app.local_settings.user_config_path.clone();
    let active = StatusLineSelection::Mcp(StatusLineProviderId {
        server: "work".into(),
        tool: "usage".into(),
    });
    app.save_status_line_provider(active).await?;
    let before = (
        app.local_settings.clone(),
        app.chat_widget.local_settings.clone(),
    );
    for contents in ["[malformed", "[[tui]]\ntheme = 'nord'\n"] {
        std::fs::write(&path, contents)?;
        let selection = StatusLineSelection::Builtin(BuiltinStatusLine::Default);
        assert!(app.save_status_line_provider(selection).await.is_err());
        assert_eq!(std::fs::read_to_string(&path)?, contents);
        assert_eq!(
            (&app.local_settings, &app.chat_widget.local_settings),
            (&before.0, &before.1)
        );
    }
    Ok(())
}
