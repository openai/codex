use std::time::Duration;

use codex_app_server_protocol::PluginUninstallResponse;

use super::*;

#[tokio::test]
async fn successful_plugin_uninstall_dispatches_plugin_list_refresh() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let cwd = app.chat_widget.config_ref().cwd.to_path_buf();
    while app_event_rx.try_recv().is_ok() {}

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await?;
    let control = Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::PluginUninstallLoaded {
            cwd: cwd.clone(),
            plugin_id: "plugin-docs".to_string(),
            plugin_display_name: "Docs".to_string(),
            result: Ok(PluginUninstallResponse {}),
        },
    ))
    .await?;
    assert!(matches!(control, AppRunControl::Continue));

    let refresh_result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match app_event_rx.recv().await {
                Some(AppEvent::PluginsLoaded {
                    cwd: event_cwd,
                    result,
                }) if event_cwd == cwd => break result,
                Some(_) => {}
                None => panic!("app event channel closed before plugin refresh completed"),
            }
        }
    })
    .await
    .expect("dispatcher should initiate a plugin list refresh");
    refresh_result.expect("plugin list refresh should succeed");

    app_server.shutdown().await?;
    Ok(())
}
