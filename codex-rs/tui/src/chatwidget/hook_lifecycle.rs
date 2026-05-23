//! Hook run lifecycle handling for `ChatWidget`.
//!
//! This module keeps active hook cells, hook timers, and hook completion output
//! together.

use super::*;

impl ChatWidget {
    pub(super) fn on_hook_started(&mut self, run: codex_app_server_protocol::HookRunSummary) {
        self.flush_answer_stream_with_separator();
        self.flush_completed_hook_output();
        match self.active_hook_cell.as_mut() {
            Some(cell) => {
                cell.start_run(run);
                self.bump_active_cell_revision();
            }
            None => {
                self.active_hook_cell = Some(history_cell::new_active_hook_cell(
                    run,
                    self.config.animations,
                ));
                self.bump_active_cell_revision();
            }
        }
        self.request_redraw();
    }

    pub(super) fn on_hook_completed(
        &mut self,
        completed: codex_app_server_protocol::HookRunSummary,
    ) {
        let completed_existing_run = self
            .active_hook_cell
            .as_mut()
            .map(|cell| cell.complete_run(completed.clone()))
            .unwrap_or(false);
        if completed_existing_run {
            self.bump_active_cell_revision();
        } else {
            match self.active_hook_cell.as_mut() {
                Some(cell) => {
                    cell.add_completed_run(completed);
                    self.bump_active_cell_revision();
                }
                None => {
                    let cell =
                        history_cell::new_completed_hook_cell(completed, self.config.animations);
                    if !cell.is_empty() {
                        self.active_hook_cell = Some(cell);
                        self.bump_active_cell_revision();
                    }
                }
            }
        }
        self.flush_completed_hook_output();
        self.finish_active_hook_cell_if_idle();
        self.request_redraw();
    }

    pub(super) fn flush_completed_hook_output(&mut self) {
        let Some(completed_cell) = self
            .active_hook_cell
            .as_mut()
            .and_then(HookCell::take_completed_persistent_runs)
        else {
            return;
        };
        let active_cell_is_empty = self
            .active_hook_cell
            .as_ref()
            .is_some_and(HookCell::is_empty);
        if active_cell_is_empty {
            self.active_hook_cell = None;
        }
        self.bump_active_cell_revision();
        self.transcript.needs_final_message_separator = true;
        self.app_event_tx
            .send(AppEvent::InsertHistoryCell(Box::new(completed_cell)));
    }

    pub(super) fn finish_active_hook_cell_if_idle(&mut self) {
        let Some(cell) = self.active_hook_cell.as_ref() else {
            return;
        };
        if cell.is_empty() {
            self.active_hook_cell = None;
            self.bump_active_cell_revision();
            return;
        }
        if cell.should_flush()
            && let Some(cell) = self.active_hook_cell.take()
        {
            self.bump_active_cell_revision();
            self.transcript.needs_final_message_separator = true;
            self.app_event_tx
                .send(AppEvent::InsertHistoryCell(Box::new(cell)));
        }
    }

    pub(super) fn update_due_hook_visibility(&mut self) {
        let Some(cell) = self.active_hook_cell.as_mut() else {
            return;
        };
        let now = Instant::now();
        if cell.advance_time(now) {
            self.bump_active_cell_revision();
        }
        self.finish_active_hook_cell_if_idle();
    }

    pub(super) fn schedule_hook_timer_if_needed(&self) {
        if self.config.animations
            && self
                .active_hook_cell
                .as_ref()
                .is_some_and(HookCell::has_visible_running_run)
        {
            self.frame_requester
                .schedule_frame_in(Duration::from_millis(50));
        }

        let Some(deadline) = self
            .active_hook_cell
            .as_ref()
            .and_then(HookCell::next_timer_deadline)
        else {
            return;
        };
        let delay = deadline.saturating_duration_since(Instant::now());
        self.frame_requester.schedule_frame_in(delay);
    }
}
