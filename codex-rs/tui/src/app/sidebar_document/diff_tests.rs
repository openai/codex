#![cfg(test)]

use super::*;
use crate::app::test_support::enable_side_retention;
use crate::app::test_support::side_frame;
use crate::app::tests::make_test_app_with_channels;
use crate::app::tests::test_thread_session;
use crate::slash_command::SlashCommand;
use crate::test_support::boxed_future;
use codex_app_server_client::AppServerClient;
use pretty_assertions::assert_eq;
use ratatui::layout::Size;
use tokio::sync::mpsc::UnboundedReceiver;

async fn invoke(
    app: &mut App,
    tui: &mut tui::Tui,
    server: &mut AppServerSession,
    events: &mut UnboundedReceiver<AppEvent>,
) -> Result<Uuid> {
    app.chat_widget.dispatch_command(SlashCommand::Diff);
    loop {
        let event = events.recv().await.unwrap();
        match event {
            AppEvent::OpenDiff { .. } => {
                boxed_future!(app.handle_event(tui, server, event)).await?;
            }
            AppEvent::SidebarDiffLoaded { request, .. } => return Ok(request),
            AppEvent::DiffResult { .. } => panic!("expected sidebar"),
            _ => {}
        }
    }
}

#[tokio::test]
async fn slash_diff_opens_one_reader_and_only_the_latest_result_updates_it() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let (client, _commands, _events) =
        codex_app_server_client::CloudClient::channel(/*capacity*/ 1);
    let mut server = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(client),
        crate::ThreadParamsMode::Cloud,
    );
    let thread = ThreadId::new();
    let local_session = test_thread_session(thread, app.config.cwd.to_path_buf());
    let mut remote_session = local_session.clone();
    let foreign_path = if cfg!(windows) { "/repo" } else { r"D:\repo" };
    remote_session.cwd = PathUri::try_from(LegacyAppPathString::from_string(foreign_path))?;
    app.chat_widget.handle_thread_session(remote_session);
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let wide = Size::new(/*width*/ 110, /*height*/ 12);
    let narrow = Size::new(/*width*/ 42, /*height*/ 12);
    let complete = |request, patch: &str| AppEvent::SidebarDiffLoaded {
        request,
        text: patch.to_owned(),
    };
    let patch = "diff --git a/fees.rs b/fees.rs\n\x1b[36m@@ -1 +1 @@\x1b[m\n\x1b[31m-const FEE: u32 = 5;\x1b[m\n\x1b[32m+const FEE: u32 = 7;\x1b[m\n";
    let first = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
    assert_eq!(app.panel_focus(), Some(PaneFocus::Sidebar));
    assert!(side_frame(&mut app, &mut tui, wide)?.contains("Opening…"));
    let latest = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
    boxed_future!(app.handle_event(&mut tui, &mut server, complete(latest, patch))).await?;
    boxed_future!(app.handle_event(&mut tui, &mut server, complete(first, "old patch"))).await?;
    let small = side_frame(&mut app, &mut tui, narrow)?;
    let buf = tui.terminal.previous_buffer();
    for (label, fg) in [
        ("-const", ratatui::style::Color::Red),
        ("+const", ratatui::style::Color::Green),
    ] {
        let (row, col) = small
            .lines()
            .enumerate()
            .find_map(|(y, l)| l.find(label).map(|x| (y, x)))
            .unwrap();
        assert_eq!(buf[(col as u16, row as u16)].fg, fg);
    }
    let refresh = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
    let refreshing = side_frame(&mut app, &mut tui, narrow)?;
    assert!(refreshing.contains("Refreshing…"));
    assert!(refreshing.contains("+const FEE: u32 = 7;"));
    app.focus_workspace_composer(&mut tui);
    let updated = patch.replace("= 7", "= 8");
    boxed_future!(app.handle_event(&mut tui, &mut server, complete(refresh, &updated))).await?;
    assert_eq!(app.panel_focus(), Some(PaneFocus::Main));
    let split = side_frame(&mut app, &mut tui, wide)?;
    assert!(!split.contains("Refreshing…"));
    let sidebar_text = |frame: &str| {
        frame
            .lines()
            .filter_map(|l| l.split_once('│').map(|(_, rhs)| rhs))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let sidebar = sidebar_text(&split);
    let clean = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
    boxed_future!(app.handle_event(&mut tui, &mut server, complete(clean, ""))).await?;
    assert!(side_frame(&mut app, &mut tui, wide)?.contains("No changes detected."));
    let pending = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
    app.focus_workspace_composer(&mut tui);
    let refreshing_clean = side_frame(&mut app, &mut tui, wide)?;
    assert!(refreshing_clean.contains("No changes detected."));
    assert!(refreshing_clean.contains("Refreshing…"));
    let refreshing_clean = sidebar_text(&refreshing_clean);
    insta::assert_snapshot!(
        "slash_diff_sidebar",
        format!(
            "Focused at 42 columns\n{small}\n\nRefreshing at 42 columns\n{refreshing}\n\nRefreshed beside chat\n{sidebar}\n\nRefreshing a clean tree beside chat\n{refreshing_clean}"
        )
    );
    let tab = SidebarTab::Document(app.sidebar().unwrap().diff_tab().unwrap());
    app.activate_sidebar_tab(&mut tui, &mut server, tab).await?;
    boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Char('q').into())
    ))
    .await?;
    boxed_future!(app.handle_event(&mut tui, &mut server, complete(pending, patch))).await?;
    assert_eq!(app.panel_focus(), None);
    assert!(
        app.sidebar()
            .unwrap()
            .recent_tab(/*excluding*/ None)
            .is_none()
    );

    let pending = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
    app.chat_widget.handle_thread_session(local_session);
    boxed_future!(app.handle_event(&mut tui, &mut server, complete(pending, patch))).await?;
    assert_eq!(app.panel_focus(), None);
    assert!(
        app.sidebar()
            .unwrap()
            .recent_tab(/*excluding*/ None)
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn large_diff_keeps_its_last_hunk_visible_across_resize_and_refresh() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let (client, _commands, _events) =
        codex_app_server_client::CloudClient::channel(/*capacity*/ 1);
    let mut server = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(client),
        crate::ThreadParamsMode::Cloud,
    );
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let wide = Size::new(/*width*/ 120, /*height*/ 12);
    let narrow = Size::new(/*width*/ 34, /*height*/ 12);
    let mut text = "+a line of generated output\n".repeat(66_000);
    for tail in ["+last hunk", "+refreshed hunk"] {
        text.push_str(tail);
        text.push('\n');
        let request = invoke(&mut app, &mut tui, &mut server, &mut events).await?;
        if tail == "+refreshed hunk" {
            let pending = side_frame(&mut app, &mut tui, narrow)?;
            assert!(pending.contains("Refreshing…"));
            assert!(pending.contains("+last hunk"));
        }
        let loaded = AppEvent::SidebarDiffLoaded {
            request,
            text: text.clone(),
        };
        boxed_future!(app.handle_event(&mut tui, &mut server, loaded)).await?;
        if tail == "+last hunk" {
            assert!(side_frame(&mut app, &mut tui, wide)?.contains("generated output"));
            boxed_future!(app.handle_tui_event(
                &mut tui,
                &mut server,
                TuiEvent::Key(KeyCode::End.into())
            ))
            .await?;
        }
        assert!(side_frame(&mut app, &mut tui, wide)?.contains(tail));
        assert!(side_frame(&mut app, &mut tui, narrow)?.contains(tail));
    }
    Ok(())
}
