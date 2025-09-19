use std::collections::VecDeque;
use std::fs::File;
use std::mem;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use crate::load_default_config_for_test;

#[derive(Debug, Clone, PartialEq)]
pub struct HarnessOutputs {
    pub request: Value,
    pub events: Vec<Value>,
}

pub struct HarnessData {
    pub actual: HarnessOutputs,
    pub expected: HarnessOutputs,
}

pub async fn run_fixture(dir: impl AsRef<Path>) -> Result<HarnessData> {
    let dir = dir.as_ref();

    let sse_path = dir.join("response_events.json");
    let sse_sequences = load_sse_sequences(&sse_path)
        .with_context(|| format!("load SSE fixture from {}", sse_path.display()))?;
    let sequence_count = sse_sequences.len();

    let prompts_path = dir.join("user_prompts.json");
    let prompts_file = File::open(&prompts_path)
        .with_context(|| format!("open prompts fixture {}", prompts_path.display()))?;
    let prompt_ops: Vec<Op> = serde_json::from_reader(prompts_file)
        .with_context(|| format!("parse prompts fixture {}", prompts_path.display()))?;

    let expected_request_path = dir.join("expected_request.json");
    let expected_request_file = File::open(&expected_request_path).with_context(|| {
        format!(
            "open expected request fixture {}",
            expected_request_path.display()
        )
    })?;
    let expected_request: Value =
        serde_json::from_reader(expected_request_file).with_context(|| {
            format!(
                "parse expected request fixture {}",
                expected_request_path.display()
            )
        })?;

    let expected_events_path = dir.join("expected_events.json");
    let expected_events_file = File::open(&expected_events_path).with_context(|| {
        format!(
            "open expected events fixture {}",
            expected_events_path.display()
        )
    })?;
    let expected_events: Vec<Value> =
        serde_json::from_reader(expected_events_file).with_context(|| {
            format!(
                "parse expected events fixture {}",
                expected_events_path.display()
            )
        })?;

    let expected = HarnessOutputs {
        request: expected_request,
        events: expected_events,
    };

    let server = MockServer::start().await;

    let responder_state = Arc::new(Mutex::new(VecDeque::from(sse_sequences)));
    let responder = SequentialSseResponder {
        bodies: responder_state.clone(),
    };
    let builder = Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(responder);
    let builder = if sequence_count > 0 {
        builder.expect(sequence_count as u64)
    } else {
        builder
    };
    builder.mount(&server).await;

    let provider = ModelProviderInfo {
        name: "harness-mock".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
    };

    let codex_home = TempDir::new().context("create temp dir for config")?;
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider.clone();
    config.model_provider_id = provider.name.clone();
    config
        .model_providers
        .insert(provider.name.clone(), provider.clone());

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("test"));
    let new_conversation = conversation_manager
        .new_conversation(config.clone())
        .await
        .context("spawn conversation")?;
    let mut events: Vec<Event> = vec![Event {
        id: String::new(),
        msg: EventMsg::SessionConfigured(new_conversation.session_configured),
    }];
    let codex = new_conversation.conversation;

    for op in &prompt_ops {
        codex
            .submit(op.clone())
            .await
            .with_context(|| format!("submit op {op:?}"))?;
    }

    let expected_event_count = expected.events.len();
    anyhow::ensure!(
        expected_event_count >= events.len(),
        "expected events fixture must include at least the session configured event"
    );

    while events.len() < expected_event_count {
        let next = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .context("timeout waiting for event")??;
        events.push(next);
    }

    loop {
        let extra = match timeout(Duration::from_millis(200), codex.next_event()).await {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => anyhow::bail!("error receiving extra event: {err}"),
            Err(_) => break,
        };
        events.push(extra);
    }

    let received = server
        .received_requests()
        .await
        .context("read recorded requests")?;
    anyhow::ensure!(
        received.len() == sequence_count,
        "expected {sequence_count} Responses API requests but recorded {}",
        received.len(),
    );

    let replacements = build_replacements(&config, codex_home.path(), &server);
    let sanitized_events = sanitize_events(events, &replacements);

    let mut request_values: Vec<Value> = Vec::new();
    for req in received {
        let body = req
            .body_json::<Value>()
            .context("parse request body JSON")?;
        let sanitized = sanitize_request(body, &replacements);
        request_values.push(sanitized);
    }

    anyhow::ensure!(
        responder_state.lock().expect("lock bodies").is_empty(),
        "unused SSE responses remain in fixture"
    );

    let request_value = match request_values.len() {
        0 => Value::Array(Vec::new()),
        1 => request_values.into_iter().next().expect("request value"),
        _ => Value::Array(request_values),
    };

    let actual = HarnessOutputs {
        request: request_value,
        events: sanitized_events,
    };

    Ok(HarnessData { actual, expected })
}

struct SequentialSseResponder {
    bodies: Arc<Mutex<VecDeque<String>>>,
}

impl Respond for SequentialSseResponder {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let mut bodies = self.bodies.lock().expect("lock SSE bodies");
        match bodies.pop_front() {
            Some(body) => ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(body, "text/event-stream"),
            None => ResponseTemplate::new(500).set_body_string("unexpected additional SSE request"),
        }
    }
}

fn load_sse_sequences(path: &Path) -> Result<Vec<String>> {
    let file = File::open(path).with_context(|| format!("open SSE file {}", path.display()))?;
    let value: Value = serde_json::from_reader(file)
        .with_context(|| format!("parse SSE fixture {}", path.display()))?;
    match value {
        Value::Array(entries) => {
            if entries.iter().all(Value::is_object) {
                Ok(vec![events_to_sse(entries)?])
            } else if entries.iter().all(Value::is_array) {
                let mut bodies = Vec::new();
                for seq in entries {
                    let seq_events = seq.as_array().cloned().ok_or_else(|| {
                        anyhow::anyhow!(
                            "SSE fixture {} entries must be objects or arrays",
                            path.display()
                        )
                    })?;
                    bodies.push(events_to_sse(seq_events)?);
                }
                Ok(bodies)
            } else {
                anyhow::bail!(
                    "SSE fixture {} must be an array of objects or an array of arrays",
                    path.display()
                );
            }
        }
        _ => anyhow::bail!("SSE fixture {} must be a JSON array", path.display()),
    }
}

fn events_to_sse(events: Vec<Value>) -> Result<String> {
    let mut body = String::new();
    for event in events {
        let Some(obj) = event.as_object() else {
            anyhow::bail!("SSE event must be an object: {event}");
        };
        let kind = obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("SSE event missing type: {event}"))?;
        body.push_str(&format!("event: {kind}\n"));
        if obj.len() > 1 {
            body.push_str("data: ");
            body.push_str(&serde_json::to_string(&event)?);
            body.push('\n');
        }
        body.push('\n');
    }
    Ok(body)
}

fn build_replacements(
    config: &codex_core::config::Config,
    codex_home: &Path,
    server: &MockServer,
) -> Vec<(String, &'static str)> {
    let mut pairs = Vec::new();
    let cwd = config.cwd.to_string_lossy().into_owned();
    if !cwd.is_empty() {
        pairs.push((cwd, "<CWD>"));
    }
    let home = codex_home.to_string_lossy().into_owned();
    if !home.is_empty() {
        pairs.push((home, "<CODEX_HOME>"));
    }
    pairs.push((server.uri(), "<MOCK_SERVER>"));
    pairs
}

fn sanitize_events(events: Vec<Event>, replacements: &[(String, &'static str)]) -> Vec<Value> {
    events
        .into_iter()
        .map(|event| {
            let mut value = serde_json::to_value(event).expect("serialize event");
            sanitize_value(&mut value, replacements);

            if let Some(msg) = value.get_mut("msg") {
                if let Some(msg_obj) = msg.as_object_mut() {
                    let msg_type = msg_obj
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if msg_type == "session_configured" {
                        msg_obj.insert(
                            "session_id".to_string(),
                            Value::String("<session>".to_string()),
                        );
                        if msg_obj.contains_key("history_log_id") {
                            msg_obj.insert("history_log_id".to_string(), json!(0));
                        }
                        if msg_obj.contains_key("rollout_path") {
                            msg_obj.insert(
                                "rollout_path".to_string(),
                                Value::String("<rollout>".to_string()),
                            );
                        }
                    }
                }
            }

            value
        })
        .collect()
}

fn sanitize_request(mut value: Value, replacements: &[(String, &'static str)]) -> Value {
    sanitize_value(&mut value, replacements);
    if let Some(obj) = value.as_object_mut() {
        if obj.contains_key("prompt_cache_key") {
            obj.insert(
                "prompt_cache_key".to_string(),
                Value::String("<prompt_cache_key>".to_string()),
            );
        }
    }
    value
}

fn sanitize_value(value: &mut Value, replacements: &[(String, &'static str)]) {
    match value {
        Value::String(s) => {
            let mut current = mem::take(s);
            for (pattern, replacement) in replacements {
                if !pattern.is_empty() && current.contains(pattern) {
                    current = current.replace(pattern, replacement);
                }
            }
            *s = current;
        }
        Value::Array(items) => {
            for item in items {
                sanitize_value(item, replacements);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                sanitize_value(item, replacements);
            }
        }
        _ => {}
    }
}
