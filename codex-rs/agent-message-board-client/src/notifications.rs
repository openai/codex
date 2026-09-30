//! Receives live board previews for an active turn using the extension's turn hooks.
//! Provisioning and durable board ownership belong to the research host.

use crate::RemoteAgentMessageBoard;
use codex_agent_message_board_extension::MessageBoardHost;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnErrorInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;
use codex_extension_api::TurnStopInput;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErrKind;
use futures::channel::oneshot;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::task::AbortOnDropHandle;

type Binding = (Arc<RemoteAgentMessageBoard>, Arc<dyn MessageBoardHost>);
type ResolveBinding = dyn Fn(&TurnStartInput<'_>) -> Option<Binding> + Send + Sync;

struct Notifications(Box<ResolveBinding>);
struct Receiver {
    _task: AbortOnDropHandle<()>,
}

impl TurnLifecycleContributor for Notifications {
    fn turn_start_phase(&self, _thread_store: &ExtensionData) -> TurnStartPhase {
        TurnStartPhase::RegularTaskStart
    }

    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let Some((board, host)) = (self.0)(&input) else {
                return;
            };
            let Ok(caller) = ThreadId::from_string(input.thread_store.level_id()) else {
                return;
            };
            let turn_id = input.turn_id.to_owned();
            let (first_attempt_tx, first_attempt_rx) = oneshot::channel();
            input.turn_store.insert(Receiver {
                _task: AbortOnDropHandle::new(tokio::spawn(async move {
                    let mut first_attempt_tx = Some(first_attempt_tx);
                    loop {
                        let connection = board.notifications(caller, turn_id.clone()).await;
                        if let Some(first_attempt_tx) = first_attempt_tx.take() {
                            let _ = first_attempt_tx.send(());
                        }
                        match connection {
                            Ok(mut stream) => loop {
                                match stream.next().await {
                                    Ok(Some(notice)) => {
                                        if let Err(error) = host.notify(caller, notice.post).await {
                                            tracing::warn!(
                                                %caller, %turn_id,
                                                error_kind = ?CodexErrKind::from(&error),
                                                "Remote board notification delivery failed"
                                            );
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(error) => {
                                        tracing::warn!(
                                            %caller, %turn_id,
                                            error_kind = ?CodexErrKind::from(&error),
                                            "Remote board notification stream failed"
                                        );
                                        break;
                                    }
                                }
                            },
                            Err(error) => {
                                tracing::warn!(
                                    %caller, %turn_id,
                                    error_kind = ?CodexErrKind::from(&error),
                                    "Remote board notification connection failed"
                                );
                            }
                        }
                        // Reopen for future previews; the service does not replay missed ones.
                        tokio::time::sleep(Duration::from_secs(/*secs*/ 1)).await;
                    }
                })),
            });
            // A healthy service is listening before inference. A failed first attempt
            // releases startup while the same turn-owned receiver keeps retrying.
            let _ = first_attempt_rx.await;
        })
    }

    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.turn_store.remove::<Receiver>();
        })
    }

    fn on_turn_abort<'a>(&'a self, input: TurnAbortInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.turn_store.remove::<Receiver>();
        })
    }

    fn on_turn_error<'a>(&'a self, input: TurnErrorInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.turn_store.remove::<Receiver>();
        })
    }
}

/// Installs live notification delivery. The host resolves the existing remote
/// client and a delivery host bound to this exact turn; idle agents are never woken.
/// The turn store owns the receiver and cancels it when the turn ends.
pub fn install_notifications<C: Sync>(
    registry: &mut ExtensionRegistryBuilder<C>,
    resolve: impl Fn(&TurnStartInput<'_>) -> Option<Binding> + Send + Sync + 'static,
) {
    registry.turn_lifecycle_contributor(Arc::new(Notifications(Box::new(resolve))));
}
