use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::InitialGoalContributor;
use codex_extension_api::InitialGoalError;
use codex_extension_api::InitialGoalInput;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialGoal;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::sync::oneshot;

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedInitialGoal {
    turn_id: String,
    objective: String,
}

#[derive(Default)]
struct RecordingInitialGoalContributor {
    calls: Mutex<Vec<RecordedInitialGoal>>,
}

impl RecordingInitialGoalContributor {
    fn calls(&self) -> Vec<RecordedInitialGoal> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl InitialGoalContributor for RecordingInitialGoalContributor {
    fn replace_for_turn<'a>(
        &'a self,
        input: InitialGoalInput<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<(), InitialGoalError>> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(RecordedInitialGoal {
                    turn_id: input.turn_id.to_string(),
                    objective: input.goal.objective.clone(),
                });
            Ok(())
        })
    }
}

fn user_input(text: &str) -> Op {
    Op::UserInput {
        items: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        environments: None,
        final_output_json_schema: None,
        responsesapi_client_metadata: None,
        additional_context: Default::default(),
        thread_settings: Default::default(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initial_goal_starts_one_turn_and_rejects_a_concurrent_goal() -> Result<()> {
    let (completion_gate_tx, completion_gate_rx) = oneshot::channel();
    let (server, _) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![ev_response_created("response-1")]),
        },
        StreamingSseChunk {
            gate: Some(completion_gate_rx),
            body: responses::sse(vec![
                ev_assistant_message("message-1", "Initial pass complete."),
                ev_completed("response-1"),
            ]),
        },
    ]])
    .await;

    let contributor = Arc::new(RecordingInitialGoalContributor::default());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    extension_builder.initial_goal_contributor(contributor.clone());
    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_extensions(Arc::new(extension_builder.build()));
    let test = builder.build_with_streaming_server(&server).await?;

    let first_turn_id = test
        .codex
        .submit_user_input_with_client_user_message_id(
            user_input("Improve benchmark coverage"),
            /*trace*/ None,
            /*client_user_message_id*/ None,
            Some(InitialGoal {
                objective: "Improve benchmark coverage".to_string(),
            }),
        )
        .await?;
    let running_status = test.codex.agent_status().await;
    let started_turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    server.wait_for_request_count(1).await;

    let second_error = match test
        .codex
        .submit_user_input_with_client_user_message_id(
            user_input("Replace the active goal"),
            /*trace*/ None,
            /*client_user_message_id*/ None,
            Some(InitialGoal {
                objective: "Replace the active goal".to_string(),
            }),
        )
        .await
    {
        Err(CodexErr::InvalidRequest(message)) => message,
        Err(err) => anyhow::bail!("expected invalid request, got {err}"),
        Ok(turn_id) => anyhow::bail!("concurrent goal unexpectedly started turn {turn_id}"),
    };

    completion_gate_tx
        .send(())
        .map_err(|()| anyhow::anyhow!("response completion gate closed"))?;
    let completed_turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    let requests = server.requests().await;
    let request_body: Value = serde_json::from_slice(
        requests
            .first()
            .ok_or_else(|| anyhow::anyhow!("expected initial model request"))?,
    )?;
    let original_user_texts = request_body
        .get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .filter(|text| !text.starts_with("<environment_context>"))
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(
        (
            contributor.calls(),
            running_status,
            started_turn_id,
            completed_turn_id,
            second_error,
            requests.len(),
            original_user_texts,
        ),
        (
            vec![RecordedInitialGoal {
                turn_id: first_turn_id.clone(),
                objective: "Improve benchmark coverage".to_string(),
            }],
            AgentStatus::Running,
            first_turn_id.clone(),
            first_turn_id,
            "cannot start a goal while another turn is active".to_string(),
            1,
            vec!["Improve benchmark coverage".to_string()],
        )
    );

    Ok(())
}
