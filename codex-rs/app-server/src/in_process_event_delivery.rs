use std::collections::HashMap;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::error_code::OVERLOADED_ERROR_CODE;
use crate::error_code::internal_error;
use crate::in_process::InProcessEventDelivery;
use crate::in_process::InProcessServerEvent;
use crate::in_process::PendingClientRequestResponse;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::QueuedOutgoingMessage;

pub(crate) fn server_notification_requires_delivery(notification: &ServerNotification) -> bool {
    matches!(
        notification,
        ServerNotification::TurnCompleted(_)
            | ServerNotification::ThreadSettingsUpdated(_)
            | ServerNotification::ExternalAgentConfigImportCompleted(_)
    )
}

async fn send_server_request(
    event_tx: &mpsc::Sender<InProcessServerEvent>,
    outgoing: Option<&OutgoingMessageSender>,
    request: ServerRequest,
    delivery: InProcessEventDelivery,
) -> bool {
    let send_error = match delivery {
        InProcessEventDelivery::BestEffort => event_tx
            .try_send(InProcessServerEvent::ServerRequest(request))
            .err()
            .map(|error| match error {
                mpsc::error::TrySendError::Full(event) => (event, true, true),
                mpsc::error::TrySendError::Closed(event) => (event, false, true),
            }),
        InProcessEventDelivery::Lossless => event_tx
            .send(InProcessServerEvent::ServerRequest(request))
            .await
            .err()
            .map(|error| (error.0, false, false)),
    };

    let Some((event, queue_full, keep_running)) = send_error else {
        return true;
    };
    let InProcessServerEvent::ServerRequest(request) = event else {
        unreachable!("server request delivery returned a different event variant");
    };
    let error = if queue_full {
        JSONRPCErrorError {
            code: OVERLOADED_ERROR_CODE,
            message: "in-process server request queue is full".to_string(),
            data: None,
        }
    } else {
        internal_error("in-process server request consumer is closed")
    };
    if let Some(outgoing) = outgoing {
        outgoing
            .notify_client_error(request.id().clone(), error)
            .await;
    }
    keep_running
}

async fn send_server_notification(
    event_tx: &mpsc::Sender<InProcessServerEvent>,
    notification: ServerNotification,
    delivery: InProcessEventDelivery,
) -> bool {
    if matches!(delivery, InProcessEventDelivery::Lossless)
        || server_notification_requires_delivery(&notification)
    {
        return event_tx
            .send(InProcessServerEvent::ServerNotification(notification))
            .await
            .is_ok();
    }

    match event_tx.try_send(InProcessServerEvent::ServerNotification(notification)) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            warn!("dropping in-process server notification (queue full)");
            true
        }
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
}

pub(crate) async fn route_queued_message(
    queued_message: QueuedOutgoingMessage,
    pending_request_responses: &mut HashMap<
        RequestId,
        oneshot::Sender<PendingClientRequestResponse>,
    >,
    event_tx: &mpsc::Sender<InProcessServerEvent>,
    outgoing: Option<&OutgoingMessageSender>,
    delivery: InProcessEventDelivery,
) -> bool {
    let keep_running = match queued_message.message {
        OutgoingMessage::Response(response) => {
            if let Some(response_tx) = pending_request_responses.remove(&response.id) {
                let _ = response_tx.send(Ok(response.result));
            } else {
                warn!(
                    request_id = ?response.id,
                    "dropping unmatched in-process response"
                );
            }
            true
        }
        OutgoingMessage::Error(error) => {
            if let Some(response_tx) = pending_request_responses.remove(&error.id) {
                let _ = response_tx.send(Err(error.error));
            } else {
                warn!(
                    request_id = ?error.id,
                    "dropping unmatched in-process error response"
                );
            }
            true
        }
        OutgoingMessage::Request(request) => {
            send_server_request(event_tx, outgoing, request, delivery).await
        }
        OutgoingMessage::AppServerNotification(notification) => {
            send_server_notification(event_tx, notification, delivery).await
        }
    };

    if let Some(write_complete_tx) = queued_message.write_complete_tx {
        let _ = write_complete_tx.send(());
    }
    keep_running
}

pub(crate) async fn drain_writer_until_task_finishes(
    task: &mut JoinHandle<()>,
    writer_rx: &mut mpsc::Receiver<QueuedOutgoingMessage>,
    pending_request_responses: &mut HashMap<
        RequestId,
        oneshot::Sender<PendingClientRequestResponse>,
    >,
    event_tx: &mpsc::Sender<InProcessServerEvent>,
    outgoing: Option<&OutgoingMessageSender>,
    delivery: InProcessEventDelivery,
) -> bool {
    let mut writer_open = true;
    loop {
        tokio::select! {
            biased;
            _ = &mut *task => return true,
            queued_message = writer_rx.recv(), if writer_open => {
                match queued_message {
                    Some(queued_message) => {
                        if !route_queued_message(
                            queued_message,
                            pending_request_responses,
                            event_tx,
                            outgoing,
                            delivery,
                        )
                        .await
                        {
                            return false;
                        }
                    }
                    None => writer_open = false,
                }
            }
        }
    }
}

pub(crate) async fn drain_writer(
    writer_rx: &mut mpsc::Receiver<QueuedOutgoingMessage>,
    pending_request_responses: &mut HashMap<
        RequestId,
        oneshot::Sender<PendingClientRequestResponse>,
    >,
    event_tx: &mpsc::Sender<InProcessServerEvent>,
    outgoing: Option<&OutgoingMessageSender>,
    delivery: InProcessEventDelivery,
) -> bool {
    while let Some(queued_message) = writer_rx.recv().await {
        if !route_queued_message(
            queued_message,
            pending_request_responses,
            event_tx,
            outgoing,
            delivery,
        )
        .await
        {
            return false;
        }
    }
    true
}
