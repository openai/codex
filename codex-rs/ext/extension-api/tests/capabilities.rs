use std::sync::Arc;
use std::sync::Mutex;

use codex_extension_api::AgentSpawnFuture;
use codex_extension_api::AgentSpawner;
use codex_extension_api::NoopResponseItemInjector;
use codex_extension_api::ResponseItemInjector;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn noop_response_item_injector_returns_original_items() {
    let items = vec![ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "keep this input".to_string(),
        }],
        phase: None,
    }];

    let returned_items = NoopResponseItemInjector
        .inject_response_items(items.clone())
        .await
        .expect_err("noop injector should reject same-turn injection");

    assert_eq!(returned_items, items);
}

#[tokio::test]
async fn closure_agent_spawner_forwards_arguments_and_result() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorded_calls = Arc::clone(&calls);
    let spawner = move |thread_id: ThreadId,
                        request: String|
          -> AgentSpawnFuture<'static, usize, &'static str> {
        recorded_calls
            .lock()
            .expect("agent spawn calls lock")
            .push((thread_id, request.clone()));
        Box::pin(async move { Ok(request.len()) })
    };
    let thread_id =
        ThreadId::from_string("11111111-1111-4111-8111-111111111111").expect("valid thread id");

    let spawned = spawner
        .spawn_subagent(thread_id, "delegate this".to_string())
        .await;

    assert_eq!(spawned, Ok(13));
    assert_eq!(
        calls.lock().expect("agent spawn calls lock").as_slice(),
        [(thread_id, "delegate this".to_string())]
    );
}
