//! Verifies provider attribution follows the selected route, including cached reads.

use super::*;
use pretty_assertions::assert_eq;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;

#[derive(Clone, Default)]
struct ReadEvents(Arc<Mutex<Vec<Value>>>);

#[derive(Default)]
struct Fields(serde_json::Map<String, Value>);

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), json!(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().into(), json!(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_str(field, &format!("{value:?}"));
    }
}

impl<S: tracing::Subscriber> Layer<S> for ReadEvents {
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        if fields.0.get("event.name") == Some(&json!("codex.skill_read.source_selected")) {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(Value::Object(fields.0));
        }
    }
}

#[tokio::test]
async fn source_events_cover_cached_cloud_and_explicit_executor_reads() -> TestResult {
    let cloud = cloud_skill("work").with_plugin_id(Some("cloud-plugin".into()));
    let installed = installed_skill("matching", "work")?;
    let cloud_provider = provider(vec![cloud.clone()]);
    let cloud_reads = cloud_provider.read_requests.clone();
    let executor = provider(vec![installed]);
    let executor_reads = executor.read_requests.clone();
    let harness = MergeHarness::new(executor, cloud_provider, default_config()).await?;
    harness
        .turn_store
        .insert(selected_plugins(&[None, Some("env-1:matching")], &[]));
    let tools = harness.tools(&harness.step_store).await;
    harness.render().await?;
    let events = ReadEvents::default();
    let subscriber = tracing_subscriber::registry().with(events.clone().with_filter(
        tracing_subscriber::filter::filter_fn(codex_otel::OtelProvider::trace_export_filter),
    ));
    // Prevent parallel tests with no subscriber from disabling this callsite globally.
    let _interest_cache_guard =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    async {
        for source in ["cloud", "cloud", "executor", "executor"] {
            let selector = if source == "executor" {
                json!({"kind": source, "environment_id": "env-1"})
            } else {
                json!({"kind": source})
            };
            call_tool(
                &tools,
                "read",
                json!({"package": cloud.id.0, "source": selector}),
            )
            .await?;
        }
        assert!(
            call_tool(&tools, "read", json!({"package": "missing"}))
                .await
                .is_err()
        );
        TestResult::Ok(())
    }
    .with_subscriber(subscriber)
    .await?;

    let expected = ["cloud", "cloud", "executor", "executor"].map(|source| {
        json!({
            "message": "Selected skill read source",
            "event.name": "codex.skill_read.source_selected",
            "conversation.id": "thread",
            "turn_id": "turn-1",
            "call_id": "skill-test",
            "tool_namespace": "skills",
            "tool_name": "read",
            "skill_source": source,
            "skill_name": "deploy:work",
            "skill_plugin_id": if source == "cloud" { "cloud-plugin" } else { "matching" },
            "skill_resource_kind": "main_prompt",
            "is_continuation": false,
        })
    });
    assert_eq!(*events.0.lock().unwrap(), expected);
    // Both cloud reads are attributable even though the second needs no provider call.
    assert_eq!(
        (
            read_request_keys(&cloud_reads).len(),
            read_request_keys(&executor_reads).len()
        ),
        (1, 2),
    );
    Ok(())
}
