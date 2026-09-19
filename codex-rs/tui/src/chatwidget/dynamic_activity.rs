//! Retain dynamic tool state in either renderer; the app emits only finished rows to terminal history.

use super::*;

impl ChatWidget {
    pub(super) fn on_dynamic_tool_item(&mut self, item: ThreadItem) {
        let started = matches!(
            &item,
            ThreadItem::DynamicToolCall {
                status: codex_app_server_protocol::DynamicToolCallStatus::InProgress,
                ..
            }
        );
        self.defer_or_handle(
            item,
            |interrupts, item| {
                if started {
                    interrupts.push_item_started(item);
                } else {
                    interrupts.push_item_completed(item);
                }
            },
            Self::handle_dynamic_tool_item_now,
        );
    }

    pub(crate) fn handle_dynamic_tool_item_now(&mut self, item: ThreadItem) {
        let ThreadItem::DynamicToolCall { id, .. } = &item else {
            return;
        };
        let id = id.clone();
        if let Some(cell) = self.transcript.dynamic_calls.get(&id) {
            cell.update_from_item(item);
            if !cell.is_active() {
                self.transcript.dynamic_calls.remove(&id);
            }
        } else if let Some(cell) = history_cell::DynamicToolCallCell::from_item(item) {
            self.flush_answer_stream_with_separator();
            if cell.is_active() {
                self.transcript
                    .dynamic_calls
                    .insert(cell.call_id().to_owned(), cell.clone());
            }
            self.add_to_history(cell);
        }
        self.request_redraw();
    }

    pub(super) fn finish_dynamic_activity(&mut self) {
        for (_, cell) in self.transcript.dynamic_calls.drain() {
            cell.mark_interrupted();
        }
    }
}
