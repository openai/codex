use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::SamplingInputContext;
use codex_extension_api::SamplingInputContributor;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

const USER_PROMPT: &str = "exercise sampling input contributors";
const MARKER_PREFIX: &str = "[sampling-attempt=";

struct TimestampLikeContributor {
    calls: AtomicUsize,
}

impl SamplingInputContributor for TimestampLikeContributor {
    fn contribute<'a>(
        &'a self,
        input: SamplingInputContext<'a>,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let attempt = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            let Some(content) = input.request_input.iter_mut().rev().find_map(|item| {
                let ResponseItem::Message { role, content, .. } = item else {
                    return None;
                };
                (role == "user").then_some(content)
            }) else {
                return Err("sampling request has no user message".to_string());
            };
            let Some(text) = content.iter_mut().find_map(|item| {
                let ContentItem::InputText { text } = item else {
                    return None;
                };
                Some(text)
            }) else {
                return Err("user message has no input text".to_string());
            };
            text.push_str(&format!("\n{MARKER_PREFIX}{attempt}]"));
            Ok(())
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampling_input_contributor_runs_for_each_request_without_rewriting_history() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let plan_args = json!({
        "plan": [{"step": "exercise follow-up sampling", "status": "in_progress"}],
    })
    .to_string();
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call("call-1", "update_plan", &plan_args),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let contributor = Arc::new(TimestampLikeContributor {
        calls: AtomicUsize::new(0),
    });
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.sampling_input_contributor(contributor.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build(&server)
        .await?;

    test.submit_turn(USER_PROMPT).await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let user_prompts = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("user")
                .into_iter()
                .find(|text| text.starts_with(USER_PROMPT))
                .expect("request should contain the submitted user prompt")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        user_prompts,
        vec![
            format!("{USER_PROMPT}\n{MARKER_PREFIX}1]"),
            format!("{USER_PROMPT}\n{MARKER_PREFIX}2]"),
        ]
    );
    assert_eq!(contributor.calls.load(Ordering::Relaxed), 2);

    Ok(())
}
