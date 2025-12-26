use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ConfiguredToolSpec;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::build_specs;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ShellToolCallParams;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_name: String,
    pub call_id: String,
    pub payload: ToolPayload,
}

pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
}

impl ToolRouter {
    pub fn from_config(
        config: &ToolsConfig,
        mcp_tools: Option<HashMap<String, mcp_types::Tool>>,
    ) -> Self {
        let builder = build_specs(config, mcp_tools);
        let (specs, registry) = builder.build();

        Self { registry, specs }
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.specs
            .iter()
            .map(|config| config.spec.clone())
            .collect()
    }

    pub fn tool_supports_parallel(&self, tool_name: &str) -> bool {
        self.specs
            .iter()
            .filter(|config| config.supports_parallel_tool_calls)
            .any(|config| config.spec.name() == tool_name)
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn build_tool_call(
        session: &Session,
        item: ResponseItem,
    ) -> Result<Option<ToolCall>, FunctionCallError> {
        match item {
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                if let Some((server, tool)) = session.parse_mcp_tool_name(&name).await {
                    Ok(Some(ToolCall {
                        tool_name: name,
                        call_id,
                        payload: ToolPayload::Mcp {
                            server,
                            tool,
                            raw_arguments: arguments,
                        },
                    }))
                } else {
                    Ok(Some(ToolCall {
                        tool_name: name,
                        call_id,
                        payload: ToolPayload::Function { arguments },
                    }))
                }
            }
            ResponseItem::CustomToolCall {
                name,
                input,
                call_id,
                ..
            } => Ok(Some(ToolCall {
                tool_name: name,
                call_id,
                payload: ToolPayload::Custom { input },
            })),
            ResponseItem::LocalShellCall {
                id,
                call_id,
                action,
                ..
            } => {
                let call_id = call_id
                    .or(id)
                    .ok_or(FunctionCallError::MissingLocalShellCallId)?;

                match action {
                    LocalShellAction::Exec(exec) => {
                        let params = ShellToolCallParams {
                            command: exec.command,
                            workdir: exec.working_directory,
                            timeout_ms: exec.timeout_ms,
                            sandbox_permissions: Some(SandboxPermissions::UseDefault),
                            justification: None,
                        };
                        Ok(Some(ToolCall {
                            tool_name: "local_shell".to_string(),
                            call_id,
                            payload: ToolPayload::LocalShell { params },
                        }))
                    }
                }
            }
            _ => Ok(None),
        }
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn dispatch_tool_call(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        cancellation_token: tokio_util::sync::CancellationToken,
        call: ToolCall,
    ) -> Result<ResponseInputItem, FunctionCallError> {
        let ToolCall {
            tool_name,
            call_id,
            payload,
        } = call;
        let payload_outputs_custom = matches!(payload, ToolPayload::Custom { .. });
        let failure_call_id = call_id.clone();

        let invocation = ToolInvocation {
            session,
            turn,
            tracker,
            cancellation_token,
            call_id,
            tool_name,
            payload,
        };

        invocation.session.services.hook_runner.on_tool_call_begin(
            invocation.session.as_ref(),
            invocation.turn.as_ref(),
            invocation.tool_name.as_str(),
            invocation.call_id.as_str(),
        );

        match self.registry.dispatch(invocation.clone()).await {
            Ok(response) => {
                invocation.session.services.hook_runner.on_tool_call_end(
                    invocation.session.as_ref(),
                    invocation.turn.as_ref(),
                    invocation.tool_name.as_str(),
                    invocation.call_id.as_str(),
                    &response,
                );
                Ok(response)
            }
            Err(FunctionCallError::Fatal(message)) => {
                invocation.session.services.hook_runner.on_tool_call_fatal(
                    invocation.session.as_ref(),
                    invocation.turn.as_ref(),
                    invocation.tool_name.as_str(),
                    invocation.call_id.as_str(),
                    message.as_str(),
                );
                Err(FunctionCallError::Fatal(message))
            }
            Err(err) => {
                let response = Self::failure_response(failure_call_id, payload_outputs_custom, err);
                invocation.session.services.hook_runner.on_tool_call_end(
                    invocation.session.as_ref(),
                    invocation.turn.as_ref(),
                    invocation.tool_name.as_str(),
                    invocation.call_id.as_str(),
                    &response,
                );
                Ok(response)
            }
        }
    }

    fn failure_response(
        call_id: String,
        payload_outputs_custom: bool,
        err: FunctionCallError,
    ) -> ResponseInputItem {
        let message = err.to_string();
        if payload_outputs_custom {
            ResponseInputItem::CustomToolCallOutput {
                call_id,
                output: message,
            }
        } else {
            ResponseInputItem::FunctionCallOutput {
                call_id,
                output: codex_protocol::models::FunctionCallOutputPayload {
                    content: message,
                    success: Some(false),
                    ..Default::default()
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::ToolOutput;
    use crate::tools::registry::ToolHandler;
    use crate::tools::registry::ToolKind;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::Instant;
    use tokio_util::sync::CancellationToken;

    struct DummyTool;

    #[async_trait]
    impl ToolHandler for DummyTool {
        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        async fn handle(
            &self,
            invocation: ToolInvocation,
        ) -> Result<ToolOutput, FunctionCallError> {
            assert_eq!(invocation.tool_name, "dummy_tool");
            Ok(ToolOutput::Function {
                content: "ok".to_string(),
                content_items: None,
                success: Some(true),
            })
        }
    }

    #[tokio::test]
    async fn tool_call_hooks_fire_on_dispatch() {
        let (session, turn_context) = crate::codex::make_session_and_context().await;
        let mut session = Arc::new(session);
        let turn_context = Arc::new(turn_context);

        let tmp = TempDir::new().expect("create temp dir");
        let out_path = tmp.path().join("hook_tool_call.jsonl");
        let out_path_str = out_path.to_string_lossy().to_string();

        Arc::get_mut(&mut session)
            .expect("unique arc")
            .services
            .hook_runner = crate::hooks::HookRunner::try_new(vec![crate::config::HookConfig {
            id: Some("test-tool-call".to_string()),
            when: vec!["tool.call.end".to_string()],
            matcher: Some("dummy_tool".to_string()),
            command: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!("cat >> \"{out_path_str}\""),
            ],
            timeout_ms: Some(2_000),
            include_output: false,
            include_patch_contents: false,
            include_mcp_arguments: false,
        }])
        .expect("build hook runner");

        let registry = ToolRegistry::new(HashMap::from([(
            "dummy_tool".to_string(),
            Arc::new(DummyTool) as Arc<dyn ToolHandler>,
        )]));
        let router = ToolRouter {
            registry,
            specs: Vec::new(),
        };

        let tracker: SharedTurnDiffTracker =
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let call = ToolCall {
            tool_name: "dummy_tool".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        };

        let _ = router
            .dispatch_tool_call(
                Arc::clone(&session),
                Arc::clone(&turn_context),
                tracker,
                CancellationToken::new(),
                call,
            )
            .await
            .expect("dispatch ok");

        let started = Instant::now();
        loop {
            let contents = std::fs::read_to_string(&out_path).unwrap_or_default();
            if contents.contains("\"type\":\"tool.call.end\"")
                && contents.contains("\"tool_name\":\"dummy_tool\"")
            {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "timed out waiting for hook output to be written"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
