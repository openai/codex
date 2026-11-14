//! Frozen transcript views preserve drafts while their live attachment is unavailable.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadOnlyThread {
    ExternalWriter,
    Archived,
}

impl ReadOnlyThread {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::ExternalWriter => "Close it there and press R to continue here.",
            Self::Archived => "Press U to unarchive this thread and continue here.",
        }
    }

    pub(super) fn action(self) -> (&'static str, &'static str) {
        match self {
            Self::ExternalWriter => ("r", "retry"),
            Self::Archived => ("u", "unarchive"),
        }
    }
}

impl ChatWidget {
    pub(crate) fn show_external_writer_thread(&mut self) {
        self.show_read_only_thread(ReadOnlyThread::ExternalWriter);
    }

    pub(crate) fn show_read_only_thread(&mut self, reason: ReadOnlyThread) {
        self.cancel_image_submission();
        self.blocks_direct_input = true;
        self.read_only_thread = Some(reason);
        self.input_queue.suppress_queue_autosend = true;
        self.set_initial_user_message_submit_suppressed(/*suppressed*/ true);
        if let Some(message) = self.initial_user_message.take() {
            self.restore_user_message_to_composer(message);
        }
        self.pause_unavailable_thread();
        if let Some(questions) = &mut self.bottom_pane.questions {
            questions.set_expanded(/*expanded*/ false);
        }
        if let Some(cell) = self.transcript.active_cell.as_mut() {
            if let Some(exec) = cell.as_any_mut().downcast_mut::<ExecCell>() {
                exec.freeze_snapshot();
            } else if let Some(tool) = cell.as_any_mut().downcast_mut::<McpToolCallCell>() {
                tool.freeze_snapshot();
            }
        }
        self.bottom_pane
            .set_composer_input_enabled(/*enabled*/ false, /*placeholder*/ None);
        self.request_redraw();
    }

    pub(crate) fn is_external_writer_view(&self) -> bool {
        self.read_only_thread == Some(ReadOnlyThread::ExternalWriter)
    }

    pub(crate) fn is_archived_view(&self) -> bool {
        self.read_only_thread == Some(ReadOnlyThread::Archived)
    }

    pub(crate) fn is_read_only_view(&self) -> bool {
        self.read_only_thread.is_some()
    }
}
