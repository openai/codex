//! Prepare remote image attachments off the event loop, retaining the draft until completion.
//! Cancellation restores the draft; worker notifications from a retired submission are ignored.
//! Preparation is serialized so canceled decodes cannot pile up across retries or thread switches.

use super::*;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::snapshot_local_user_input;
use codex_protocol::user_input::UserInput as CoreUserInput;
use tokio::sync::oneshot;

static IMAGE_PREPARATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(super) struct PendingImageSubmission {
    id: uuid::Uuid,
    pub(super) message: UserMessage,
    history_record: UserMessageHistoryRecord,
    source: UserMessageSource,
    result: oneshot::Receiver<Result<Vec<UserInput>, String>>,
}

impl ChatWidget {
    pub(super) fn prepare_image_submission(
        &mut self,
        message: UserMessage,
        history_record: UserMessageHistoryRecord,
        source: UserMessageSource,
    ) {
        let id = uuid::Uuid::new_v4();
        let images = message.local_images.clone();
        let remote_bytes = message.remote_image_urls.iter().map(String::len).sum();
        let (mut tx, result) = oneshot::channel();
        let events = self.app_event_tx.clone();
        tokio::spawn(async move {
            let permit = tokio::select! {
                biased;
                _ = tx.closed() => return,
                permit = IMAGE_PREPARATION.lock() => permit,
            };
            let prepared = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                prepare_images(images, remote_bytes)
            })
            .await
            .unwrap_or_else(|error| Err(format!("Failed to prepare images: {error}")));
            if tx.send(prepared).is_ok() {
                events.send(AppEvent::ImagesPrepared(id));
            }
        });
        self.pending_image_submission = Some(PendingImageSubmission {
            id,
            message,
            history_record,
            source,
            result,
        });
        self.refresh_pending_input_preview();
        self.request_redraw();
    }

    pub(crate) fn on_images_prepared(&mut self, id: uuid::Uuid) {
        let Some(mut pending) = self
            .pending_image_submission
            .take_if(|pending| pending.id == id)
        else {
            return;
        };
        match pending
            .result
            .try_recv()
            .unwrap_or_else(|error| Err(format!("Failed to prepare images: {error}")))
        {
            Ok(images) => {
                let (accepted, _) = self.submit_user_message_with_prepared_images(
                    pending.message,
                    pending.history_record,
                    ShellEscapePolicy::Disallow,
                    pending.source,
                    Some(images),
                );
                if !accepted {
                    self.input_queue.recovered_queue |=
                        self.input_queue.has_queued_follow_up_messages();
                }
            }
            Err(error) => {
                self.input_queue.recovered_queue |=
                    self.input_queue.has_queued_follow_up_messages();
                self.add_error_message(error);
                self.restore_user_message_to_composer(user_message_for_restore(
                    pending.message,
                    &pending.history_record,
                ));
            }
        }
        self.refresh_pending_input_preview();
        self.request_redraw();
    }

    pub(super) fn requeue_image_submission(&mut self) {
        if let Some(pending) = self.pending_image_submission.take() {
            self.input_queue
                .queued_user_messages
                .push_front(QueuedUserMessage {
                    source: pending.source,
                    ..QueuedUserMessage::new(pending.message, QueuedInputAction::Literal)
                });
            self.input_queue
                .queued_user_message_history_records
                .push_front(pending.history_record);
            self.refresh_pending_input_preview();
            self.request_redraw();
        }
    }

    pub(super) fn cancel_image_submission(&mut self) -> bool {
        let Some(pending) = self.pending_image_submission.take() else {
            return false;
        };
        self.input_queue.recovered_queue |= self.input_queue.has_queued_follow_up_messages();
        self.restore_user_message_to_composer(user_message_for_restore(
            pending.message,
            &pending.history_record,
        ));
        self.refresh_pending_input_preview();
        self.request_redraw();
        true
    }
}

fn prepare_images(
    images: Vec<LocalImageAttachment>,
    remote_bytes: usize,
) -> Result<Vec<UserInput>, String> {
    // Leave headroom below the WebSocket server's 64 MiB message limit.
    const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
    let mut remaining_image_bytes = MAX_IMAGE_BYTES.saturating_sub(remote_bytes);
    images
        .into_iter()
        .map(|image| {
            let mut input = CoreUserInput::LocalImage {
                path: image.path.clone(),
                // Preserve source pixels; core applies the model-specific resize policy.
                detail: Some(ImageDetail::Original),
            };
            let prepared = (|| -> std::io::Result<()> {
                let too_large =
                    || std::io::Error::other("image attachments exceed the 32 MiB transport limit");
                if std::fs::metadata(&image.path)?.len() > (remaining_image_bytes / 4 * 3) as u64 {
                    return Err(too_large());
                }
                snapshot_local_user_input(&mut input)?;
                if let CoreUserInput::Image { image_url, .. } = &input {
                    remaining_image_bytes = remaining_image_bytes
                        .checked_sub(image_url.len())
                        .ok_or_else(too_large)?;
                }
                Ok(())
            })();
            prepared.map_err(|error| {
                format!("Failed to prepare image: {}: {error}", image.path.display())
            })?;
            let mut input = UserInput::from(input);
            if let UserInput::Image { detail, .. } = &mut input {
                *detail = None;
            }
            Ok(input)
        })
        .collect()
}
