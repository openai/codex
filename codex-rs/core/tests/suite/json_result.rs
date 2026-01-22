#![cfg(not(target_os = "windows"))]

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use responses::ev_assistant_message;
use responses::ev_completed;
use responses::sse;
use responses::start_mock_server;

const SCHEMA: &str = r#"
{
    "type": "object",
    "properties": {
        "explanation": { "type": "string" },
        "final_answer": { "type": "string" }
    },
    "required": ["explanation", "final_answer"],
    "additionalProperties": false
}
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_returns_json_result_for_gpt5() -> anyhow::Result<()> {
    codex_returns_json_result("gpt-5.1".to_string()).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_returns_json_result_for_gpt5_codex() -> anyhow::Result<()> {
    codex_returns_json_result("gpt-5.1-codex".to_string()).await
}

async fn codex_returns_json_result(model: String) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message(
            "m2",
            r#"{"explanation": "explanation", "final_answer": "final_answer"}"#,
        ),
        ev_completed("r1"),
    ]);

    let expected_schema: serde_json::Value = serde_json::from_str(SCHEMA)?;
    let match_json_text_param = move |req: &wiremock::Request| {
        let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
        let Some(text) = body.get("text") else {
            return false;
        };
        let Some(format) = text.get("format") else {
            return false;
        };

        format.get("name") == Some(&serde_json::Value::String("codex_output_schema".into()))
            && format.get("type") == Some(&serde_json::Value::String("json_schema".into()))
            && format.get("strict") == Some(&serde_json::Value::Bool(true))
            && format.get("schema") == Some(&expected_schema)
    };
    responses::mount_sse_once_match(&server, match_json_text_param, sse1).await;

    let TestCodex { codex, cwd, .. } = test_codex().build(&server).await?;

    // 1) Normal user input – should hit server once.
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello world".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: Some(serde_json::from_str(SCHEMA)?),
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let message = wait_for_event(&codex, |ev| matches!(ev, EventMsg::AgentMessage(_))).await;
    if let EventMsg::AgentMessage(message) = message {
        let json: serde_json::Value = serde_json::from_str(&message.message)?;
        assert_eq!(
            json.get("explanation"),
            Some(&serde_json::Value::String("explanation".into()))
        );
        assert_eq!(
            json.get("final_answer"),
            Some(&serde_json::Value::String("final_answer".into()))
        );
    } else {
        anyhow::bail!("expected agent message event");
    }

    Ok(())
}
