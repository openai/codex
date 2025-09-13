use super::compact::FIRST_REPLY;
use super::compact::SUMMARIZE_TRIGGER;
use super::compact::SUMMARY_TEXT;
use super::compact::ev_assistant_message;
use super::compact::ev_completed;
use super::compact::mount_sse_once;
use super::compact::sse;
use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use serde_json::json;
use tempfile::TempDir;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compact_resume_and_fork_preserve_model_history_view() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Start a mock server and mount five sequential expectations.
    let server = MockServer::start().await;

    // 1) Normal assistant reply so it is recorded in history.
    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    // 2) Summarizer returns a summary message (this is what compaction keeps).
    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);
    // 3) After the post-compact user message, return an assistant reply so the
    //    subsequent history includes: compact msg + user msg + response msg.
    let sse3 = sse(vec![
        ev_assistant_message("m3", "AFTER_COMPACT_REPLY"),
        ev_completed("r3"),
    ]);
    let sse4 = sse(vec![ev_completed("r4")]);
    let sse5 = sse(vec![ev_completed("r5")]);

    // Mount expectations with distinct matchers so they are consumed in order.
    let match_first = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"hello world\"")
            && !body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
    };
    mount_sse_once(&server, match_first, sse1).await;

    let match_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(&format!("\"text\":\"{SUMMARIZE_TRIGGER}\""))
    };
    mount_sse_once(&server, match_compact, sse2).await;

    let match_after_compact = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_COMPACT\"")
            && !body.contains("\"text\":\"AFTER_RESUME\"")
            && !body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(&server, match_after_compact, sse3).await;

    let match_after_resume = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_RESUME\"")
    };
    mount_sse_once(&server, match_after_resume, sse4).await;

    let match_after_fork = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains("\"text\":\"AFTER_FORK\"")
    };
    mount_sse_once(&server, match_after_fork, sse5).await;

    // Build config pointing to the mock server and spawn Codex.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider.clone();

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: base, ..
    } = conversation_manager
        .new_conversation(config.clone())
        .await
        .expect("create conversation");

    // Start -> msg -> response.
    base.submit(Op::UserInput {
        items: vec![InputItem::Text {
            text: "hello world".into(),
        }],
    })
    .await
    .unwrap();
    wait_for_event(&base, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // compact
    base.submit(Op::Compact).await.unwrap();
    wait_for_event(&base, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // -> msg (post-compact)
    base.submit(Op::UserInput {
        items: vec![InputItem::Text {
            text: "AFTER_COMPACT".into(),
        }],
    })
    .await
    .unwrap();
    wait_for_event(&base, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Capture rollout path from base conversation (flushes on GetPath).
    base.submit(Op::GetPath).await.unwrap();
    let base_path =
        match wait_for_event(&base, |ev| matches!(ev, EventMsg::ConversationPath(_))).await {
            EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path.clone(),
            _ => panic!("expected ConversationPath"),
        };

    // resume -> msg
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: resumed,
        ..
    } = conversation_manager
        .resume_conversation_from_rollout(config.clone(), base_path.clone(), auth_manager)
        .await
        .expect("resume conversation");

    resumed
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "AFTER_RESUME".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&resumed, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // response -> fork 1 message back
    resumed.submit(Op::GetPath).await.unwrap();
    let resumed_path =
        match wait_for_event(&resumed, |ev| matches!(ev, EventMsg::ConversationPath(_))).await {
            EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path.clone(),
            _ => panic!("expected ConversationPath after resume"),
        };

    let NewConversation {
        conversation: forked,
        ..
    } = conversation_manager
        .fork_conversation(1, config.clone(), resumed_path.clone())
        .await
        .expect("fork conversation");

    forked
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "AFTER_FORK".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&forked, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Verify five requests were made.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 5, "expected exactly five requests");

    // Extract the model-visible history (input without the trailing user message) for
    // requests 3 (after compact), 4 (after resume), and 5 (after fork).
    let req3_body_after_compact = requests[2].body_json::<serde_json::Value>().unwrap();
    let req4_body_after_resume = requests[3].body_json::<serde_json::Value>().unwrap();
    let req5_body_after_fork = requests[4].body_json::<serde_json::Value>().unwrap();

    let prior_history_after_compact = req3_body_after_compact
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap()
        .clone();
    let prior_history_after_resume = req4_body_after_resume
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap()
        .clone();
    let prior_history_after_fork = req5_body_after_fork
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap()
        .clone();

    // Remove the current-turn user message to compare prior context only.
    let expected_prior_history_after_compact = json!([{"type":"message","role":"assistant","content":[{"type":"output_text","text":"SUMMARY_ONLY_CONTEXT"}]},
                                                             {"type":"message","role":"user","content":[{"type":"input_text","text":"AFTER_COMPACT"}]}]);

    let expected_prior_history_after_resume = json!([{"type":"message","role":"assistant","content":[{"type":"output_text","text":"SUMMARY_ONLY_CONTEXT"}]},
                                                            {"type":"message","role":"user","content":[{"type":"input_text","text":"AFTER_COMPACT"}]},
                                                            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"AFTER_COMPACT_REPLY"}]},
                                                            {"type":"message","role":"user","content":[{"type":"input_text","text":"AFTER_RESUME"}]}]);

    let expected_prior_history_after_fork = json!([{"type":"message","role":"assistant","content":[{"type":"output_text","text":"SUMMARY_ONLY_CONTEXT"}]},
                                                            {"type":"message","role":"user","content":[{"type":"input_text","text":"AFTER_COMPACT"}]},
                                                            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"AFTER_COMPACT_REPLY"}]},
                                                            {"type":"message","role":"user","content":[{"type":"input_text","text":"AFTER_FORK"}]}]);
    assert_eq!(
        serde_json::to_value(&prior_history_after_compact).unwrap(),
        expected_prior_history_after_compact
    );
    assert_eq!(
        serde_json::to_value(&prior_history_after_resume).unwrap(),
        expected_prior_history_after_resume
    );
    assert_eq!(
        serde_json::to_value(&prior_history_after_fork).unwrap(),
        expected_prior_history_after_fork
    );
}
