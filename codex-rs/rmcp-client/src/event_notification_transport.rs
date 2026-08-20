use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use rmcp::model::ClientRequest;
use rmcp::model::CustomNotification;
use rmcp::model::JsonRpcMessage;
use rmcp::model::RequestId;
use rmcp::model::ServerNotification;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::IntoTransport;
use rmcp::transport::Transport;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tracing::warn;

pub(crate) const MAX_EVENT_NOTIFICATION_BYTES: usize = 1024 * 1024;
const MAX_QUEUED_EVENT_NOTIFICATION_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct EventNotificationSender {
    notifications: mpsc::UnboundedSender<QueuedEventNotification>,
    available_bytes: Arc<Semaphore>,
}

pub struct EventNotificationReceiver {
    notifications: mpsc::UnboundedReceiver<QueuedEventNotification>,
    available_bytes: Arc<Semaphore>,
}

struct QueuedEventNotification {
    notification: CustomNotification,
    _bytes: OwnedSemaphorePermit,
}

pub(crate) fn event_notification_channel() -> (EventNotificationSender, EventNotificationReceiver) {
    let (notifications_tx, notifications_rx) = mpsc::unbounded_channel();
    let available_bytes = Arc::new(Semaphore::new(MAX_QUEUED_EVENT_NOTIFICATION_BYTES));

    (
        EventNotificationSender {
            notifications: notifications_tx,
            available_bytes: Arc::clone(&available_bytes),
        },
        EventNotificationReceiver {
            notifications: notifications_rx,
            available_bytes,
        },
    )
}

impl EventNotificationSender {
    fn send(&self, notification: CustomNotification, notification_bytes: usize) -> Result<(), ()> {
        let bytes = u32::try_from(notification_bytes).map_err(|_| ())?;
        let permit = Arc::clone(&self.available_bytes)
            .try_acquire_many_owned(bytes)
            .map_err(|_| ())?;
        self.notifications
            .send(QueuedEventNotification {
                notification,
                _bytes: permit,
            })
            .map_err(|_| ())
    }

    fn close(&self) {
        self.available_bytes.close();
    }
}

impl EventNotificationReceiver {
    pub async fn recv(&mut self) -> Option<CustomNotification> {
        self.notifications
            .recv()
            .await
            .map(|queued| queued.notification)
    }
}

impl Drop for EventNotificationReceiver {
    fn drop(&mut self) {
        self.available_bytes.close();
    }
}

/// Consumes Plugin Runtime event notifications before rmcp can log their payloads.
pub(crate) fn capture_event_notifications<T, E, A>(
    transport: T,
) -> impl Transport<RoleClient, Error = E> + 'static
where
    T: IntoTransport<RoleClient, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    EventNotificationTransport {
        inner: transport.into_transport(),
        routes: Arc::default(),
    }
}

struct EventNotificationTransport<T> {
    inner: T,
    routes: Arc<Mutex<HashMap<RequestId, EventNotificationSender>>>,
}

impl<T> Transport<RoleClient> for EventNotificationTransport<T>
where
    T: Transport<RoleClient> + 'static,
{
    type Error = T::Error;

    fn send(
        &mut self,
        mut message: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let request_route = match &mut message {
            JsonRpcMessage::Request(envelope) => match &mut envelope.request {
                ClientRequest::CustomRequest(request) if request.method == "events/stream" => {
                    request
                        .extensions
                        .remove::<EventNotificationSender>()
                        .map(|sender| (envelope.id.clone(), sender))
                }
                _ => None,
            },
            JsonRpcMessage::Notification(envelope) => {
                if let rmcp::model::ClientNotification::CancelledNotification(cancelled) =
                    &envelope.notification
                    && let Some(request_id) = cancelled.params.request_id.as_ref()
                    && let Some(route) = self
                        .routes
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .remove(request_id)
                {
                    route.close();
                }
                None
            }
            JsonRpcMessage::Response(_) | JsonRpcMessage::Error(_) => None,
        };

        let route_id = request_route
            .as_ref()
            .map(|(request_id, _)| request_id.clone());
        if let Some((request_id, sender)) = request_route
            && let Some(previous) = self
                .routes
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(request_id, sender)
        {
            previous.close();
        }

        let routes = Arc::clone(&self.routes);
        let send = self.inner.send(message);
        async move {
            let result = send.await;
            if result.is_err()
                && let Some(route_id) = route_id
                && let Some(route) = routes
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&route_id)
            {
                route.close();
            }
            result
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        loop {
            let message = self.inner.receive().await?;
            if let Some(request_id) = response_id(&message) {
                if let Some(route) = self
                    .routes
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(request_id)
                {
                    route.close();
                }
                return Some(message);
            }

            let JsonRpcMessage::Notification(envelope) = &message else {
                return Some(message);
            };
            let ServerNotification::CustomNotification(notification) = &envelope.notification
            else {
                return Some(message);
            };
            if !notification.method.starts_with("notifications/events/") {
                return Some(message);
            }
            let Some(subscription_id) =
                rmcp::model::GetMeta::get_meta(notification).subscription_id()
            else {
                continue;
            };

            let route = self
                .routes
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&subscription_id)
                .cloned();
            let Some(route) = route else {
                continue;
            };
            let notification_bytes = serde_json::to_vec(&message)
                .map(|message| message.len())
                .unwrap_or(usize::MAX);
            if notification_bytes > MAX_EVENT_NOTIFICATION_BYTES {
                warn!(
                    notification_bytes,
                    "discarding oversized MCP event notification"
                );
                continue;
            }
            if route
                .send(notification.clone(), notification_bytes)
                .is_err()
                && let Some(route) = self
                    .routes
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&subscription_id)
            {
                let _ = route.send(
                    CustomNotification::new(
                        "notifications/events/terminated",
                        /*params*/ None,
                    ),
                    /*notification_bytes*/ 0,
                );
                route.close();
            }
        }
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        let routes =
            std::mem::take(&mut *self.routes.lock().unwrap_or_else(PoisonError::into_inner));
        for route in routes.into_values() {
            route.close();
        }
        self.inner.close().await
    }
}

fn response_id(message: &RxJsonRpcMessage<RoleClient>) -> Option<&RequestId> {
    match message {
        JsonRpcMessage::Response(response) => Some(&response.id),
        JsonRpcMessage::Error(error) => error.id.as_ref(),
        JsonRpcMessage::Request(_) | JsonRpcMessage::Notification(_) => None,
    }
}
