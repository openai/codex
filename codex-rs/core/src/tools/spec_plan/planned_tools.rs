use std::sync::Arc;

use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use futures::future::BoxFuture;
use serde_json::Value;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::registry::ToolExposure;
use crate::tools::registry::ToolRegistry;
use crate::tools::registry::ToolTelemetryTags;
use crate::tools::tool_search_entry::ToolSearchInfo;

type PlannedRuntime = Arc<dyn CoreToolRuntime>;

#[derive(Default)]
pub(super) struct PlannedTools {
    runtimes: Vec<PlannedRuntime>,
}

impl PlannedTools {
    pub(super) fn add<T>(&mut self, runtime: T)
    where
        T: CoreToolRuntime + 'static,
    {
        self.runtimes.push(Arc::new(runtime));
    }

    pub(super) fn add_with_exposure<T>(&mut self, runtime: T, exposure: ToolExposure)
    where
        T: CoreToolRuntime + 'static,
    {
        let runtime: PlannedRuntime = Arc::new(runtime);
        if runtime.exposure() == exposure {
            self.runtimes.push(runtime);
        } else {
            self.runtimes.push(Arc::new(OverriddenTool {
                runtime,
                exposure,
                namespace: None,
            }));
        }
    }

    pub(super) fn add_hidden<T>(&mut self, runtime: T)
    where
        T: CoreToolRuntime + 'static,
    {
        self.add_with_exposure(runtime, ToolExposure::Hidden);
    }

    pub(super) fn add_namespaced_with_exposure<T>(
        &mut self,
        runtime: T,
        namespace: &str,
        description: &str,
        exposure: ToolExposure,
    ) where
        T: CoreToolRuntime + 'static,
    {
        self.runtimes.push(Arc::new(OverriddenTool {
            runtime: Arc::new(runtime),
            exposure,
            namespace: Some(NamespaceOverride {
                name: namespace.to_string(),
                description: description.to_string(),
            }),
        }));
    }

    pub(super) fn prepend(&mut self, runtimes: impl IntoIterator<Item = PlannedRuntime>) {
        self.runtimes.splice(0..0, runtimes);
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &PlannedRuntime> {
        self.runtimes.iter()
    }

    pub(super) fn into_registry(self) -> ToolRegistry {
        ToolRegistry::from_tools(self.runtimes)
    }
}

struct NamespaceOverride {
    name: String,
    description: String,
}

/// A runtime whose name, spec, and exposure are projected for one turn.
///
/// The wrapper remains a runtime so the registry never stores identity or
/// presentation data separately from the executable tool.
struct OverriddenTool {
    runtime: PlannedRuntime,
    exposure: ToolExposure,
    namespace: Option<NamespaceOverride>,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for OverriddenTool {
    fn tool_name(&self) -> ToolName {
        match &self.namespace {
            Some(namespace) => {
                ToolName::namespaced(namespace.name.clone(), self.runtime.tool_name().name)
            }
            None => self.runtime.tool_name(),
        }
    }

    fn spec(&self) -> ToolSpec {
        match (&self.namespace, self.runtime.spec()) {
            (Some(namespace), ToolSpec::Function(tool)) => {
                ToolSpec::Namespace(ResponsesApiNamespace {
                    name: namespace.name.clone(),
                    description: namespace.description.clone(),
                    tools: vec![ResponsesApiNamespaceTool::Function(tool)],
                })
            }
            (_, spec) => spec,
        }
    }

    fn exposure(&self) -> ToolExposure {
        self.exposure
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.exposure != ToolExposure::Hidden && self.runtime.supports_parallel_tool_calls()
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        self.runtime.handle(invocation).await
    }
}

impl CoreToolRuntime for OverriddenTool {
    fn search_info(&self) -> Option<ToolSearchInfo> {
        self.runtime.search_info()
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        self.runtime.matches_kind(payload)
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        self.runtime.pre_tool_use_payload(invocation)
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        self.runtime.post_tool_use_payload(invocation, result)
    }

    fn with_updated_hook_input(
        &self,
        invocation: ToolInvocation,
        updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        self.runtime
            .with_updated_hook_input(invocation, updated_input)
    }

    fn telemetry_tags<'a>(
        &'a self,
        invocation: &'a ToolInvocation,
    ) -> BoxFuture<'a, ToolTelemetryTags> {
        self.runtime.telemetry_tags(invocation)
    }

    fn create_diff_consumer(&self) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.runtime.create_diff_consumer()
    }
}
