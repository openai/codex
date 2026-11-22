//! Transient feedback while command-center selection waits for a session to load.
//! Repaint directly during backend waits because the app event handler owns the event loop.

use super::App;
use crate::app::agents_overview_view::OpeningTask;
use crate::motion::LOADING_FRAME_INTERVAL;
use crate::tui::Tui;
use codex_protocol::ThreadId;
use color_eyre::Result;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::future::Future;
use std::time::Instant;

impl App {
    pub(super) fn paint_docked_task_opening(
        &mut self,
        tui: &mut Tui,
        thread_id: ThreadId,
    ) -> Result<()> {
        if self.agents_overview.workspace.is_open() && tui.is_owned_screen() {
            if let Ok(mut state) = self.agents_overview.view_state.lock() {
                state.opening_task = Some(OpeningTask {
                    thread_id,
                    started_at: Instant::now(),
                });
            }
            // This handler awaits the backend, so a scheduled draw alone cannot show feedback.
            if let Err(error) = self.refresh_owned_transcript(tui) {
                self.finish_docked_task_opening(tui, thread_id);
                return Err(error);
            }
        }
        Ok(())
    }

    pub(in crate::app) fn docked_task_opening(&self, tui: &Tui) -> bool {
        tui.is_owned_screen()
            && self
                .agents_overview
                .view_state
                .lock()
                .is_ok_and(|state| state.opening_task.is_some())
    }

    pub(in crate::app) fn finish_docked_task_opening(
        &mut self,
        tui: &Tui,
        thread_id: ThreadId,
    ) -> bool {
        if let Ok(mut state) = self.agents_overview.view_state.lock()
            && state
                .opening_task
                .is_some_and(|opening| opening.thread_id == thread_id)
        {
            state.opening_task = None;
            tui.frame_requester().schedule_frame();
            return true;
        }
        false
    }

    pub(in crate::app) async fn wait_for_opening_task<T>(
        &mut self,
        tui: &mut Tui,
        request: impl Future<Output = T>,
    ) -> T {
        if !self.local_settings.tui.animations || !self.docked_task_opening(tui) {
            return request.await;
        }
        tokio::pin!(request);
        loop {
            tokio::select! {
                biased;
                result = &mut request => return result,
                _ = tokio::time::sleep(LOADING_FRAME_INTERVAL) => {
                    if let Err(error) = self.refresh_owned_transcript(tui) {
                        tracing::warn!(%error, "failed to update task opening progress");
                        // The request may already have changed the backend attachment.
                        return request.await;
                    }
                }
            }
        }
    }
}

pub(super) fn draw(tui: &mut Tui) -> std::io::Result<()> {
    tui.draw(u16::MAX, |frame| {
        let lines = textwrap::wrap("Loading task…", usize::from(frame.area().width.max(1)))
            .into_iter()
            .map(|line| Line::from(line.into_owned().bold()))
            .collect::<Vec<_>>();
        frame.render_widget_ref(&Paragraph::new(lines), frame.area());
    })?;
    tui.frame_requester().schedule_frame();
    Ok(())
}
