use super::*;
use codex_protocol::AgentPath;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_test::internal::MockWriter;

#[test]
fn emits_structured_lifecycle_events_at_trace() {
    let output: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let filter = Targets::new()
        .with_default(LevelFilter::WARN)
        .with_target("codex_core::agent_communication", LevelFilter::TRACE);
    let subscriber = tracing_subscriber::registry().with(filter).with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(MockWriter::new(output)),
    );
    let _guard = tracing::subscriber::set_default(subscriber);

    let sender_thread_id = ThreadId::new();
    let receiver_thread_id = ThreadId::new();
    let communication = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root(),
        Vec::new(),
        "hello".to_string(),
        /*trigger_turn*/ false,
    );
    let context = AgentCommunicationContext::from_tool_call(
        AgentCommunicationKind::Message,
        sender_thread_id,
        "call-1",
    );
    emit_agent_communication_created(&context, &communication, receiver_thread_id);
    emit_agent_communication_enqueued(context.id());

    let result_context = AgentCommunicationContext::without_source_call(
        AgentCommunicationKind::Result,
        receiver_thread_id,
    );
    emit_agent_communication_created(&result_context, &communication, sender_thread_id);

    let events = String::from_utf8(output.lock().expect("buffer lock").clone())
        .expect("JSON logs should be UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid JSON log event"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["level"], "TRACE");
    assert_eq!(events[0]["target"], "codex_core::agent_communication");
    assert_eq!(
        events[0]["fields"],
        json!({
            "message": "agent communication updated",
            "event.name": "codex.agent_communication",
            "communication_id": context.id(),
            "kind": "message",
            "state": "created",
            "sender_thread_id": sender_thread_id.to_string(),
            "receiver_thread_id": receiver_thread_id.to_string(),
            "content": "hello",
            "source_call_id": "call-1",
        })
    );
    assert_eq!(events[1]["level"], "TRACE");
    assert_eq!(events[1]["target"], "codex_core::agent_communication");
    assert_eq!(
        events[1]["fields"],
        json!({
            "message": "agent communication updated",
            "event.name": "codex.agent_communication",
            "communication_id": context.id(),
            "state": "enqueued",
        })
    );
    assert_eq!(
        events[2]["fields"],
        json!({
            "message": "agent communication updated",
            "event.name": "codex.agent_communication",
            "communication_id": result_context.id(),
            "kind": "result",
            "state": "created",
            "sender_thread_id": receiver_thread_id.to_string(),
            "receiver_thread_id": sender_thread_id.to_string(),
            "content": "hello",
        })
    );
}

#[test]
fn content_prefers_plaintext_and_falls_back_to_encrypted_content() {
    let plaintext = InterAgentCommunication {
        encrypted_content: Some("encrypted".to_string()),
        ..InterAgentCommunication::new(
            AgentPath::root(),
            AgentPath::root(),
            Vec::new(),
            "plain".to_string(),
            /*trigger_turn*/ false,
        )
    };
    let encrypted = InterAgentCommunication::new_encrypted(
        AgentPath::root(),
        AgentPath::root(),
        Vec::new(),
        "encrypted".to_string(),
        /*trigger_turn*/ false,
    );

    let contents = [&plaintext, &encrypted].map(communication_content);
    assert_eq!(contents, ["plain", "encrypted"]);
}
