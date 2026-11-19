#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn hovering_a_saved_chat_highlights_both_lines_without_moving_selection() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let parent = ThreadId::new();
    let child = ThreadId::new();
    app.chat_widget
        .handle_thread_session(test_thread_session(parent, app.config.cwd.to_path_buf()));
    app.chat_widget.replay_thread_turns(
        vec![test_turn("parent-turn", TurnStatus::Completed, Vec::new())],
        crate::chatwidget::ReplayKind::ResumeInitialMessages,
    );
    app.active_thread_id = Some(parent);
    app.primary_thread_id = Some(parent);
    let pane = app.sidebar_mut();
    pane.show(
        sidebar::SidebarContent::Chooser,
        sidebar::PaneFocus::Sidebar,
    );
    pane.update_saved_chats(vec![child.into()]);
    pane.latest_prompts.insert(child, "Hover preview".into());
    pane.choice = 0;
    let choice = pane.choice;
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let size = Size::new(/*width*/ 100, /*height*/ 18);
    let colors = crate::terminal_probe::DefaultColors {
        fg: (224, 224, 224),
        bg: (20, 20, 20),
    };
    let mut redraws = crate::tui::test_support::record_frame_requests(&mut tui);
    let frame = side_frame(&mut app, &mut tui, size)?;
    let heading = frame
        .lines()
        .position(|line| line.contains("Side chats  "))
        .expect("side chats heading is visible") as u16;
    let row = app
        .conversation_frame
        .sidebar_targets
        .iter()
        .find_map(|(target, area)| {
            matches!(target, side_focus::SidebarTarget::Row(1)).then_some(*area)
        })
        .expect("saved chat row is visible");
    assert_eq!(row.height, 2);
    let column = row.right() - 1;
    let mut styles = Vec::new();
    for (name, y, redraw) in [
        ("preview", row.y + 1, true),
        ("title", row.y, false),
        ("heading", heading, true),
    ] {
        while redraws.try_recv().is_ok() {}
        let event = mouse(MouseEventKind::Moved, column, y);
        boxed_future!(app.handle_tui_event(&mut tui, &mut app_server, event)).await?;
        assert_eq!(redraws.try_recv().is_ok(), redraw, "{name}");
        crate::terminal_palette::with_test_default_colors(Some(colors), || {
            side_frame(&mut app, &mut tui, size)
        })?;
        let buffer = tui.terminal.previous_buffer();
        let fill = [row.y, row.y + 1].map(|y| buffer[(column, y)].bg);
        styles.push(format!("{name} title/preview fills {fill:?}"));
        assert_eq!(app.sidebar().unwrap().choice, choice);
    }
    insta::assert_snapshot!(styles.join("\n"), @"
    preview title/preview fills [Rgb(80, 80, 80), Rgb(80, 80, 80)]
    title title/preview fills [Rgb(80, 80, 80), Rgb(80, 80, 80)]
    heading title/preview fills [Reset, Reset]
    ");
    app_server.shutdown().await?;
    Ok(())
}
