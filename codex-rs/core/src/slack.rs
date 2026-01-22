use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use futures::StreamExt;
use http::header::AUTHORIZATION;
use http::header::COOKIE;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::header::ORIGIN;
use http::header::REFERER;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::warn;

use crate::codex::Session;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::protocol::Submission;

const DEFAULT_API_BASE: &str = "https://slack.com/api/";
const USER_AGENT: &str = "wee_slack_mcp/0.1";
const SLACK_THREADS_DIR: &str = "slack_threads";
const SLACK_NOTIFY_DEFAULT_PREFIX: &str = "[Codex]";
const RTM_RECONNECT_DELAY: Duration = Duration::from_secs(2);
const DEDUPE_CAPACITY: usize = 5_000;

static USER_MENTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<@([A-Za-z0-9]+)(?:\\|([^>]+))?>").unwrap());
static USER_MENTION_FIX_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"@\\|([^\\s>]+)").unwrap());

#[derive(Clone, Debug)]
struct SlackConfig {
    token: Option<String>,
    cookie: Option<String>,
    api_base: String,
    base_url: Option<String>,
}

impl SlackConfig {
    fn from_env() -> Result<Option<Self>> {
        let token_env = std::env::var("SLACK_TOKEN").ok();
        let cookie_env = std::env::var("SLACK_COOKIE").ok();
        let workspace_env = std::env::var("SLACK_WORKSPACE").ok();

        let mut token = token_env.clone();
        let mut cookie = cookie_env.clone();

        if let Some(token_val) = token_env.as_deref()
            && token_val.contains(':')
        {
            let mut parts = token_val.splitn(2, ':');
            let token_part = parts.next().unwrap_or("").trim();
            let cookie_part = parts.next().unwrap_or("").trim();
            if token_part.is_empty() {
                anyhow::bail!("Invalid SLACK_TOKEN format (empty token part)");
            }
            token = Some(token_part.to_string());
            if !cookie_part.is_empty() {
                cookie = Some(cookie_part.to_string());
            }
        }

        if token.is_none() && cookie.is_none() {
            return Ok(None);
        }

        if token.is_none() {
            if let Some(workspace) = workspace_env.as_deref() {
                let api_base = format!("https://{workspace}.slack.com/api/");
                let base_url = Some(format!("https://{workspace}.slack.com"));
                return Ok(Some(Self {
                    token,
                    cookie,
                    api_base,
                    base_url,
                }));
            }
            anyhow::bail!("SLACK_COOKIE requires SLACK_WORKSPACE (e.g. your-workspace)");
        }

        Ok(Some(Self {
            token,
            cookie,
            api_base: DEFAULT_API_BASE.to_string(),
            base_url: None,
        }))
    }
}

#[derive(Clone)]
struct SlackClient {
    config: SlackConfig,
    client: reqwest::Client,
}

impl SlackClient {
    fn new(config: SlackConfig) -> Result<Self> {
        let client = build_reqwest_client();
        Ok(Self { config, client })
    }

    async fn api_call(&self, method: &str, params: HashMap<&str, String>) -> Result<Value> {
        let url = format!("{}{}", self.config.api_base, method);
        let mut data = params;
        if self.config.api_base != DEFAULT_API_BASE {
            if let Some(token) = &self.config.token {
                data.entry("token").or_insert_with(|| token.clone());
            }
        }
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static(USER_AGENT),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip"),
        );
        if let Some(token) = &self.config.token {
            let value = format!("Bearer {token}");
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&value)?);
        }
        if let Some(base_url) = &self.config.base_url {
            headers.insert(ORIGIN, HeaderValue::from_str(base_url)?);
            headers.insert(REFERER, HeaderValue::from_str(&format!("{base_url}/"))?);
            headers.insert(
                HeaderName::from_static("x-slack-user-agent"),
                HeaderValue::from_static("SlackWeb/1.0"),
            );
            if let Ok(version_ts) = std::env::var("SLACK_VERSION_TS") {
                headers.insert(
                    HeaderName::from_static("x-slack-version-ts"),
                    HeaderValue::from_str(&version_ts)?,
                );
            }
        }
        if let Some(cookie) = &self.config.cookie {
            let mut cookie_val = cookie.clone();
            if !cookie_val.starts_with("d=") {
                cookie_val = format!("d={cookie_val}");
            }
            headers.insert(COOKIE, HeaderValue::from_str(&cookie_val)?);
        }

        let response = self
            .client
            .post(url)
            .headers(headers)
            .form(&data)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .context("Slack API request failed")?;
        let payload: Value = response
            .json()
            .await
            .context("Slack API returned non-JSON response")?;
        if payload.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            Ok(payload)
        } else {
            let error = payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown_error");
            anyhow::bail!("Slack API error for {method}: {error}");
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SlackThreadRecord {
    thread_id: String,
    channel_name: String,
    channel_id: String,
    thread_ts: String,
}

struct SlackThreadStore {
    root: PathBuf,
}

impl SlackThreadStore {
    fn new(codex_home: &Path) -> Self {
        Self {
            root: codex_home.join(SLACK_THREADS_DIR),
        }
    }

    fn thread_path(&self, thread_id: &ThreadId) -> PathBuf {
        self.root.join(format!("{}.json", thread_id))
    }

    async fn load(&self, thread_id: &ThreadId) -> Result<Option<SlackThreadRecord>> {
        let path = self.thread_path(thread_id);
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let record = serde_json::from_slice::<SlackThreadRecord>(&bytes)
            .context("failed to parse Slack thread record")?;
        Ok(Some(record))
    }

    async fn save(&self, record: &SlackThreadRecord) -> Result<()> {
        tokio::fs::create_dir_all(&self.root).await?;
        let path = self.root.join(format!("{}.json", record.thread_id));
        let bytes = serde_json::to_vec_pretty(record)?;
        tokio::fs::write(path, bytes).await?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SlackNotifyResult {
    pub channel: String,
    pub ts: String,
    pub thread_ts: String,
}

#[derive(Default)]
struct DedupeState {
    seen: HashSet<String>,
    order: VecDeque<String>,
}

impl DedupeState {
    fn insert(&mut self, message_id: String) -> bool {
        if self.seen.contains(&message_id) {
            return false;
        }
        self.seen.insert(message_id.clone());
        self.order.push_back(message_id);
        while self.order.len() > DEDUPE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.seen.remove(&oldest);
            }
        }
        true
    }
}

#[derive(Default)]
struct SlackThreadState {
    record: Option<SlackThreadRecord>,
}

pub struct SlackThreadManager {
    notify_channel: String,
    notify_prefix: String,
    thread_id: ThreadId,
    cwd: PathBuf,
    store: SlackThreadStore,
    client: SlackClient,
    state: Mutex<SlackThreadState>,
    dedupe: Mutex<DedupeState>,
    self_user_id: Mutex<Option<String>>,
    cancel_token: CancellationToken,
    started: AtomicBool,
}

impl SlackThreadManager {
    pub async fn from_env(
        config: &Config,
        thread_id: ThreadId,
        cwd: PathBuf,
    ) -> Result<Option<Arc<Self>>> {
        let Some(channel) = std::env::var("SLACKMCP_NOTIFY_CHANNEL").ok() else {
            return Ok(None);
        };
        let notify_prefix = SLACK_NOTIFY_DEFAULT_PREFIX.to_string();
        let Some(slack_config) = SlackConfig::from_env()? else {
            warn!("Slack integration disabled: missing SLACK_TOKEN or SLACK_COOKIE");
            return Ok(None);
        };
        let client = SlackClient::new(slack_config)?;
        let store = SlackThreadStore::new(&config.codex_home);
        let record = store.load(&thread_id).await?;
        let state = SlackThreadState { record };
        Ok(Some(Arc::new(Self {
            notify_channel: channel,
            notify_prefix,
            thread_id,
            cwd,
            store,
            client,
            state: Mutex::new(state),
            dedupe: Mutex::new(DedupeState::default()),
            self_user_id: Mutex::new(None),
            cancel_token: CancellationToken::new(),
            started: AtomicBool::new(false),
        })))
    }

    pub fn start(
        self: &Arc<Self>,
        session: Arc<Session>,
        submission_tx: async_channel::Sender<Submission>,
    ) {
        if self
            .started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(err) = manager.ensure_thread().await {
                warn!("Failed to initialize Slack thread: {err:#}");
            }
            manager.run_rtm_listener(session, submission_tx).await;
        });
    }

    pub fn shutdown(&self) {
        self.cancel_token.cancel();
    }

    pub async fn notify_user(&self, message: &str) -> Result<SlackNotifyResult> {
        if message.trim().is_empty() {
            anyhow::bail!("message must not be empty");
        }
        let record = self.ensure_thread().await?;
        let text = format!("{} {}", self.notify_prefix, message.trim());
        let payload = self
            .client
            .api_call(
                "chat.postMessage",
                HashMap::from([
                    ("channel", record.channel_id.clone()),
                    ("text", text),
                    ("thread_ts", record.thread_ts.clone()),
                ]),
            )
            .await?;
        let channel = payload
            .get("channel")
            .and_then(Value::as_str)
            .unwrap_or(&record.channel_id)
            .to_string();
        let ts = payload
            .get("ts")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(SlackNotifyResult {
            channel,
            ts,
            thread_ts: record.thread_ts,
        })
    }

    async fn ensure_thread(&self) -> Result<SlackThreadRecord> {
        {
            let state = self.state.lock().await;
            if let Some(record) = &state.record
                && record.channel_name == self.notify_channel
            {
                return Ok(record.clone());
            }
        }

        let channel_id = self.find_channel_id(&self.notify_channel).await?;
        let thread_title = format!(
            "{} New Codex thread: {} (cwd: {})",
            self.notify_prefix,
            self.thread_id,
            self.cwd.display()
        );
        let payload = self
            .client
            .api_call(
                "chat.postMessage",
                HashMap::from([("channel", channel_id.clone()), ("text", thread_title)]),
            )
            .await?;
        let thread_ts = payload
            .get("ts")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if thread_ts.is_empty() {
            anyhow::bail!("Slack API did not return thread timestamp");
        }
        let record = SlackThreadRecord {
            thread_id: self.thread_id.to_string(),
            channel_name: self.notify_channel.clone(),
            channel_id,
            thread_ts,
        };
        self.store.save(&record).await?;
        let mut state = self.state.lock().await;
        state.record = Some(record.clone());
        Ok(record)
    }

    async fn find_channel_id(&self, channel_name: &str) -> Result<String> {
        let normalized = channel_name.trim().trim_start_matches('#');
        let mut cursor: Option<String> = None;
        loop {
            let mut params = HashMap::from([
                ("types", "public_channel,private_channel".to_string()),
                ("exclude_archived", "1".to_string()),
                ("limit", "1000".to_string()),
            ]);
            if let Some(cursor_val) = cursor.as_deref()
                && !cursor_val.is_empty()
            {
                params.insert("cursor", cursor_val.to_string());
            }
            let payload = self.client.api_call("conversations.list", params).await?;
            let channels = payload
                .get("channels")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for ch in channels {
                let name = ch.get("name").and_then(Value::as_str).unwrap_or("");
                if name == normalized {
                    if let Some(id) = ch.get("id").and_then(Value::as_str) {
                        return Ok(id.to_string());
                    }
                }
            }
            let next = payload
                .get("response_metadata")
                .and_then(|meta| meta.get("next_cursor"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if next.is_empty() {
                break;
            }
            cursor = Some(next);
        }
        anyhow::bail!("Could not find channel #{}", normalized)
    }

    async fn current_user_id(&self) -> Result<Option<String>> {
        let mut guard = self.self_user_id.lock().await;
        if guard.is_some() {
            return Ok(guard.clone());
        }
        if self.client.config.token.is_none() {
            return Ok(None);
        }
        let payload = self.client.api_call("auth.test", HashMap::new()).await?;
        let user_id = payload
            .get("user_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        *guard = user_id.clone();
        Ok(user_id)
    }

    async fn run_rtm_listener(
        self: Arc<Self>,
        session: Arc<Session>,
        submission_tx: async_channel::Sender<Submission>,
    ) {
        if self.client.config.token.is_none() {
            debug!("Slack RTM disabled: no token available");
            return;
        }
        loop {
            if self.cancel_token.is_cancelled() {
                break;
            }
            let rtm_url = match self.rtm_connect().await {
                Ok(url) => url,
                Err(err) => {
                    warn!("Slack RTM connect failed: {err:#}");
                    sleep(RTM_RECONNECT_DELAY).await;
                    continue;
                }
            };
            if let Err(err) = self
                .listen_on_socket(&rtm_url, &session, &submission_tx)
                .await
            {
                warn!("Slack RTM socket ended: {err:#}");
                sleep(RTM_RECONNECT_DELAY).await;
            }
        }
    }

    async fn rtm_connect(&self) -> Result<String> {
        let payload = self
            .client
            .api_call(
                "rtm.connect",
                HashMap::from([("batch_presence_aware", "1".to_string())]),
            )
            .await?;
        payload
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("Slack RTM missing url")
    }

    async fn listen_on_socket(
        &self,
        url: &str,
        session: &Arc<Session>,
        submission_tx: &async_channel::Sender<Submission>,
    ) -> Result<()> {
        let mut request = http::Request::builder().uri(url);
        request = request.header(reqwest::header::USER_AGENT, USER_AGENT);
        if let Some(base_url) = &self.client.config.base_url {
            request = request.header(ORIGIN, base_url);
            request = request.header(REFERER, format!("{base_url}/"));
        }
        if let Some(cookie) = &self.client.config.cookie {
            let cookie_val = if cookie.starts_with("d=") {
                cookie.clone()
            } else {
                format!("d={cookie}")
            };
            request = request.header(COOKIE, cookie_val);
        }
        let request = request.body(())?;
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await?;

        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    return Ok(());
                }
                msg = socket.next() => {
                    let Some(msg) = msg else { return Ok(()); };
                    let msg = msg?;
                    match msg {
                        Message::Text(text) => {
                            if let Ok(event) = serde_json::from_str::<SlackEvent>(&text) {
                                if let Some(message) = self.normalize_message_event(event).await? {
                                    self.handle_incoming_message(message, session, submission_tx).await;
                                }
                            }
                        }
                        Message::Close(_) => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }

    async fn handle_incoming_message(
        &self,
        message: SlackMessageEvent,
        session: &Arc<Session>,
        submission_tx: &async_channel::Sender<Submission>,
    ) {
        let mut dedupe = self.dedupe.lock().await;
        if !dedupe.insert(message.message_id.clone()) {
            return;
        }
        drop(dedupe);

        let text = format!(
            "Slack thread reply from {}: {}",
            message.author, message.text
        );
        let input = vec![UserInput::Text {
            text,
            text_elements: Vec::new(),
        }];
        if session.inject_input(input.clone()).await.is_ok() {
            return;
        }
        let sub = Submission {
            id: format!("slack-{}", message.message_id),
            op: Op::UserInput {
                items: input,
                final_output_json_schema: None,
            },
        };
        if let Err(err) = submission_tx.send(sub).await {
            warn!("Failed to enqueue Slack input: {err}");
        }
    }

    async fn normalize_message_event(
        &self,
        event: SlackEvent,
    ) -> Result<Option<SlackMessageEvent>> {
        if event.kind != "message" {
            return Ok(None);
        }
        let mut event = event;
        if let Some(subtype) = event.subtype.as_deref() {
            if subtype == "message_replied" {
                if let Some(message) = event.message.take() {
                    event.text = message.text;
                    event.user = message.user;
                    event.username = message.username;
                    event.bot_id = message.bot_id;
                    event.ts = message.ts;
                    event.thread_ts = message.thread_ts;
                    event.client_msg_id = message.client_msg_id;
                }
            } else {
                return Ok(None);
            }
        }
        let Some(channel) = event.channel.as_deref() else {
            return Ok(None);
        };
        let record = {
            let state = self.state.lock().await;
            state.record.clone()
        };
        let Some(record) = record else {
            return Ok(None);
        };
        if channel != record.channel_id {
            return Ok(None);
        }
        let thread_ts = event.thread_ts.as_deref().unwrap_or("");
        if thread_ts != record.thread_ts {
            return Ok(None);
        }
        let user_id = event.user.clone();
        if let Some(self_user) = self.current_user_id().await? {
            if user_id.as_deref() == Some(self_user.as_str()) {
                return Ok(None);
            }
        }
        if event.bot_id.is_some() {
            return Ok(None);
        }
        let text = simplify_mentions(event.text.unwrap_or_default());
        let author = event
            .username
            .or(event.user)
            .or(event.bot_id)
            .unwrap_or_else(|| "unknown".to_string());
        let message_id = event
            .client_msg_id
            .or(event.ts.clone())
            .unwrap_or_else(|| format!("{}:{}", record.channel_id, thread_ts));
        Ok(Some(SlackMessageEvent {
            author,
            text,
            message_id,
        }))
    }
}

#[derive(Debug, Deserialize)]
struct SlackEvent {
    #[serde(rename = "type")]
    kind: String,
    subtype: Option<String>,
    channel: Option<String>,
    user: Option<String>,
    username: Option<String>,
    bot_id: Option<String>,
    text: Option<String>,
    ts: Option<String>,
    thread_ts: Option<String>,
    client_msg_id: Option<String>,
    message: Option<SlackMessage>,
}

#[derive(Debug, Deserialize)]
struct SlackMessage {
    user: Option<String>,
    username: Option<String>,
    bot_id: Option<String>,
    text: Option<String>,
    ts: Option<String>,
    thread_ts: Option<String>,
    client_msg_id: Option<String>,
}

#[derive(Debug)]
struct SlackMessageEvent {
    author: String,
    text: String,
    message_id: String,
}

fn simplify_mentions(text: String) -> String {
    let simplified = USER_MENTION_RE.replace_all(&text, |caps: &regex::Captures| {
        let label = caps.get(2).map(|m| m.as_str());
        if let Some(label) = label {
            format!("@{label}")
        } else {
            format!("@{}", &caps[1])
        }
    });
    USER_MENTION_FIX_RE
        .replace_all(&simplified, "@$1")
        .to_string()
}
