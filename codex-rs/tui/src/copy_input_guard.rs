//! Keep queued submissions paused until the transcript selector releases its owner.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[derive(Debug)]
pub(crate) struct CopyInputGuard(pub(crate) AppEventSender);

impl Drop for CopyInputGuard {
    fn drop(&mut self) {
        self.0.send(AppEvent::TranscriptCopyClosed);
    }
}
