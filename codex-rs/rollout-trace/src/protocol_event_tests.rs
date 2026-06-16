use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SubAgentActivityEvent;
use codex_protocol::protocol::SubAgentActivityKind;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::ToolRuntimeTraceEvent;
use super::tool_runtime_trace_event;
use crate::ExecutionStatus;

#[test]
fn sub_agent_activity_is_a_terminal_tool_runtime_event() -> anyhow::Result<()> {
    let agent_thread_id = ThreadId::new();
    let event = EventMsg::SubAgentActivity(SubAgentActivityEvent {
        event_id: "call-spawn".to_string(),
        occurred_at_ms: 1234,
        agent_thread_id,
        agent_path: AgentPath::try_from("/root/reviewer").map_err(anyhow::Error::msg)?,
        kind: SubAgentActivityKind::Started,
    });

    let Some(ToolRuntimeTraceEvent::Ended {
        tool_call_id,
        status,
        payload,
    }) = tool_runtime_trace_event(&event)
    else {
        panic!("expected terminal tool runtime event");
    };

    assert_eq!(tool_call_id, "call-spawn");
    assert_eq!(status, ExecutionStatus::Completed);
    assert_eq!(
        serde_json::to_value(payload)?,
        json!({
            "event_id": "call-spawn",
            "occurred_at_ms": 1234,
            "agent_thread_id": agent_thread_id,
            "agent_path": "/root/reviewer",
            "kind": "started"
        })
    );
    Ok(())
}
