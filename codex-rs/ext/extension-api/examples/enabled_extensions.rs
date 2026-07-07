#[path = "enabled_extensions/shared_state_extension.rs"]
mod shared_state_extension;

use std::future::Future;
use std::pin::pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_protocol::openai_models::ModelInfo;
use shared_state_extension::recorded_style_contributions;
use shared_state_extension::recorded_usage_contributions;

fn main() {
    // 1. Install the contributors for the thread-start input type this host exposes.
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    shared_state_extension::install(&mut builder);
    let registry = builder.build();

    // 2. The host decides which stores are shared.
    let session_store = ExtensionData::new("session");
    let first_thread_store = ExtensionData::new("thread-1");
    let second_thread_store = ExtensionData::new("thread-2");
    let model_info = example_model_info();

    // 3. Reusing the same session store shares session state across threads.
    let first_thread_fragments = block_on_ready(contribute_prompt(
        &registry,
        &session_store,
        &first_thread_store,
        &model_info,
    ));
    block_on_ready(contribute_prompt(
        &registry,
        &session_store,
        &first_thread_store,
        &model_info,
    ));
    block_on_ready(contribute_prompt(
        &registry,
        &session_store,
        &second_thread_store,
        &model_info,
    ));

    println!("first prompt fragments: {}", first_thread_fragments.len());
    println!(
        "session style contributions: {}",
        recorded_style_contributions(&session_store)
    );
    println!(
        "session usage contributions: {}",
        recorded_usage_contributions(&session_store)
    );
    println!(
        "first thread style contributions: {}",
        recorded_style_contributions(&first_thread_store)
    );
    println!(
        "first thread usage contributions: {}",
        recorded_usage_contributions(&first_thread_store)
    );
    println!(
        "second thread style contributions: {}",
        recorded_style_contributions(&second_thread_store)
    );
    println!(
        "second thread usage contributions: {}",
        recorded_usage_contributions(&second_thread_store)
    );
}

async fn contribute_prompt(
    registry: &codex_extension_api::ExtensionRegistry<()>,
    session_store: &ExtensionData,
    thread_store: &ExtensionData,
    model_info: &ModelInfo,
) -> Vec<codex_extension_api::PromptFragment> {
    let mut fragments = Vec::new();
    for contributor in registry.context_contributors() {
        fragments.extend(
            contributor
                .contribute_thread_context(session_store, thread_store, model_info)
                .await,
        );
    }
    fragments
}

fn example_model_info() -> ModelInfo {
    serde_json::from_value(serde_json::json!({
        "slug": "example-model",
        "display_name": "Example Model",
        "supported_reasoning_levels": [],
        "shell_type": "default",
        "visibility": "none",
        "supported_in_api": true,
        "priority": 0,
        "service_tiers": [],
        "base_instructions": "",
        "supports_reasoning_summaries": false,
        "support_verbosity": false,
        "truncation_policy": { "mode": "bytes", "limit": 10_000 },
        "supports_parallel_tool_calls": false,
        "experimental_supported_tools": []
    }))
    .expect("example model metadata should deserialize")
}

fn block_on_ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("example context contributors should complete immediately"),
    }
}
