//! A minimal async client for the Model Context Protocol (MCP).
//!
//! The client is intentionally lightweight – it is only capable of:
//!   1. Spawning a subprocess that launches a conforming MCP server that
//!      communicates over stdio.
//!   2. Sending MCP requests and pairing them with their corresponding
//!      responses.
//!   3. Offering a convenience helper for the common `tools/list` request.
//!
//! The crate hides all JSON‐RPC framing details behind a typed API. Users
//! interact with the [`ModelContextProtocolRequest`] trait from `mcp-types` to
//! issue requests and receive strongly-typed results.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use mcp_types::CallToolRequest;
use mcp_types::CallToolRequestParams;
use mcp_types::InitializeRequest;
use mcp_types::InitializeRequestParams;
use mcp_types::InitializedNotification;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCError;
use mcp_types::JSONRPCErrorError;
use mcp_types::JSONRPCMessage;
use mcp_types::JSONRPCNotification;
use mcp_types::JSONRPCRequest;
use mcp_types::JSONRPCResponse;
use mcp_types::ListToolsRequest;
use mcp_types::ListToolsRequestParams;
use mcp_types::ListToolsResult;
use mcp_types::ModelContextProtocolNotification;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use reqwest::Url;
use reqwest::header::ACCEPT;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Capacity of the bounded channels used for transporting messages between the
/// client API and the IO tasks.
const CHANNEL_CAPACITY: usize = 128;

/// Internal representation of a pending request sender.
type PendingSender = oneshot::Sender<JSONRPCMessage>;

enum TransportHandle {
    Stdio(tokio::process::Child),
    Network,
}

/// A running MCP client instance.
pub struct McpClient {
    transport: TransportHandle,

    /// Channel for sending JSON-RPC messages *to* the background writer task.
    outgoing_tx: mpsc::Sender<JSONRPCMessage>,

    /// Map of `request.id -> oneshot::Sender` used to dispatch responses back
    /// to the originating caller.
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,

    /// Monotonically increasing counter used to generate request IDs.
    id_counter: AtomicI64,
}

impl McpClient {
    /// Spawn the given command and establish an MCP session over its STDIO.
    /// Caller is responsible for sending the `initialize` request. See
    /// [`initialize`](Self::initialize) for details.
    pub async fn new_stdio_client(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<String, String>>,
    ) -> std::io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .env_clear()
            .envs(create_env_for_mcp_server(env))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            // As noted in the `kill_on_drop` documentation, the Tokio runtime makes
            // a "best effort" to reap-after-exit to avoid zombie processes, but it
            // is not a guarantee.
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("failed to capture child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("failed to capture child stdout"))?;

        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
        let pending: Arc<Mutex<HashMap<i64, PendingSender>>> = Arc::new(Mutex::new(HashMap::new()));

        // Spawn writer task. It listens on the `outgoing_rx` channel and
        // writes messages to the child's STDIN.
        let writer_handle = {
            let mut stdin = stdin;
            tokio::spawn(async move {
                while let Some(msg) = outgoing_rx.recv().await {
                    match serde_json::to_string(&msg) {
                        Ok(json) => {
                            debug!("MCP message to server: {json}");
                            if stdin.write_all(json.as_bytes()).await.is_err() {
                                error!("failed to write message to child stdin");
                                break;
                            }
                            if stdin.write_all(b"\n").await.is_err() {
                                error!("failed to write newline to child stdin");
                                break;
                            }
                            // No explicit flush needed on a pipe; write_all is sufficient.
                        }
                        Err(e) => error!("failed to serialize JSONRPCMessage: {e}"),
                    }
                }
            })
        };

        // Spawn reader task. It reads line-delimited JSON from the child's
        // STDOUT and dispatches responses to the pending map.
        let reader_handle = {
            let pending = pending.clone();
            let mut lines = BufReader::new(stdout).lines();

            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!("MCP message from server: {line}");
                    match serde_json::from_str::<JSONRPCMessage>(&line) {
                        Ok(JSONRPCMessage::Response(resp)) => {
                            Self::dispatch_response(resp, &pending).await;
                        }
                        Ok(JSONRPCMessage::Error(err)) => {
                            Self::dispatch_error(err, &pending).await;
                        }
                        Ok(JSONRPCMessage::Notification(JSONRPCNotification { .. })) => {
                            // For now we only log server-initiated notifications.
                            info!("<- notification: {}", line);
                        }
                        Ok(other) => {
                            // Batch responses and requests are currently not
                            // expected from the server – log and ignore.
                            info!("<- unhandled message: {:?}", other);
                        }
                        Err(e) => {
                            error!("failed to deserialize JSONRPCMessage: {e}; line = {}", line)
                        }
                    }
                }
            })
        };

        // We intentionally *detach* the tasks. They will keep running in the
        // background as long as their respective resources (channels/stdin/
        // stdout) are alive. Dropping `McpClient` cancels the tasks due to
        // dropped resources.
        let _ = (writer_handle, reader_handle);

        Ok(Self {
            transport: TransportHandle::Stdio(child),
            outgoing_tx,
            pending,
            id_counter: AtomicI64::new(1),
        })
    }

    /// Establish an MCP session over an SSE transport.
    pub async fn new_sse_client(
        stream_url: &str,
        messages_url: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<Self> {
        let stream_url = Url::parse(stream_url)
            .with_context(|| format!("invalid SSE stream URL: {stream_url}"))?;
        let post_url = match messages_url {
            Some(url) => {
                Url::parse(url).with_context(|| format!("invalid SSE messages URL: {url}"))?
            }
            None => stream_url.clone(),
        };

        let header_map = Arc::new(build_header_map(headers.as_ref())?);
        let client = reqwest::Client::builder()
            .build()
            .context("failed to construct HTTP client")?;

        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
        let pending: Arc<Mutex<HashMap<i64, PendingSender>>> = Arc::new(Mutex::new(HashMap::new()));

        let writer_client = client.clone();
        let writer_headers = Arc::clone(&header_map);
        let writer_post_url = post_url;
        let pending_for_writer = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                match post_json_message(&writer_client, &writer_post_url, &writer_headers, &message)
                    .await
                {
                    Ok(_) => {}
                    Err(err) => handle_send_failure(&message, &pending_for_writer, err).await,
                }
            }
        });

        spawn_sse_reader(client, stream_url, header_map, Arc::clone(&pending));

        Ok(Self {
            transport: TransportHandle::Network,
            outgoing_tx,
            pending,
            id_counter: AtomicI64::new(1),
        })
    }

    /// Establish an MCP session over the MCP HTTP (streamable) transport.
    pub async fn new_http_client(
        stream_url: &str,
        messages_url: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<Self> {
        let stream_url = Url::parse(stream_url)
            .with_context(|| format!("invalid HTTP stream URL: {stream_url}"))?;
        let post_url = match messages_url {
            Some(url) => {
                Url::parse(url).with_context(|| format!("invalid HTTP messages URL: {url}"))?
            }
            None => stream_url.clone(),
        };

        let header_map = Arc::new(build_header_map(headers.as_ref())?);
        let client = reqwest::Client::builder()
            .build()
            .context("failed to construct HTTP client")?;

        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
        let pending: Arc<Mutex<HashMap<i64, PendingSender>>> = Arc::new(Mutex::new(HashMap::new()));

        let writer_client = client.clone();
        let writer_headers = Arc::clone(&header_map);
        let writer_post_url = post_url;
        let pending_for_writer = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                match post_json_message(&writer_client, &writer_post_url, &writer_headers, &message)
                    .await
                {
                    Ok(response) => {
                        if let Err(err) =
                            process_ndjson_stream(response, Arc::clone(&pending_for_writer)).await
                        {
                            handle_send_failure(&message, &pending_for_writer, err).await;
                        }
                    }
                    Err(err) => handle_send_failure(&message, &pending_for_writer, err).await,
                }
            }
        });

        spawn_http_stream_reader(client, stream_url, header_map, Arc::clone(&pending));

        Ok(Self {
            transport: TransportHandle::Network,
            outgoing_tx,
            pending,
            id_counter: AtomicI64::new(1),
        })
    }

    /// Send an arbitrary MCP request and await the typed result.
    ///
    /// If `timeout` is `None` the call waits indefinitely. If `Some(duration)`
    /// is supplied and no response is received within the given period, a
    /// timeout error is returned.
    pub async fn send_request<R>(
        &self,
        params: R::Params,
        timeout: Option<Duration>,
    ) -> Result<R::Result>
    where
        R: ModelContextProtocolRequest,
        R::Params: Serialize,
        R::Result: DeserializeOwned,
    {
        // Create a new unique ID.
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        let request_id = RequestId::Integer(id);

        // Serialize params -> JSON. For many request types `Params` is
        // `Option<T>` and `None` should be encoded as *absence* of the field.
        let params_json = serde_json::to_value(&params)?;
        let params_field = if params_json.is_null() {
            None
        } else {
            Some(params_json)
        };

        let jsonrpc_request = JSONRPCRequest {
            id: request_id.clone(),
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: R::METHOD.to_string(),
            params: params_field,
        };

        let message = JSONRPCMessage::Request(jsonrpc_request);

        // oneshot channel for the response.
        let (tx, rx) = oneshot::channel();

        // Register in pending map *before* sending the message so a race where
        // the response arrives immediately cannot be lost.
        {
            let mut guard = self.pending.lock().await;
            guard.insert(id, tx);
        }

        // Send to writer task.
        if self.outgoing_tx.send(message).await.is_err() {
            return Err(anyhow!(
                "failed to send message to writer task - channel closed"
            ));
        }

        // Await the response, optionally bounded by a timeout.
        let msg = match timeout {
            Some(duration) => {
                match time::timeout(duration, rx).await {
                    Ok(Ok(msg)) => msg,
                    Ok(Err(_)) => {
                        // Channel closed without a reply – remove the pending entry.
                        let mut guard = self.pending.lock().await;
                        guard.remove(&id);
                        return Err(anyhow!(
                            "response channel closed before a reply was received"
                        ));
                    }
                    Err(_) => {
                        // Timed out. Remove the pending entry so we don't leak.
                        let mut guard = self.pending.lock().await;
                        guard.remove(&id);
                        return Err(anyhow!("request timed out"));
                    }
                }
            }
            None => rx
                .await
                .map_err(|_| anyhow!("response channel closed before a reply was received"))?,
        };

        match msg {
            JSONRPCMessage::Response(JSONRPCResponse { result, .. }) => {
                let typed: R::Result = serde_json::from_value(result)?;
                Ok(typed)
            }
            JSONRPCMessage::Error(err) => Err(anyhow!(format!(
                "server returned JSON-RPC error: code = {}, message = {}",
                err.error.code, err.error.message
            ))),
            other => Err(anyhow!(format!(
                "unexpected message variant received in reply path: {:?}",
                other
            ))),
        }
    }

    pub async fn send_notification<N>(&self, params: N::Params) -> Result<()>
    where
        N: ModelContextProtocolNotification,
        N::Params: Serialize,
    {
        // Serialize params -> JSON. For many request types `Params` is
        // `Option<T>` and `None` should be encoded as *absence* of the field.
        let params_json = serde_json::to_value(&params)?;
        let params_field = if params_json.is_null() {
            None
        } else {
            Some(params_json)
        };

        let method = N::METHOD.to_string();
        let jsonrpc_notification = JSONRPCNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.clone(),
            params: params_field,
        };

        let notification = JSONRPCMessage::Notification(jsonrpc_notification);
        self.outgoing_tx
            .send(notification)
            .await
            .with_context(|| format!("failed to send notification `{method}` to writer task"))
    }

    /// Negotiates the initialization with the MCP server. Sends an `initialize`
    /// request with the specified `initialize_params` and then the
    /// `notifications/initialized` notification once the response has been
    /// received. Returns the response to the `initialize` request.
    pub async fn initialize(
        &self,
        initialize_params: InitializeRequestParams,
        initialize_notification_params: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::InitializeResult> {
        let response = self
            .send_request::<InitializeRequest>(initialize_params, timeout)
            .await?;
        self.send_notification::<InitializedNotification>(initialize_notification_params)
            .await?;
        Ok(response)
    }

    /// Convenience wrapper around `tools/list`.
    pub async fn list_tools(
        &self,
        params: Option<ListToolsRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsResult> {
        self.send_request::<ListToolsRequest>(params, timeout).await
    }

    /// Convenience wrapper around `tools/call`.
    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::CallToolResult> {
        let params = CallToolRequestParams { name, arguments };
        debug!("MCP tool call: {params:?}");
        self.send_request::<CallToolRequest>(params, timeout).await
    }

    /// Internal helper: route a JSON-RPC *response* object to the pending map.
    async fn dispatch_response(
        resp: JSONRPCResponse,
        pending: &Arc<Mutex<HashMap<i64, PendingSender>>>,
    ) {
        let id = match resp.id {
            RequestId::Integer(i) => i,
            RequestId::String(_) => {
                // We only ever generate integer IDs. Receiving a string here
                // means we will not find a matching entry in `pending`.
                error!("response with string ID - no matching pending request");
                return;
            }
        };

        let tx_opt = {
            let mut guard = pending.lock().await;
            guard.remove(&id)
        };
        if let Some(tx) = tx_opt {
            // Ignore send errors – the receiver might have been dropped.
            let _ = tx.send(JSONRPCMessage::Response(resp));
        } else {
            warn!(id, "no pending request found for response");
        }
    }

    /// Internal helper: route a JSON-RPC *error* object to the pending map.
    async fn dispatch_error(
        err: mcp_types::JSONRPCError,
        pending: &Arc<Mutex<HashMap<i64, PendingSender>>>,
    ) {
        let id = match err.id {
            RequestId::Integer(i) => i,
            RequestId::String(_) => return, // see comment above
        };

        let tx_opt = {
            let mut guard = pending.lock().await;
            guard.remove(&id)
        };
        if let Some(tx) = tx_opt {
            let _ = tx.send(JSONRPCMessage::Error(err));
        }
    }
}

fn build_header_map(headers: Option<&HashMap<String, String>>) -> anyhow::Result<HeaderMap> {
    let mut header_map = HeaderMap::new();
    if let Some(headers) = headers {
        for (key, value) in headers {
            let name = HeaderName::from_bytes(key.as_bytes())
                .with_context(|| format!("invalid header name: {key}"))?;
            let value = HeaderValue::from_str(value)
                .with_context(|| format!("invalid header value for {key}"))?;
            header_map.insert(name, value);
        }
    }
    Ok(header_map)
}

async fn post_json_message(
    client: &reqwest::Client,
    url: &Url,
    headers: &HeaderMap,
    message: &JSONRPCMessage,
) -> anyhow::Result<reqwest::Response> {
    let mut request = client.post(url.clone());
    if !headers.is_empty() {
        request = request.headers(headers.clone());
    }
    let body = serde_json::to_vec(message)?;
    let response = request
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await?;
    Ok(response.error_for_status()?)
}

fn spawn_sse_reader(
    client: reqwest::Client,
    stream_url: Url,
    headers: Arc<HeaderMap>,
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,
) {
    tokio::spawn(async move {
        loop {
            let mut request = client.get(stream_url.clone());
            if !headers.is_empty() {
                request = request.headers((*headers).clone());
            }
            request = request.header(ACCEPT, "text/event-stream");

            match request.send().await {
                Ok(response) => {
                    if let Err(err) = process_sse_stream(response, Arc::clone(&pending)).await {
                        warn!("SSE stream error: {err:#}");
                    }
                }
                Err(err) => warn!("failed to establish SSE stream: {err:#}"),
            }

            time::sleep(Duration::from_secs(1)).await;
        }
    });
}

fn spawn_http_stream_reader(
    client: reqwest::Client,
    stream_url: Url,
    headers: Arc<HeaderMap>,
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,
) {
    tokio::spawn(async move {
        loop {
            let mut request = client.get(stream_url.clone());
            if !headers.is_empty() {
                request = request.headers((*headers).clone());
            }
            request = request.header(ACCEPT, "application/x-ndjson");

            match request.send().await {
                Ok(response) => {
                    if let Err(err) = process_ndjson_stream(response, Arc::clone(&pending)).await {
                        warn!("HTTP stream error: {err:#}");
                    }
                }
                Err(err) => warn!("failed to establish HTTP stream: {err:#}"),
            }

            time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn process_sse_stream(
    response: reqwest::Response,
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,
) -> anyhow::Result<()> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("SSE stream returned {status}: {body}");
    }

    let mut events = response.bytes_stream().eventsource();
    while let Some(event) = events.next().await {
        match event {
            Ok(event) => {
                let data = event.data.trim();
                if data.is_empty() {
                    continue;
                }
                match serde_json::from_str::<JSONRPCMessage>(data) {
                    Ok(message) => handle_incoming_message(message, &pending).await,
                    Err(err) => {
                        warn!("failed to decode SSE payload as JSON-RPC: {err}; payload={data}")
                    }
                }
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

async fn process_ndjson_stream(
    response: reqwest::Response,
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,
) -> anyhow::Result<()> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("HTTP stream returned {status}: {body}");
    }

    let mut buffer = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.extend_from_slice(&chunk);

        while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
            let mut line = buffer.drain(..=pos).collect::<Vec<u8>>();
            if let Some(last) = line.last()
                && *last == b'\n'
            {
                line.pop();
            }
            if let Some(last) = line.last()
                && *last == b'\r'
            {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            let text = String::from_utf8(line)
                .map_err(|err| anyhow!("invalid UTF-8 in NDJSON stream: {err}"))?;
            match serde_json::from_str::<JSONRPCMessage>(&text) {
                Ok(message) => handle_incoming_message(message, &pending).await,
                Err(err) => {
                    warn!("failed to decode NDJSON payload as JSON-RPC: {err}; payload={text}")
                }
            }
        }
    }

    if !buffer.is_empty() {
        let text = String::from_utf8(buffer)
            .map_err(|err| anyhow!("invalid UTF-8 in NDJSON tail: {err}"))?;
        if !text.trim().is_empty() {
            match serde_json::from_str::<JSONRPCMessage>(&text) {
                Ok(message) => handle_incoming_message(message, &pending).await,
                Err(err) => {
                    warn!("failed to decode NDJSON tail as JSON-RPC: {err}; payload={text}")
                }
            }
        }
    }

    Ok(())
}

async fn handle_incoming_message(
    message: JSONRPCMessage,
    pending: &Arc<Mutex<HashMap<i64, PendingSender>>>,
) {
    match message {
        JSONRPCMessage::Response(resp) => McpClient::dispatch_response(resp, pending).await,
        JSONRPCMessage::Error(err) => McpClient::dispatch_error(err, pending).await,
        JSONRPCMessage::Notification(notification) => {
            info!("<- notification: {}", notification.method);
        }
        JSONRPCMessage::Request(request) => {
            info!("<- server-initiated request ignored: {}", request.method);
        }
    }
}

async fn handle_send_failure(
    message: &JSONRPCMessage,
    pending: &Arc<Mutex<HashMap<i64, PendingSender>>>,
    error: anyhow::Error,
) {
    warn!("failed to send MCP message: {error:#}");

    let request_id = match message {
        JSONRPCMessage::Request(req) => match &req.id {
            RequestId::Integer(id) => Some(*id),
            RequestId::String(_) => None,
        },
        _ => None,
    };

    if let Some(id) = request_id {
        let mut guard = pending.lock().await;
        if let Some(tx) = guard.remove(&id) {
            let err = JSONRPCError {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id: RequestId::Integer(id),
                error: JSONRPCErrorError {
                    code: -32000,
                    message: format!("failed to send request: {error:#}"),
                    data: None,
                },
            };
            let _ = tx.send(JSONRPCMessage::Error(err));
        }
    }
}
impl Drop for McpClient {
    fn drop(&mut self) {
        if let TransportHandle::Stdio(child) = &mut self.transport {
            // Even though we have already tagged this process with
            // `kill_on_drop(true)` above, this extra check has the benefit of
            // forcing the process to be reaped immediately if it has already exited
            // instead of waiting for the Tokio runtime to reap it later.
            let _ = child.try_wait();
        }
    }
}

/// Environment variables that are always included when spawning a new MCP
/// server.
#[rustfmt::skip]
#[cfg(unix)]
const DEFAULT_ENV_VARS: &[&str] = &[
    // https://modelcontextprotocol.io/docs/tools/debugging#environment-variables
    // states:
    //
    // > MCP servers inherit only a subset of environment variables automatically,
    // > like `USER`, `HOME`, and `PATH`.
    //
    // But it does not fully enumerate the list. Empirically, when spawning a
    // an MCP server via Claude Desktop on macOS, it reports the following
    // environment variables:
    "HOME",
    "LOGNAME",
    "PATH",
    "SHELL",
    "USER",
    "__CF_USER_TEXT_ENCODING",

    // Additional environment variables Codex chooses to include by default:
    "LANG",
    "LC_ALL",
    "TERM",
    "TMPDIR",
    "TZ",
];

#[cfg(windows)]
const DEFAULT_ENV_VARS: &[&str] = &[
    // TODO: More research is necessary to curate this list.
    "PATH",
    "PATHEXT",
    "USERNAME",
    "USERDOMAIN",
    "USERPROFILE",
    "TEMP",
    "TMP",
];

/// `extra_env` comes from the config for an entry in `mcp_servers` in
/// `config.toml`.
fn create_env_for_mcp_server(
    extra_env: Option<HashMap<String, String>>,
) -> HashMap<String, String> {
    DEFAULT_ENV_VARS
        .iter()
        .filter_map(|var| match std::env::var(var) {
            Ok(value) => Some((var.to_string(), value)),
            Err(_) => None,
        })
        .chain(extra_env.unwrap_or_default())
        .collect::<HashMap<_, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_create_env_for_mcp_server() {
        let env_var = "USER";
        let env_var_existing_value = std::env::var(env_var).unwrap_or_default();
        let env_var_new_value = format!("{env_var_existing_value}-extra");
        let extra_env = HashMap::from([(env_var.to_owned(), env_var_new_value.clone())]);
        let mcp_server_env = create_env_for_mcp_server(Some(extra_env));
        assert!(mcp_server_env.contains_key("PATH"));
        assert_eq!(Some(&env_var_new_value), mcp_server_env.get(env_var));
    }
}
