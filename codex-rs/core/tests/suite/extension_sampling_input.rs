use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::SamplingInputContext;
use codex_extension_api::SamplingInputContributor;
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

struct ReminderFragment(usize);

impl ContextualUserFragment for ReminderFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        format!("{MARKER_PREFIX}{}]", self.0)
    }
}

impl SamplingInputContributor for TimestampLikeContributor {
    fn contribute<'a>(
        &'a self,
        _input: SamplingInputContext<'a>,
    ) -> ExtensionFuture<'a, Result<Vec<Box<dyn ContextualUserFragment + Send>>, String>> {
        Box::pin(async move {
            let attempt = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            let reminder: Box<dyn ContextualUserFragment + Send> =
                Box::new(ReminderFragment(attempt));
            Ok(vec![reminder])
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampling_input_contributor_appends_items_to_history_before_each_request() -> Result<()> {
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
    let reminder_messages = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| text.starts_with(MARKER_PREFIX))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reminder_messages,
        vec![
            vec![format!("{MARKER_PREFIX}1]")],
            vec![format!("{MARKER_PREFIX}1]"), format!("{MARKER_PREFIX}2]"),],
        ]
    );
    assert_eq!(contributor.calls.load(Ordering::Relaxed), 2);

    Ok(())
}
