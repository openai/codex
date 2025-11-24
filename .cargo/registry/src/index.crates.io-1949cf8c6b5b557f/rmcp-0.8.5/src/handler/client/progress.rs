use std::{collections::HashMap, sync::Arc};

use futures::{Stream, StreamExt};
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;

use crate::model::{ProgressNotificationParam, ProgressToken};
type Dispatcher =
    Arc<RwLock<HashMap<ProgressToken, tokio::sync::mpsc::Sender<ProgressNotificationParam>>>>;

/// A dispatcher for progress notifications.
#[derive(Debug, Clone, Default)]
pub struct ProgressDispatcher {
    pub(crate) dispatcher: Dispatcher,
}

impl ProgressDispatcher {
    const CHANNEL_SIZE: usize = 16;
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle a progress notification by sending it to the appropriate subscriber
    pub async fn handle_notification(&self, notification: ProgressNotificationParam) {
        let token = &notification.progress_token;
        if let Some(sender) = self.dispatcher.read().await.get(token).cloned() {
            let send_result = sender.send(notification).await;
            if let Err(e) = send_result {
                tracing::warn!("Failed to send progress notification: {e}");
            }
        }
    }

    /// Subscribe to progress notifications for a specific token.
    ///
    /// If you drop the returned `ProgressSubscriber`, it will automatically unsubscribe from notifications for that token.
    pub async fn subscribe(&self, progress_token: ProgressToken) -> ProgressSubscriber {
        let (sender, receiver) = tokio::sync::mpsc::channel(Self::CHANNEL_SIZE);
        self.dispatcher
            .write()
            .await
            .insert(progress_token.clone(), sender);
        let receiver = ReceiverStream::new(receiver);
        ProgressSubscriber {
            progress_token,
            receiver,
            dispatcher: self.dispatcher.clone(),
        }
    }

    /// Unsubscribe from progress notifications for a specific token.
    pub async fn unsubscribe(&self, token: &ProgressToken) {
        self.dispatcher.write().await.remove(token);
    }

    /// Clear all dispatcher.
    pub async fn clear(&self) {
        let mut dispatcher = self.dispatcher.write().await;
        dispatcher.clear();
    }
}

pub struct ProgressSubscriber {
    pub(crate) progress_token: ProgressToken,
    pub(crate) receiver: ReceiverStream<ProgressNotificationParam>,
    pub(crate) dispatcher: Dispatcher,
}

impl ProgressSubscriber {
    pub fn progress_token(&self) -> &ProgressToken {
        &self.progress_token
    }
}

impl Stream for ProgressSubscriber {
    type Item = ProgressNotificationParam;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.receiver.poll_next_unpin(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.receiver.size_hint()
    }
}

impl Drop for ProgressSubscriber {
    fn drop(&mut self) {
        let token = self.progress_token.clone();
        self.receiver.close();
        let dispatcher = self.dispatcher.clone();
        tokio::spawn(async move {
            let mut dispatcher = dispatcher.write_owned().await;
            dispatcher.remove(&token);
        });
    }
}
