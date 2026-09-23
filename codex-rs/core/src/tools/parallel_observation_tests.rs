//! Outer tool timing observations work without enabling textual tracing.
use super::*;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolTimingInput;
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
struct Observer(Mutex<Vec<(String, Duration)>>);
impl ToolLifecycleContributor for Observer {
    fn on_tool_timing(&self, input: ToolTimingInput<'_>) {
        self.0
            .lock()
            .unwrap()
            .push((input.call_id.to_string(), input.duration));
    }
}
#[test]
fn cancelled_dispatch_reports_zero_handler_work_even_without_tracing() {
    let observer = Arc::new(Observer::default());
    let call = ToolCall {
        tool_name: codex_tools::ToolName::plain("tool"),
        call_id: "call".into(),
        payload: ToolPayload::Function {
            arguments: "{}".into(),
        },
        encrypted_function_args: None,
    };
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.tool_lifecycle_contributor(observer.clone());
    let guard = ToolCallTimingGuard::capture(
        Instant::now(),
        &"thread",
        "turn",
        &call,
        &ToolCallSource::Direct,
        Arc::new(extensions.build()),
    )
    .unwrap();
    drop(guard);
    assert_eq!(
        *observer.0.lock().unwrap(),
        vec![("call".into(), Duration::ZERO)]
    );
}
