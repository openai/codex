//! Typed remote implementation of the board contract. HTTP policy is supplied
//! by the caller; reads and writes never fall back to a private local board.
//! Live notification frames are bounded before SSE parsing.

use crate::protocol::AccessToken;
use crate::protocol::BoardNotification;
use crate::protocol::Call;
use crate::protocol::Failure;
use crate::protocol::MAX_BODY;
use crate::protocol::MAX_RESPONSE;
use crate::protocol::Operation;
use crate::protocol::Registration;
use crate::protocol::Watch;
use anyhow::Context;
use chrono::DateTime;
use chrono::Utc;
use codex_agent_message_board_extension::AgentMessageBoard;
use codex_agent_message_board_extension::ChannelQuery;
use codex_agent_message_board_extension::ChannelSummary;
use codex_agent_message_board_extension::CreateChannelRequest;
use codex_agent_message_board_extension::Page;
use codex_agent_message_board_extension::PostContent;
use codex_agent_message_board_extension::PostMetadata;
use codex_agent_message_board_extension::PostPreview;
use codex_agent_message_board_extension::PostQuery;
use codex_agent_message_board_extension::PostRequest;
use codex_agent_message_board_extension::ReadPostRequest;
use codex_agent_message_board_extension::ReadThreadRequest;
use codex_agent_message_board_extension::SubscriptionRequest;
use codex_agent_message_board_extension::SubscriptionState;
use codex_agent_message_board_extension::ThreadPage;
use codex_agent_message_board_extension::ThreadQuery;
use codex_agent_message_board_extension::ThreadSummary;
use codex_http_client::HttpClient;
use codex_http_client::HttpResponse;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use url::Url;

/// A client for one session's board. Use the shared HTTP client's configured
/// proxy and CA policy.
#[derive(Clone)]
pub struct RemoteAgentMessageBoard {
    http: HttpClient,
    board_url: Url,
    board: SessionId,
    token: AccessToken,
    clock: Arc<dyn Fn(ThreadId) -> BoxFuture<'static, Result<DateTime<Utc>>> + Send + Sync>,
}

impl RemoteAgentMessageBoard {
    /// Validate the HTTP(S) endpoint and append the board path to its optional path prefix.
    pub fn new(
        http: HttpClient,
        endpoint: &str,
        board: SessionId,
        token: AccessToken,
    ) -> anyhow::Result<Self> {
        let mut board_url = Url::parse(endpoint).context("invalid message-board endpoint")?;
        anyhow::ensure!(
            matches!(board_url.scheme(), "http" | "https") && board_url.host_str().is_some(),
            "message-board endpoint must be an HTTP(S) URL"
        );
        anyhow::ensure!(
            board_url.query().is_none() && board_url.fragment().is_none(),
            "message-board endpoint must not contain a query or fragment"
        );
        board_url.set_path(&format!(
            "{}/v1/boards/{board}",
            board_url.path().trim_end_matches('/')
        ));
        Ok(Self {
            http,
            board_url,
            board,
            token,
            clock: Arc::new(|_| Box::pin(async { Ok(Utc::now()) })),
        })
    }

    /// Supply the runtime's configured clock for research's simulated time.
    pub fn with_clock(
        mut self,
        clock: impl Fn(ThreadId) -> BoxFuture<'static, Result<DateTime<Utc>>> + Send + Sync + 'static,
    ) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    pub async fn create_board(&self) -> Result<SessionId> {
        let response = self
            .http
            .request(http::Method::PUT, self.url(""))
            .bearer_auth(&self.token.0)
            .timeout(Duration::from_secs(/*secs*/ 30))
            .send()
            .await
            .map_err(transport_error)?;
        decode(response).await
    }

    pub async fn delete_board(&self) -> Result<()> {
        let response = self
            .http
            .delete(self.url(""))
            .bearer_auth(&self.token.0)
            .timeout(Duration::from_secs(/*secs*/ 30))
            .send()
            .await
            .map_err(transport_error)?;
        if response.status().is_success() {
            Ok(())
        } else {
            decode(response).await
        }
    }

    /// Register the session's agents. The returned session token is stable on retry.
    pub async fn register_members(
        &self,
        members: HashMap<ThreadId, AgentPath>,
    ) -> Result<AccessToken> {
        let response = self
            .http
            .post(self.url("/members"))
            .bearer_auth(&self.token.0)
            .json(&Registration { members })
            .timeout(Duration::from_secs(/*secs*/ 30))
            .send()
            .await
            .map_err(transport_error)?;
        decode(response).await
    }

    /// Opens a live receiver and waits until the service has registered it.
    /// Setup, including readiness or an error body, has a 30-second deadline.
    /// Drop it when the turn ends. Reopening does not replay earlier previews.
    pub async fn notifications(
        &self,
        caller: ThreadId,
        turn_id: String,
    ) -> Result<BoardNotifications> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(/*secs*/ 30);
        let response = tokio::time::timeout_at(
            deadline,
            self.http
                .post(self.url("/notifications"))
                .bearer_auth(&self.token.0)
                .json(&Watch {
                    caller,
                    turn_id: turn_id.clone(),
                })
                .send(),
        )
        .await
        .map_err(transport_error)?
        .map_err(transport_error)?;
        if !response.status().is_success() {
            tokio::time::timeout_at(deadline, decode::<()>(response))
                .await
                .map_err(transport_error)??;
            return Err(transport_error("unexpected notification response"));
        }
        // Count wire bytes before the parser buffers them, resetting at blank lines.
        // Defer CR handling so CRLF counts as one line ending even across chunks.
        let mut frame_bytes = 0;
        let mut line_empty = true;
        let mut previous_cr = false;
        // Validate before the SSE decoder can retain malformed UTF-8 indefinitely.
        // Only an incomplete code point (at most three bytes) carries across chunks.
        let mut utf8 = [0; 4];
        let mut utf8_len = 0;
        let mut chunks = Some(response.bytes_stream());
        let mut events = futures::stream::poll_fn(move |cx| {
            let Some(source) = chunks.as_mut() else {
                return Poll::Ready(None);
            };
            let Some(chunk) = std::task::ready!(source.poll_next_unpin(cx)) else {
                chunks = None;
                return Poll::Ready(None);
            };
            let result = chunk.map_err(transport_error).and_then(|chunk| {
                for &byte in chunk.iter() {
                    if !byte.is_ascii() || utf8_len > 0 {
                        utf8[utf8_len] = byte;
                        utf8_len += 1;
                        match std::str::from_utf8(&utf8[..utf8_len]) {
                            Ok(_) => utf8_len = 0,
                            Err(error) if error.error_len().is_some() => {
                                return Err(transport_error(error));
                            }
                            Err(_) => {}
                        }
                    }
                    if previous_cr && byte != b'\n' {
                        if line_empty {
                            frame_bytes = 0;
                        }
                        line_empty = true;
                    }
                    frame_bytes += 1;
                    if frame_bytes > MAX_BODY {
                        return Err(transport_error("board SSE frame exceeds the service limit"));
                    }
                    match byte {
                        b'\n' => {
                            if line_empty {
                                frame_bytes = 0;
                            }
                            line_empty = true;
                        }
                        b'\r' => {}
                        _ => line_empty = false,
                    }
                    previous_cr = byte == b'\r';
                }
                Ok(chunk)
            });
            if result.is_err() {
                chunks = None;
            }
            Poll::Ready(Some(result))
        })
        .eventsource();
        let ready = tokio::time::timeout_at(deadline, events.next())
            .await
            .map_err(transport_error)?
            .ok_or_else(|| transport_error("notification stream closed before readiness"))?
            .map_err(transport_error)?;
        if ready.event != "ready" {
            return Err(transport_error(
                "notification stream did not acknowledge readiness",
            ));
        }
        let stream = events
            .map(|event| {
                let event = event.map_err(transport_error)?;
                if event.event != "notification" {
                    return Err(transport_error("invalid board notification"));
                }
                serde_json::from_str(&event.data).map_err(CodexErr::from)
            })
            .boxed();
        Ok(BoardNotifications {
            caller,
            turn_id,
            stream,
        })
    }

    fn url(&self, suffix: &str) -> Url {
        let mut url = self.board_url.clone();
        url.set_path(&format!("{}{suffix}", url.path()));
        url
    }

    async fn call<T: DeserializeOwned>(&self, caller: ThreadId, operation: Operation) -> Result<T> {
        let timestamp = match &operation {
            Operation::CreateChannel(_) => Some((self.clock)(caller).await?),
            // The service can replay a committed post without a fresh timestamp.
            // A new post still requires one; never substitute wall-clock time.
            Operation::Post(_) => (self.clock)(caller).await.ok(),
            Operation::ListChannels(_)
            | Operation::ListThreads(_)
            | Operation::SearchPosts(_)
            | Operation::ReadThread(_)
            | Operation::ReadPost(_)
            | Operation::SetSubscription(_) => None,
        };
        let call = Call {
            caller,
            timestamp,
            operation,
        };
        let body = serde_json::to_vec(&call)?;
        if body.len() > MAX_BODY {
            return Err(CodexErr::InvalidRequest(
                "board request exceeds the service limit".into(),
            ));
        }
        let response = self
            .http
            .post(self.url("/call"))
            .bearer_auth(&self.token.0)
            .header("content-type", "application/json")
            .body(body)
            .timeout(Duration::from_secs(/*secs*/ 30))
            .send()
            .await
            .map_err(transport_error)?;
        decode(response).await
    }
}

macro_rules! operation {
    ($method:ident, $variant:ident, $input:ty, $output:ty) => {
        fn $method(&self, caller: ThreadId, request: $input) -> BoxFuture<'_, Result<$output>> {
            Box::pin(self.call(caller, Operation::$variant(request)))
        }
    };
}

impl AgentMessageBoard for RemoteAgentMessageBoard {
    fn identity(&self) -> SessionId {
        self.board
    }
    operation!(
        create_channel,
        CreateChannel,
        CreateChannelRequest,
        ChannelSummary
    );
    operation!(
        list_channels,
        ListChannels,
        ChannelQuery,
        Page<ChannelSummary>
    );
    operation!(post, Post, PostRequest, PostMetadata);
    operation!(list_threads, ListThreads, ThreadQuery, Page<ThreadSummary>);
    operation!(search_posts, SearchPosts, PostQuery, Page<PostPreview>);
    operation!(read_thread, ReadThread, ReadThreadRequest, ThreadPage);
    operation!(read_post, ReadPost, ReadPostRequest, PostContent);
    operation!(
        set_subscription,
        SetSubscription,
        SubscriptionRequest,
        SubscriptionState
    );
}

/// One active turn's live receiver. The host must also check the turn atomically
/// when injecting a returned notification, since the turn can end after `next`.
pub struct BoardNotifications {
    caller: ThreadId,
    turn_id: String,
    stream: BoxStream<'static, Result<BoardNotification>>,
}

impl BoardNotifications {
    pub async fn next(&mut self) -> Result<Option<BoardNotification>> {
        let Some(notice) = self.stream.next().await.transpose()? else {
            return Ok(None);
        };
        if notice.recipient != self.caller || notice.turn_id != self.turn_id {
            return Err(transport_error(
                "notification belongs to another agent or turn",
            ));
        }
        if notice.post.text_preview.chars().count() > 150 {
            return Err(transport_error(
                "board notification preview exceeds 150 characters",
            ));
        }
        Ok(Some(notice))
    }
}

async fn decode<T: DeserializeOwned>(mut response: HttpResponse) -> Result<T> {
    let status = response.status();
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE {
            return Err(transport_error("board response exceeds the service limit"));
        }
        body.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let failure: Failure = serde_json::from_slice(&body)
            .with_context(|| format!("invalid board error response (HTTP {status})"))
            .map_err(transport_error)?;
        let message = format!("{}: {}", failure.code, failure.message);
        return Err(if status.is_server_error() {
            transport_error(message)
        } else {
            CodexErr::InvalidRequest(message)
        });
    }
    serde_json::from_slice(&body).map_err(CodexErr::from)
}

fn transport_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> CodexErr {
    CodexErr::Io(std::io::Error::other(error))
}
