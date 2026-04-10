use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use codex_app_server_protocol::ClientRequestSerializationScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::Mutex;
use tracing::Instrument;

use crate::outgoing_message::ConnectionId;

type BoxFutureUnit = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RequestSerializationQueueKey {
    Global(&'static str),
    Thread {
        thread_id: String,
    },
    ThreadPath {
        path: PathBuf,
    },
    CommandExecProcess {
        connection_id: ConnectionId,
        process_id: String,
    },
    FuzzyFileSearchSession {
        session_id: String,
    },
    FsWatch {
        connection_id: ConnectionId,
        watch_id: String,
    },
    Plugin {
        marketplace_path: AbsolutePathBuf,
        plugin_name: String,
    },
    PluginId {
        plugin_id: String,
    },
    McpOauth {
        server_name: String,
    },
}

impl RequestSerializationQueueKey {
    pub(crate) fn from_scope(
        connection_id: ConnectionId,
        scope: ClientRequestSerializationScope,
    ) -> Self {
        match scope {
            ClientRequestSerializationScope::Global(name) => Self::Global(name),
            ClientRequestSerializationScope::Thread { thread_id } => Self::Thread { thread_id },
            ClientRequestSerializationScope::ThreadPath { path } => Self::ThreadPath { path },
            ClientRequestSerializationScope::CommandExecProcess { process_id } => {
                Self::CommandExecProcess {
                    connection_id,
                    process_id,
                }
            }
            ClientRequestSerializationScope::FuzzyFileSearchSession { session_id } => {
                Self::FuzzyFileSearchSession { session_id }
            }
            ClientRequestSerializationScope::FsWatch { watch_id } => Self::FsWatch {
                connection_id,
                watch_id,
            },
            ClientRequestSerializationScope::Plugin {
                marketplace_path,
                plugin_name,
            } => Self::Plugin {
                marketplace_path,
                plugin_name,
            },
            ClientRequestSerializationScope::PluginId { plugin_id } => Self::PluginId { plugin_id },
            ClientRequestSerializationScope::McpOauth { server_name } => {
                Self::McpOauth { server_name }
            }
        }
    }
}

pub(crate) struct QueuedInitializedRequest {
    future: BoxFutureUnit,
}

impl QueuedInitializedRequest {
    pub(crate) fn new(future: impl Future<Output = ()> + Send + 'static) -> Self {
        Self {
            future: Box::pin(future),
        }
    }

    pub(crate) async fn run(self) {
        self.future.await;
    }
}

#[derive(Clone, Default)]
pub(crate) struct RequestSerializationQueues {
    inner: Arc<Mutex<HashMap<RequestSerializationQueueKey, VecDeque<QueuedInitializedRequest>>>>,
}

impl RequestSerializationQueues {
    pub(crate) async fn enqueue(
        &self,
        key: RequestSerializationQueueKey,
        request: QueuedInitializedRequest,
    ) {
        let should_spawn = {
            let mut queues = self.inner.lock().await;
            match queues.get_mut(&key) {
                Some(queue) => {
                    queue.push_back(request);
                    false
                }
                None => {
                    let mut queue = VecDeque::new();
                    queue.push_back(request);
                    queues.insert(key.clone(), queue);
                    true
                }
            }
        };

        if should_spawn {
            let queues = self.clone();
            let span = tracing::debug_span!("app_server.serialized_request_queue", ?key);
            tokio::spawn(async move { queues.drain(key).await }.instrument(span));
        }
    }

    async fn drain(self, key: RequestSerializationQueueKey) {
        loop {
            let request = {
                let mut queues = self.inner.lock().await;
                let Some(queue) = queues.get_mut(&key) else {
                    return;
                };
                match queue.pop_front() {
                    Some(request) => request,
                    None => {
                        queues.remove(&key);
                        return;
                    }
                }
            };

            request.run().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn same_key_requests_run_fifo() {
        let queues = RequestSerializationQueues::default();
        let key = RequestSerializationQueueKey::Global("test");
        let (tx, mut rx) = mpsc::unbounded_channel();

        for value in [1, 2, 3] {
            let tx = tx.clone();
            queues
                .enqueue(
                    key.clone(),
                    QueuedInitializedRequest::new(async move {
                        tx.send(value).expect("receiver should be open");
                    }),
                )
                .await;
        }
        drop(tx);

        let mut values = Vec::new();
        while let Some(value) = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for queued request")
        {
            values.push(value);
        }

        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn different_keys_run_concurrently() {
        let queues = RequestSerializationQueues::default();
        let (blocked_tx, blocked_rx) = oneshot::channel::<()>();
        let (ran_tx, ran_rx) = oneshot::channel::<()>();

        queues
            .enqueue(
                RequestSerializationQueueKey::Global("blocked"),
                QueuedInitializedRequest::new(async move {
                    let _ = blocked_rx.await;
                }),
            )
            .await;
        queues
            .enqueue(
                RequestSerializationQueueKey::Global("other"),
                QueuedInitializedRequest::new(async move {
                    ran_tx.send(()).expect("receiver should be open");
                }),
            )
            .await;

        timeout(Duration::from_secs(1), ran_rx)
            .await
            .expect("other key should not be blocked")
            .expect("sender should be open");
        blocked_tx
            .send(())
            .expect("blocked request should be waiting");
    }
}
