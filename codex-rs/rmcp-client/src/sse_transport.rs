use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use eventsource_stream::Event;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Url;
use reqwest::header::ACCEPT;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

const EVENT_STREAM_MIME_TYPE: &str = "text/event-stream";
const JSON_MIME_TYPE: &str = "application/json";
const ENDPOINT_EVENT: &str = "endpoint";
const PING_EVENT: &str = "ping";
const DONE_PAYLOAD: &str = "[DONE]";

pub(crate) struct SseClientTransport {
    client: reqwest::Client,
    bearer_token: Option<String>,
    message_url: Arc<RwLock<Option<Arc<str>>>>,
    message_url_ready: Arc<Notify>,
    incoming: mpsc::Receiver<RxJsonRpcMessage<RoleClient>>,
    reader_handle: JoinHandle<()>,
    closed: Arc<AtomicBool>,
}

impl SseClientTransport {
    pub(crate) async fn connect(
        client: reqwest::Client,
        sse_url: String,
        message_url: Option<String>,
        bearer_token: Option<String>,
    ) -> Result<Self> {
        let base_url =
            Url::parse(&sse_url).with_context(|| format!("invalid SSE URL '{sse_url}'"))?;

        let initial_message_url = match message_url.as_deref() {
            Some(endpoint) => Some(resolve_message_url(&base_url, endpoint)?),
            None => None,
        };

        let (tx, rx) = mpsc::channel(16);
        let message_url = Arc::new(RwLock::new(initial_message_url.map(Arc::from)));
        let message_url_ready = Arc::new(Notify::new());
        if message_url.read().await.is_some() {
            message_url_ready.notify_waiters();
        }

        let closed = Arc::new(AtomicBool::new(false));
        let reader_handle = spawn_sse_reader(
            client.clone(),
            sse_url.clone(),
            bearer_token.clone(),
            base_url,
            Arc::clone(&message_url),
            Arc::clone(&message_url_ready),
            tx,
            Arc::clone(&closed),
        )?;

        Ok(Self {
            client,
            bearer_token,
            message_url,
            message_url_ready,
            incoming: rx,
            reader_handle,
            closed,
        })
    }
}

impl Transport<RoleClient> for SseClientTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        let client = self.client.clone();
        let bearer_token = self.bearer_token.clone();
        let message_url = Arc::clone(&self.message_url);
        let message_url_ready = Arc::clone(&self.message_url_ready);
        let closed = Arc::clone(&self.closed);
        async move {
            let message_url = wait_for_message_url(message_url, message_url_ready, closed).await?;
            let mut request = client.post(message_url.as_ref());
            if let Some(token) = bearer_token.as_ref() {
                request = request.bearer_auth(token);
            }
            let response = request.json(&item).send().await.map_err(|err| {
                io::Error::new(io::ErrorKind::Other, format!("SSE send error: {err}"))
            })?;
            if !response.status().is_success() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "SSE message POST to {} failed with status {}",
                        message_url,
                        response.status()
                    ),
                ));
            }
            Ok(())
        }
    }

    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleClient>>> {
        self.incoming.recv()
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.closed.store(true, Ordering::SeqCst);
        self.message_url_ready.notify_waiters();
        self.reader_handle.abort();
        Ok(())
    }
}

fn resolve_message_url(base_url: &Url, endpoint: &str) -> Result<String> {
    if endpoint.trim().is_empty() {
        return Err(anyhow::anyhow!("empty SSE endpoint"));
    }

    if let Ok(url) = Url::parse(endpoint) {
        return Ok(url.to_string());
    }

    Ok(base_url
        .join(endpoint)
        .with_context(|| format!("invalid SSE endpoint '{endpoint}'"))?
        .to_string())
}

async fn wait_for_message_url(
    message_url: Arc<RwLock<Option<Arc<str>>>>,
    message_url_ready: Arc<Notify>,
    closed: Arc<AtomicBool>,
) -> Result<Arc<str>, io::Error> {
    loop {
        if closed.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "SSE transport closed",
            ));
        }

        let notified = message_url_ready.notified();
        if let Some(url) = message_url.read().await.clone() {
            return Ok(url);
        }
        notified.await;
    }
}

fn spawn_sse_reader(
    client: reqwest::Client,
    sse_url: String,
    bearer_token: Option<String>,
    base_url: Url,
    message_url: Arc<RwLock<Option<Arc<str>>>>,
    message_url_ready: Arc<Notify>,
    tx: mpsc::Sender<RxJsonRpcMessage<RoleClient>>,
    closed: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    let handle = tokio::spawn(async move {
        let mut request = client.get(&sse_url).header(
            ACCEPT,
            format!("{EVENT_STREAM_MIME_TYPE}, {JSON_MIME_TYPE}"),
        );
        if let Some(token) = bearer_token.as_ref() {
            request = request.bearer_auth(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                warn!("SSE connect failed for {sse_url}: {error}");
                return;
            }
        };

        if let Err(error) = response.error_for_status_ref() {
            warn!("SSE connect failed for {sse_url}: {error}");
            return;
        }

        let mut events = response.bytes_stream().eventsource();
        while let Some(event) = events.next().await {
            if closed.load(Ordering::SeqCst) {
                break;
            }

            match event {
                Ok(event) => {
                    if let Err(error) = handle_event(
                        event,
                        &sse_url,
                        &base_url,
                        &message_url,
                        &message_url_ready,
                        &tx,
                    )
                    .await
                    {
                        warn!("SSE event handling failed for {sse_url}: {error}");
                    }
                }
                Err(error) => {
                    warn!("SSE stream error for {sse_url}: {error}");
                    break;
                }
            }
        }
        closed.store(true, Ordering::SeqCst);
        message_url_ready.notify_waiters();
    });

    Ok(handle)
}

async fn handle_event(
    event: Event,
    sse_url: &str,
    base_url: &Url,
    message_url: &Arc<RwLock<Option<Arc<str>>>>,
    message_url_ready: &Arc<Notify>,
    tx: &mpsc::Sender<RxJsonRpcMessage<RoleClient>>,
) -> Result<()> {
    let event_name = event.event.trim();
    let payload = event.data.trim();

    if event_name == ENDPOINT_EVENT {
        if payload.is_empty() {
            return Ok(());
        }
        match resolve_message_url(base_url, payload) {
            Ok(resolved) => {
                *message_url.write().await = Some(Arc::from(resolved));
                message_url_ready.notify_waiters();
            }
            Err(error) => {
                warn!("invalid SSE endpoint from {sse_url}: {error}");
            }
        }
        return Ok(());
    }

    if event_name == PING_EVENT {
        return Ok(());
    }

    if payload.is_empty() || payload == DONE_PAYLOAD {
        return Ok(());
    }

    match serde_json::from_str::<RxJsonRpcMessage<RoleClient>>(payload) {
        Ok(message) => {
            if tx.send(message).await.is_err() {
                warn!("SSE message channel closed for {sse_url}");
            }
        }
        Err(error) => {
            warn!("Failed to parse SSE message from {sse_url}: {error}");
        }
    }

    Ok(())
}
