//! Applies captured Multi-Agent V2 description and namespace overrides to tool specifications.
//! Execution and parameter schemas are delegated unchanged to the underlying runtime.

use crate::session::session::Session;
use crate::tools::context::ToolInvocation;
use crate::tools::registry::CoreToolRuntime;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExecutor;
use codex_tools::ToolExposure;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use futures::future::BoxFuture;
use std::sync::Arc;

const MULTI_AGENT_V2_NAMESPACE_DESCRIPTION: &str = "Tools for spawning and managing sub-agents.";

pub(super) fn multi_agent_v2_handler(
    handler: impl CoreToolRuntime + 'static,
    namespace: Option<&str>,
    description_override: Option<&str>,
) -> Arc<dyn CoreToolRuntime> {
    if namespace.is_none() && description_override.is_none() {
        return Arc::new(handler);
    }
    Arc::new(MultiAgentV2ToolOverrides {
        handler: Arc::new(handler),
        namespace: namespace.map(str::to_owned),
        description_override: description_override.map(str::to_owned),
    })
}

struct MultiAgentV2ToolOverrides {
    handler: Arc<dyn CoreToolRuntime>,
    namespace: Option<String>,
    description_override: Option<String>,
}

impl ToolExecutor<ToolInvocation> for MultiAgentV2ToolOverrides {
    fn tool_name(&self) -> ToolName {
        let tool_name = self.handler.tool_name();
        match &self.namespace {
            Some(namespace) => ToolName::namespaced(namespace.clone(), tool_name.name),
            None => tool_name,
        }
    }

    fn spec(&self) -> ToolSpec {
        let mut spec = self.handler.spec();
        if let ToolSpec::Function(tool) = &mut spec
            && let Some(description) = &self.description_override
        {
            tool.description.clone_from(description);
        }
        match (&self.namespace, spec) {
            (Some(namespace), ToolSpec::Function(tool)) => {
                ToolSpec::Namespace(ResponsesApiNamespace {
                    name: namespace.clone(),
                    description: MULTI_AGENT_V2_NAMESPACE_DESCRIPTION.to_string(),
                    tools: vec![ResponsesApiNamespaceTool::Function(tool)],
                })
            }
            (_, spec) => spec,
        }
    }

    fn exposure(&self) -> ToolExposure {
        self.handler.exposure()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.handler.supports_parallel_tool_calls()
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        self.handler.search_info()
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        self.handler.handle(invocation)
    }
}

impl CoreToolRuntime for MultiAgentV2ToolOverrides {
    fn wait_until_ready<'a>(&'a self, session: &'a Arc<Session>) -> Option<BoxFuture<'a, ()>> {
        self.handler.wait_until_ready(session)
    }

    fn matches_kind(&self, payload: &crate::tools::context::ToolPayload) -> bool {
        self.handler.matches_kind(payload)
    }

    fn create_diff_consumer(
        &self,
    ) -> Option<Box<dyn crate::tools::registry::ToolArgumentDiffConsumer>> {
        self.handler.create_diff_consumer()
    }
}
