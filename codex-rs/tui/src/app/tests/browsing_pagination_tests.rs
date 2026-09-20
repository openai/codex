//! Prompt browsing pages past a partial answer without losing its selection or exit origin.

use super::pagination_completion_tests::completed_history_app;
use super::session_lifecycle_requests::start_recording_app_server;
use super::*;
use crate::history_cell::PlainHistoryCell;
use crate::pager_overlay::TranscriptHistoryState;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn browsing_loads_before_the_oldest_prompt_after_a_partial_answer() -> Result<()> {
    for cancel_before_completion in [false, true] {
        let (mut app, _codex_home, thread_id) =
            completed_history_app(&["Oldest", "Middle", "Newest"]).await?;
        let (mut app_server, _requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        let started = app_server
            .resume_thread(
                &app.local_settings,
                app.config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::RestoreFromThread,
            )
            .await?;
        // A transport page may begin inside the preceding answer, well before its first prompt.
        app.transcript_cells = (0..30)
            .map(|index| {
                Arc::new(PlainHistoryCell::new(vec![
                    format!("Earlier answer fragment {index}").into(),
                ])) as Arc<dyn HistoryCell>
            })
            .collect();
        app.transcript_cells
            .extend(crate::thread_transcript::thread_items_to_transcript_cells(
                Some(thread_id),
                &app.config.cwd,
                started.turns.iter().flat_map(|turn| turn.items.clone()),
                crate::thread_transcript::RawReasoningVisibility::Hidden,
                Some(&app.config),
            ));
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        app.scrollback_has_older_history = app_server.has_older_history(thread_id);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = crate::app_event_sender::AppEventSender::new(event_tx);
        app.transcript_view.history = TranscriptHistoryState::Partial;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(/*owned*/ true)?;
        let size = tui.terminal.size()?;
        app.render_owned_transcript(&mut tui, size)?;
        for _ in 0..2 {
            app.handle_tui_event(
                &mut tui,
                &mut app_server,
                TuiEvent::Key(KeyCode::Esc.into()),
            )
            .await?;
        }
        assert_eq!(
            (
                app.backtrack.overlay_preview_active,
                app.backtrack.nth_user_message
            ),
            (true, 0)
        );
        assert!(!app.transcript_view.needs_history(&app.transcript_cells));
        app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
            .await?;
        assert_eq!(
            app.transcript_view.history,
            TranscriptHistoryState::LoadingOlder
        );
        let first_event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), event_rx.recv())
            .await?
            .expect("older history completion");
        let AppEvent::OlderThreadHistoryLoaded { cursor, .. } = first_event else {
            panic!("expected an older history completion")
        };
        // Repeated navigation while loading must keep a single request in flight.
        for code in [KeyCode::Left, KeyCode::Char('h')] {
            app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(code.into()))
                .await?;
        }
        assert!(app_server.is_older_history_page_pending(thread_id, &cursor));
        assert!(event_rx.try_recv().is_err());
        // Complete only an answer item first: browsing must continue to a page with a prompt.
        let first_page = app_server
            .thread_items_page(
                thread_id,
                /*turn_id*/ None,
                Some(cursor.clone()),
                /*limit*/ 1,
            )
            .await?;
        let next_cursor = first_page.next_cursor.clone().expect("older prompt page");
        if cancel_before_completion {
            app.handle_tui_event(
                &mut tui,
                &mut app_server,
                TuiEvent::Key(KeyCode::Esc.into()),
            )
            .await?;
        }
        app.handle_older_history_page(
            &mut tui,
            &mut app_server,
            thread_id,
            &cursor,
            Ok(first_page),
        )
        .await?;
        assert_eq!(
            app_server.is_older_history_page_pending(thread_id, &next_cursor),
            !cancel_before_completion
        );
        if cancel_before_completion {
            assert_eq!(
                (
                    app.backtrack.overlay_preview_active,
                    app.transcript_view.is_following()
                ),
                (false, true)
            );
        } else {
            let final_event =
                tokio::time::timeout(Duration::from_secs(/*secs*/ 5), event_rx.recv())
                    .await?
                    .expect("older prompt completion");
            Box::pin(app.handle_event(&mut tui, &mut app_server, final_event)).await?;
            assert_eq!(
                (app.backtrack.nth_user_message, app.transcript_view.history),
                (2, TranscriptHistoryState::Complete)
            );
            for (code, selected) in [(KeyCode::Left, 1), (KeyCode::Char('h'), 0)] {
                app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(code.into()))
                    .await?;
                assert_eq!(app.backtrack.nth_user_message, selected);
            }
            app.render_owned_transcript(&mut tui, size)?;
            let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
            let text = buffer
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>();
            assert!(text.contains("Oldest prompt"), "{text}");
        }
        tui.set_owned_screen(/*owned*/ false)?;
        app_server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}
