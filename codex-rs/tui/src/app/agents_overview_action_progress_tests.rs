//! The dashboard remains browsable while lifecycle requests hold the server lock.

use super::*;
use crate::chatwidget::tests::helpers::normalize_agent_center_snapshot;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use crossterm::event::KeyCode;
use futures::stream;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::collections::VecDeque;
use std::task::Poll;
use std::time::Duration;

#[tokio::test]
async fn modal_consumes_input_and_finishes_after_input_closes() -> color_eyre::Result<()> {
    for (action, snapshot) in [
        (AgentsOverviewAction::Archive, "archive_progress"),
        (AgentsOverviewAction::Delete, "delete_progress"),
    ] {
        let (mut app, mut rx, _) = crate::app::tests::make_test_app_with_channels().await;
        let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        for index in 1..=3 {
            let id = ThreadId::from_string(
                &app_test_support::create_fake_rollout(
                    &app.config.codex_home,
                    &format!("2025-01-0{index}T12-00-00"),
                    &format!("2025-01-0{index}T12:00:00Z"),
                    &format!("Task {index}"),
                    Some(&app.config.model_provider_id),
                    /*git_info*/ None,
                )
                .expect("materialize task"),
            )?;
            let mut thread = server.thread_read(id, /*include_turns*/ false).await?;
            thread.cwd = test_path_buf("/tmp/project").abs().into();
            app.agents_overview.threads.insert(id, Some(thread));
        }
        let threads = app
            .agents_overview
            .threads
            .values()
            .flatten()
            .cloned()
            .collect();
        let view = app.agents_overview_view(threads, /*selected_thread_id*/ None);
        app.agents_overview.visible_thread_ids = view.thread_ids();
        app.chat_widget.show_bottom_pane_view(Box::new(view));
        let removed = app.agents_overview.visible_thread_ids[0];
        app.prepare_agents_overview_removal(&HashSet::from([removed]));
        app.agents_overview.hidden_threads.insert(removed);
        app.agents_overview.view_state.lock().unwrap().removal = Some(action);
        app.repaint_agents_overview();
        assert!(!app.agents_overview.visible_thread_ids.contains(&removed));
        while rx.try_recv().is_ok() {}
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.pause_events();
        let (finished_input, wait_for_input) = tokio::sync::oneshot::channel();
        let mut finished_input = Some(finished_input);
        let mut events = VecDeque::from([
            tui::TuiEvent::Key(KeyCode::End.into()),
            // Leaving and server mutations wait without accumulating actions.
            tui::TuiEvent::Key(KeyCode::Esc.into()),
            tui::TuiEvent::Key(KeyCode::Enter.into()),
            tui::TuiEvent::Key(KeyCode::Char('n').into()),
            tui::TuiEvent::Draw,
            tui::TuiEvent::Resize(ratatui::layout::Size::new(66, 20)),
            tui::TuiEvent::FocusGained,
        ]);
        let events = stream::poll_fn(move |_| {
            if let Some(event) = events.pop_front() {
                Poll::Ready(Some(event))
            } else {
                if let Some(finished_input) = finished_input.take() {
                    let _ = finished_input.send(());
                }
                Poll::Ready(None)
            }
        });
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            run_lifecycle_modal(
                &mut tui,
                &mut app,
                async {
                    wait_for_input.await.expect("all modal input was consumed");
                    "operation finished"
                },
                events,
            ),
        )
        .await??;
        assert_eq!(result, "operation finished");
        assert_eq!(
            tui.terminal.last_known_screen_size,
            ratatui::layout::Size::new(66, 20)
        );
        let selected = app
            .chat_widget
            .selected_index_for_active_view(AGENTS_OVERVIEW_VIEW_ID)
            .expect("dashboard stays open");
        assert_eq!(selected, app.agents_overview.visible_thread_ids.len() - 1);
        assert!(
            !std::iter::from_fn(|| rx.try_recv().ok()).any(|event| matches!(
                event,
                AppEvent::SelectAgentsOverviewThread { .. }
                    | AppEvent::NewAgentsOverviewSession { .. }
            ))
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
        terminal.draw(|frame| app.chat_widget.render(frame.area(), frame.buffer_mut()))?;
        let text = normalize_agent_center_snapshot(terminal.backend().to_string());
        let age = regex_lite::Regex::new(r" +[0-9]+d ago").unwrap();
        insta::assert_snapshot!(snapshot, age.replace_all(&text, " [age]"));
        server.shutdown().await?;
    }
    Ok(())
}
