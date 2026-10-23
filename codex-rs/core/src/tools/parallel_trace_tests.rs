//! Dispatch keeps caller context even when TRACE spans are disabled.
#![cfg(test)]

use super::*;
use crate::session::step_context::StepContext;
use crate::tools::registry::ToolRegistry;
use crate::tools::router::ToolRouter;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::openai_models::ToolMode;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Mutex;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

#[derive(Clone, Default)]
struct ResultScopes(Arc<Mutex<Vec<Vec<String>>>>);

impl<S: tracing::Subscriber + for<'a> LookupSpan<'a>> Layer<S> for ResultScopes {
    fn on_event(&self, event: &tracing::Event<'_>, context: Context<'_, S>) {
        if event.metadata().name() == "codex.tool_result_ready" {
            let scope = context.event_scope(event).map_or_else(Vec::new, |scope| {
                scope
                    .from_root()
                    .map(|span| span.name().to_string())
                    .collect()
            });
            self.0.lock().unwrap().push(scope);
        }
    }
}

#[test_case::test_case(LevelFilter::INFO; "trace_disabled")]
#[test_case::test_case(LevelFilter::TRACE; "trace_enabled")]
#[tokio::test]
async fn spawned_dispatch_preserves_caller_context(level: LevelFilter) -> anyhow::Result<()> {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let router = Arc::new(ToolRouter::from_parts(
        ToolRegistry::from_tools([]),
        Vec::new(),
        ToolMode::Direct,
        BTreeMap::new(),
        /*tool_namespaces_info*/ None,
        &[],
    ));
    let step = StepContext::for_test(Arc::new(turn)).with_tool_router_for_test(router);
    let runtime = ToolCallRuntime::new(
        Arc::new(session),
        step,
        Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
    );
    let scopes = ResultScopes::default();
    // Match CLI logging without an exporter: INFO remains enabled, TRACE can be off.
    let subscriber = tracing_subscriber::registry()
        .with(level)
        .with(scopes.clone());
    let _interest_cache_guard =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    async {
        // The unknown-tool result still crosses the real spawned dispatch boundary.
        runtime
            .handle_tool_call(
                ToolCall {
                    tool_name: codex_tools::ToolName::plain("unknown_tool"),
                    call_id: "call-context".into(),
                    payload: ToolPayload::Function {
                        arguments: "{}".into(),
                    },
                    encrypted_function_args: None,
                },
                CancellationToken::new(),
            )
            .await
    }
    .instrument(tracing::info_span!("caller_turn"))
    .await?;

    let scopes = scopes.0.lock().unwrap();
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0].first().map(String::as_str), Some("caller_turn"));
    assert_eq!(
        scopes[0]
            .iter()
            .any(|name| name == "dispatch_tool_call_with_code_mode_result"),
        level == LevelFilter::TRACE
    );
    Ok(())
}
