use std::{
    pin::Pin,
    sync::Arc,
    task::{Poll, ready},
    time::Duration,
};

use futures::{Stream, stream::BoxStream};
use sse_stream::{Error as SseError, Sse};

use crate::model::ServerJsonRpcMessage;

pub type BoxedSseResponse = BoxStream<'static, Result<Sse, SseError>>;

pub trait SseRetryPolicy: std::fmt::Debug + Send + Sync {
    fn retry(&self, current_times: usize) -> Option<Duration>;
}

#[derive(Debug, Clone)]
pub struct FixedInterval {
    pub max_times: Option<usize>,
    pub duration: Duration,
}

impl SseRetryPolicy for FixedInterval {
    fn retry(&self, current_times: usize) -> Option<Duration> {
        if let Some(max_times) = self.max_times {
            if current_times >= max_times {
                return None;
            }
        }
        Some(self.duration)
    }
}

impl FixedInterval {
    pub const DEFAULT_MIN_DURATION: Duration = Duration::from_millis(1000);
}

impl Default for FixedInterval {
    fn default() -> Self {
        Self {
            max_times: None,
            duration: Self::DEFAULT_MIN_DURATION,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    pub max_times: Option<usize>,
    pub base_duration: Duration,
}

impl ExponentialBackoff {
    pub const DEFAULT_DURATION: Duration = Duration::from_millis(1000);
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            max_times: None,
            base_duration: Self::DEFAULT_DURATION,
        }
    }
}

impl SseRetryPolicy for ExponentialBackoff {
    fn retry(&self, current_times: usize) -> Option<Duration> {
        if let Some(max_times) = self.max_times {
            if current_times >= max_times {
                return None;
            }
        }
        Some(self.base_duration * (2u32.pow(current_times as u32)))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NeverRetry;

impl SseRetryPolicy for NeverRetry {
    fn retry(&self, _current_times: usize) -> Option<Duration> {
        None
    }
}

#[derive(Debug, Default)]
pub struct NeverReconnect<E> {
    error: Option<E>,
}

impl<E: std::error::Error + Send> SseStreamReconnect for NeverReconnect<E> {
    type Error = E;
    type Future = futures::future::Ready<Result<BoxedSseResponse, Self::Error>>;
    fn retry_connection(&mut self, _last_event_id: Option<&str>) -> Self::Future {
        futures::future::ready(Err(self.error.take().expect("should not be called again")))
    }
}

/// Abstraction for SSE reconnection logic. Implementors can hook into
/// [`handle_control_event`](Self::handle_control_event) to consume control
/// frames (e.g. `event: endpoint`) that arrive when a server restarts an SSE
/// stream. The default implementation is a no-op, keeping existing behaviour
/// intact.
pub(crate) trait SseStreamReconnect {
    type Error: std::error::Error;
    type Future: Future<Output = Result<BoxedSseResponse, Self::Error>> + Send;
    fn retry_connection(&mut self, last_event_id: Option<&str>) -> Self::Future;
    fn handle_control_event(&mut self, _event: &Sse) -> Result<(), Self::Error> {
        Ok(())
    }
    fn handle_stream_error(
        &mut self,
        error: &(dyn std::error::Error + 'static),
        last_event_id: Option<&str>,
    ) {
        if let Some(id) = last_event_id {
            tracing::warn!(%id, "sse stream error: {error}");
        } else {
            tracing::warn!("sse stream error: {error}");
        }
    }
}

pin_project_lite::pin_project! {
    pub(crate) struct SseAutoReconnectStream<R>
    where R: SseStreamReconnect
     {
        retry_policy: Arc<dyn SseRetryPolicy>,
        last_event_id: Option<String>,
        server_retry_interval: Option<Duration>,
        connector: R,
        #[pin]
        state: SseAutoReconnectStreamState<R::Future>,
    }
}

impl<R: SseStreamReconnect> SseAutoReconnectStream<R> {
    pub fn new(
        stream: BoxedSseResponse,
        connector: R,
        retry_policy: Arc<dyn SseRetryPolicy>,
    ) -> Self {
        Self {
            retry_policy,
            last_event_id: None,
            server_retry_interval: None,
            connector,
            state: SseAutoReconnectStreamState::Connected { stream },
        }
    }
}

impl<E: std::error::Error + Send> SseAutoReconnectStream<NeverReconnect<E>> {
    #[allow(dead_code)]
    pub(crate) fn never_reconnect(stream: BoxedSseResponse, error_when_reconnect: E) -> Self {
        Self {
            retry_policy: Arc::new(NeverRetry),
            last_event_id: None,
            server_retry_interval: None,
            connector: NeverReconnect {
                error: Some(error_when_reconnect),
            },
            state: SseAutoReconnectStreamState::Connected { stream },
        }
    }
}

pin_project_lite::pin_project! {
    #[project = SseAutoReconnectStreamStateProj]
    pub enum SseAutoReconnectStreamState<F> {
        Connected {
            #[pin]
            stream: BoxedSseResponse,
        },
        Retrying {
            retry_times: usize,
            #[pin]
            retrying: F,
        },
        WaitingNextRetry {
            #[pin]
            sleep: tokio::time::Sleep,
            retry_times: usize,
        },
        Terminated,
    }
}

impl<R> Stream for SseAutoReconnectStream<R>
where
    R: SseStreamReconnect,
{
    type Item = Result<ServerJsonRpcMessage, R::Error>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        // let this_state = this.state.as_mut().project()
        let state = this.state.as_mut().project();
        let next_state = match state {
            SseAutoReconnectStreamStateProj::Connected { stream } => {
                match ready!(stream.poll_next(cx)) {
                    Some(Ok(sse)) => {
                        if let Some(new_server_retry) = sse.retry {
                            *this.server_retry_interval =
                                Some(Duration::from_millis(new_server_retry));
                        }
                        if let Some(ref event_id) = sse.id {
                            *this.last_event_id = Some(event_id.clone());
                        }
                        // Only treat blank/`message` events as JSON-RPC payloads.
                        // Other control frames (endpoint, ping, etc.) are passed to
                        // the reconnection handler.
                        let is_message_event =
                            matches!(sse.event.as_deref(), None | Some("") | Some("message"));
                        if !is_message_event {
                            match this.connector.handle_control_event(&sse) {
                                Ok(()) => return self.poll_next(cx),
                                Err(e) => {
                                    this.state.set(SseAutoReconnectStreamState::Terminated);
                                    return Poll::Ready(Some(Err(e)));
                                }
                            }
                        }
                        if let Some(data) = sse.data {
                            match serde_json::from_str::<ServerJsonRpcMessage>(&data) {
                                Err(e) => {
                                    // Downgrade to debug to avoid noisy logs when servers emit
                                    // non-JSON payloads as message frames. Include last_event_id
                                    // to aid troubleshooting while keeping default behaviour.
                                    let last_id = this.last_event_id.as_deref().unwrap_or("");
                                    tracing::debug!(last_event_id=%last_id, "failed to deserialize server message: {e}");
                                    return self.poll_next(cx);
                                }
                                Ok(message) => {
                                    return Poll::Ready(Some(Ok(message)));
                                }
                            };
                        } else {
                            return self.poll_next(cx);
                        }
                    }
                    Some(Err(e)) => {
                        this.connector
                            .handle_stream_error(&e, this.last_event_id.as_deref());
                        let retrying = this
                            .connector
                            .retry_connection(this.last_event_id.as_deref());
                        SseAutoReconnectStreamState::Retrying {
                            retry_times: 0,
                            retrying,
                        }
                    }
                    None => {
                        tracing::debug!("sse stream terminated");
                        return Poll::Ready(None);
                    }
                }
            }
            SseAutoReconnectStreamStateProj::Retrying {
                retry_times,
                retrying,
            } => {
                let retry_result = ready!(retrying.poll(cx));
                match retry_result {
                    Ok(new_stream) => SseAutoReconnectStreamState::Connected { stream: new_stream },
                    Err(e) => {
                        tracing::debug!("retry sse stream error: {e}");
                        *retry_times += 1;
                        if let Some(interval) = this.retry_policy.retry(*retry_times) {
                            let interval = this
                                .server_retry_interval
                                .map(|server_retry_interval| server_retry_interval.max(interval))
                                .unwrap_or(interval);
                            let sleep = tokio::time::sleep(interval);
                            SseAutoReconnectStreamState::WaitingNextRetry {
                                sleep,
                                retry_times: *retry_times,
                            }
                        } else {
                            tracing::error!("sse stream error: {e}, max retry times reached");
                            this.state.set(SseAutoReconnectStreamState::Terminated);
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }
            }
            SseAutoReconnectStreamStateProj::WaitingNextRetry { sleep, retry_times } => {
                ready!(sleep.poll(cx));
                let retrying = this
                    .connector
                    .retry_connection(this.last_event_id.as_deref());
                let retry_times = *retry_times;
                SseAutoReconnectStreamState::Retrying {
                    retry_times,
                    retrying,
                }
            }
            SseAutoReconnectStreamStateProj::Terminated => {
                return Poll::Ready(None);
            }
        };
        // update the state
        this.state.set(next_state);
        self.poll_next(cx)
    }
}
