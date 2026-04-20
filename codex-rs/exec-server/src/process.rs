use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio::sync::watch;

use crate::ExecServerError;
use crate::ProcessId;
use crate::protocol::ExecParams;
use crate::protocol::ProcessOutputChunk;
use crate::protocol::ReadResponse;
use crate::protocol::WriteResponse;

pub struct StartedExecProcess {
    pub process: Arc<dyn ExecProcess>,
}

/// Pushed process events for consumers that want to follow process output as it
/// arrives instead of polling retained output with [`ExecProcess::read`].
///
/// The stream is scoped to one [`ExecProcess`] handle. `Output` events carry
/// stdout, stderr, or pty bytes. `Exited` reports the process exit status, while
/// `Closed` means all output streams have ended and no more output events will
/// arrive. `Failed` is used when the process session cannot continue, for
/// example because the remote executor connection disconnected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecProcessEvent {
    Output(ProcessOutputChunk),
    Exited { seq: u64, exit_code: i32 },
    Closed { seq: u64 },
    Failed(String),
}

/// Replay buffer plus live fan-out for pushed process events.
///
/// New subscribers first drain a bounded replay history, then continue on the
/// live broadcast channel. The history is bounded by event count and retained
/// output bytes: count protects against many tiny events, while bytes protects
/// against a few very large output chunks.
#[derive(Clone)]
pub(crate) struct ExecProcessEventLog {
    inner: Arc<ExecProcessEventLogInner>,
}

struct ExecProcessEventLogInner {
    history: StdMutex<ExecProcessEventHistory>,
    live_tx: broadcast::Sender<ExecProcessEvent>,
    event_capacity: usize,
    byte_capacity: usize,
}

#[derive(Default)]
struct ExecProcessEventHistory {
    events: VecDeque<ExecProcessEvent>,
    retained_bytes: usize,
}

impl ExecProcessEvent {
    /// Sequence number used to order process-owned events.
    ///
    /// `Failed` is intentionally unsequenced because it is synthesized by the
    /// client when the session or transport fails, not emitted by the process.
    pub(crate) fn seq(&self) -> Option<u64> {
        match self {
            ExecProcessEvent::Output(chunk) => Some(chunk.seq),
            ExecProcessEvent::Exited { seq, .. } | ExecProcessEvent::Closed { seq } => Some(*seq),
            ExecProcessEvent::Failed(_) => None,
        }
    }

    fn retained_len(&self) -> usize {
        match self {
            ExecProcessEvent::Output(chunk) => chunk.chunk.0.len(),
            ExecProcessEvent::Failed(message) => message.len(),
            ExecProcessEvent::Exited { .. } | ExecProcessEvent::Closed { .. } => 0,
        }
    }
}

impl ExecProcessEventLog {
    pub(crate) fn new(event_capacity: usize, byte_capacity: usize) -> Self {
        let (live_tx, _live_rx) = broadcast::channel(event_capacity);
        Self {
            inner: Arc::new(ExecProcessEventLogInner {
                history: StdMutex::new(ExecProcessEventHistory::default()),
                live_tx,
                event_capacity,
                byte_capacity,
            }),
        }
    }

    pub(crate) fn publish(&self, event: ExecProcessEvent) {
        let mut history = self
            .inner
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        history.retained_bytes += event.retained_len();
        history.events.push_back(event.clone());
        while history.events.len() > self.inner.event_capacity
            || history.retained_bytes > self.inner.byte_capacity
        {
            let Some(evicted) = history.events.pop_front() else {
                break;
            };
            history.retained_bytes = history
                .retained_bytes
                .saturating_sub(evicted.retained_len());
        }

        let _ = self.inner.live_tx.send(event);
    }

    pub(crate) fn subscribe(&self) -> ExecProcessEventReceiver {
        let history = self
            .inner
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let live_rx = self.inner.live_tx.subscribe();
        let replay = history.events.iter().cloned().collect();

        ExecProcessEventReceiver { replay, live_rx }
    }

    /// Builds a polling-style read response from locally retained pushed
    /// events.
    ///
    /// Remote process reads normally go back to the executor. After the
    /// executor transport closes, the client may still have ordered output,
    /// exit, and closed notifications queued locally. This lets the polling
    /// read path surface those retained events before reporting the synthesized
    /// transport failure.
    pub(crate) fn read_retained(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        failure: Option<String>,
    ) -> ReadResponse {
        let after_seq = after_seq.unwrap_or(0);
        let max_bytes = max_bytes.unwrap_or(usize::MAX);
        let history = self
            .inner
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut chunks = Vec::new();
        let mut total_bytes = 0;
        let mut next_seq = after_seq.saturating_add(1);
        let mut exited = false;
        let mut exit_code = None;
        let mut closed = false;

        for event in history.events.iter() {
            match event {
                ExecProcessEvent::Output(chunk) if chunk.seq > after_seq => {
                    let chunk_len = chunk.chunk.0.len();
                    if !chunks.is_empty() && total_bytes + chunk_len > max_bytes {
                        break;
                    }
                    total_bytes += chunk_len;
                    chunks.push(chunk.clone());
                    next_seq = chunk.seq.saturating_add(1);
                    if total_bytes >= max_bytes {
                        break;
                    }
                }
                ExecProcessEvent::Exited {
                    seq,
                    exit_code: code,
                } if *seq > after_seq => {
                    next_seq = next_seq.max(seq.saturating_add(1));
                    exited = true;
                    exit_code = Some(*code);
                }
                ExecProcessEvent::Closed { seq } if *seq > after_seq => {
                    next_seq = next_seq.max(seq.saturating_add(1));
                    closed = true;
                }
                ExecProcessEvent::Output(_)
                | ExecProcessEvent::Exited { .. }
                | ExecProcessEvent::Closed { .. }
                | ExecProcessEvent::Failed(_) => {}
            }
        }

        if failure.is_some() {
            exited = true;
            closed = true;
        }

        ReadResponse {
            chunks,
            next_seq,
            exited,
            exit_code,
            closed,
            failure,
        }
    }
}

pub struct ExecProcessEventReceiver {
    replay: VecDeque<ExecProcessEvent>,
    live_rx: broadcast::Receiver<ExecProcessEvent>,
}

impl ExecProcessEventReceiver {
    pub fn empty() -> Self {
        let (_live_tx, live_rx) = broadcast::channel(1);
        Self {
            replay: VecDeque::new(),
            live_rx,
        }
    }

    /// Returns the next replayed or live event.
    ///
    /// `Lagged` means this receiver fell behind the bounded live channel. The
    /// caller should recover through [`ExecProcess::read`] using the last
    /// delivered sequence number, then continue receiving pushed events.
    pub async fn recv(&mut self) -> Result<ExecProcessEvent, broadcast::error::RecvError> {
        if let Some(event) = self.replay.pop_front() {
            return Ok(event);
        }

        self.live_rx.recv().await
    }
}

/// Handle for an executor-managed process.
///
/// Implementations must support both retained-output reads and pushed events:
/// `read` is the request/response API for callers that want to page through
/// buffered output, while `subscribe_events` is the streaming API for callers
/// that want output and lifecycle changes delivered as they happen.
#[async_trait]
pub trait ExecProcess: Send + Sync {
    fn process_id(&self) -> &ProcessId;

    fn subscribe_wake(&self) -> watch::Receiver<u64>;

    fn subscribe_events(&self) -> ExecProcessEventReceiver;

    async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError>;

    async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError>;

    async fn terminate(&self) -> Result<(), ExecServerError>;
}

#[async_trait]
pub trait ExecBackend: Send + Sync {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError>;
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::timeout;

    use super::ExecProcessEvent;
    use super::ExecProcessEventLog;
    use crate::protocol::ExecOutputStream;
    use crate::protocol::ProcessOutputChunk;
    use crate::protocol::ReadResponse;

    #[tokio::test]
    async fn event_history_replay_is_bounded_by_retained_bytes() {
        let log = ExecProcessEventLog::new(/*event_capacity*/ 8, /*byte_capacity*/ 3);

        log.publish(ExecProcessEvent::Output(ProcessOutputChunk {
            seq: 1,
            stream: ExecOutputStream::Stdout,
            chunk: b"large".to_vec().into(),
        }));
        log.publish(ExecProcessEvent::Exited {
            seq: 2,
            exit_code: 0,
        });
        log.publish(ExecProcessEvent::Closed { seq: 3 });

        let mut events = log.subscribe();
        let replay = vec![
            timeout(Duration::from_secs(1), events.recv())
                .await
                .expect("exit event replay should not time out")
                .expect("exit event replay should be available"),
            timeout(Duration::from_secs(1), events.recv())
                .await
                .expect("closed event replay should not time out")
                .expect("closed event replay should be available"),
        ];

        assert_eq!(
            replay,
            vec![
                ExecProcessEvent::Exited {
                    seq: 2,
                    exit_code: 0,
                },
                ExecProcessEvent::Closed { seq: 3 },
            ]
        );
    }

    #[test]
    fn event_history_read_retained_returns_tail_before_failure() {
        let log = ExecProcessEventLog::new(/*event_capacity*/ 8, /*byte_capacity*/ 1024);
        log.publish(ExecProcessEvent::Output(ProcessOutputChunk {
            seq: 1,
            stream: ExecOutputStream::Stdout,
            chunk: b"already-read".to_vec().into(),
        }));
        log.publish(ExecProcessEvent::Output(ProcessOutputChunk {
            seq: 2,
            stream: ExecOutputStream::Stdout,
            chunk: b"tail".to_vec().into(),
        }));
        log.publish(ExecProcessEvent::Exited {
            seq: 3,
            exit_code: 0,
        });
        log.publish(ExecProcessEvent::Closed { seq: 4 });

        let response = log.read_retained(
            Some(1),
            /*max_bytes*/ None,
            Some("exec-server transport disconnected".to_string()),
        );

        assert_eq!(
            response,
            ReadResponse {
                chunks: vec![ProcessOutputChunk {
                    seq: 2,
                    stream: ExecOutputStream::Stdout,
                    chunk: b"tail".to_vec().into(),
                }],
                next_seq: 5,
                exited: true,
                exit_code: Some(0),
                closed: true,
                failure: Some("exec-server transport disconnected".to_string()),
            }
        );
    }
}
