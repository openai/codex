use super::*;
use crate::codex::make_session_and_context;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn multi_agent_v2_request_user_input_rejects_subagent_threads() {
    let (session, mut turn) = make_session_and_context().await;
    turn.session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: ThreadId::new(),
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });

    let result = RequestUserInputHandler {
        default_mode_request_user_input: true,
    }
    .handle(ToolInvocation {
        session: Arc::new(session),
        turn: Arc::new(turn),
        tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
        call_id: "call-1".to_string(),
        tool_name: codex_tools::ToolName::plain(REQUEST_USER_INPUT_TOOL_NAME),
        source: crate::tools::context::ToolCallSource::Direct,
        payload: ToolPayload::Function {
            arguments: json!({
                "questions": [{
                    "header": "Hdr",
                    "question": "Pick one",
                    "id": "pick_one",
                    "options": [
                        {
                            "label": "A",
                            "description": "A"
                        },
                        {
                            "label": "B",
                            "description": "B"
                        }
                    ]
                }]
            })
            .to_string(),
        },
    })
    .await;

    let Err(err) = result else {
        panic!("sub-agent request_user_input should fail");
    };
    assert_eq!(
        err,
        FunctionCallError::RespondToModel(
            "request_user_input can only be used by the root thread".to_string(),
        )
    );
}
