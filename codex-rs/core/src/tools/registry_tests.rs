use super::*;
use pretty_assertions::assert_eq;

struct TestHandler {
    tool_name: codex_tools::ToolName,
}

impl ToolExecutor<ToolInvocation> for TestHandler {
    fn tool_name(&self) -> codex_tools::ToolName {
        self.tool_name.clone()
    }

    fn spec(&self) -> codex_tools::ToolSpec {
        test_spec(&self.tool_name)
    }

    fn handle(&self, _invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async {
            Ok(
                Box::new(crate::tools::context::FunctionToolOutput::from_text(
                    "ok".to_string(),
                    Some(true),
                )) as Box<dyn crate::tools::context::ToolOutput>,
            )
        })
    }
}

impl CoreToolRuntime for TestHandler {}

#[derive(Clone)]
enum LifecycleTestResult {
    Ok { success: bool },
    Err,
}

struct LifecycleTestHandler {
    tool_name: codex_tools::ToolName,
    result: LifecycleTestResult,
}

impl ToolExecutor<ToolInvocation> for LifecycleTestHandler {
    fn tool_name(&self) -> codex_tools::ToolName {
        self.tool_name.clone()
    }

    fn spec(&self) -> codex_tools::ToolSpec {
        test_spec(&self.tool_name)
    }

    fn handle(&self, _invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call())
    }
}

impl LifecycleTestHandler {
    async fn handle_call(
        &self,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        match self.result.clone() {
            LifecycleTestResult::Ok { success } => Ok(Box::new(
                crate::tools::context::FunctionToolOutput::from_text(
                    "ok".to_string(),
                    Some(success),
                ),
            )
                as Box<dyn crate::tools::context::ToolOutput>),
            LifecycleTestResult::Err => Err(FunctionCallError::RespondToModel(
                "handler failed".to_string(),
            )),
        }
    }
}

impl CoreToolRuntime for LifecycleTestHandler {}

fn test_spec(tool_name: &codex_tools::ToolName) -> codex_tools::ToolSpec {
    codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
        name: tool_name.name.clone(),
        description: "Test tool.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: codex_tools::JsonSchema::default(),
        output_schema: None,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum RecordedToolLifecycle {
    Start {
        call_id: String,
        tool_name: codex_tools::ToolName,
    },
    Finish {
        call_id: String,
        tool_name: codex_tools::ToolName,
        outcome: codex_extension_api::ToolCallOutcome,
    },
}

struct ToolLifecycleRecorder {
    records: Arc<std::sync::Mutex<Vec<RecordedToolLifecycle>>>,
}

impl codex_extension_api::ToolLifecycleContributor for ToolLifecycleRecorder {
    fn on_tool_start<'a>(
        &'a self,
        input: codex_extension_api::ToolStartInput<'a>,
    ) -> codex_extension_api::ToolLifecycleFuture<'a> {
        let records = Arc::clone(&self.records);
        let record = RecordedToolLifecycle::Start {
            call_id: input.call_id.to_string(),
            tool_name: input.tool_name.clone(),
        };
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(record);
        })
    }

    fn on_tool_finish<'a>(
        &'a self,
        input: codex_extension_api::ToolFinishInput<'a>,
    ) -> codex_extension_api::ToolLifecycleFuture<'a> {
        let records = Arc::clone(&self.records);
        let record = RecordedToolLifecycle::Finish {
            call_id: input.call_id.to_string(),
            tool_name: input.tool_name.clone(),
            outcome: input.outcome,
        };
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(record);
        })
    }
}

#[test]
fn handler_looks_up_namespaced_aliases_explicitly() {
    let namespace = "mcp__codex_apps__gmail";
    let tool_name = "gmail_get_recent_emails";
    let plain_name = codex_tools::ToolName::plain(tool_name);
    let namespaced_name = codex_tools::ToolName::namespaced(namespace, tool_name);
    let plain_handler = Arc::new(TestHandler {
        tool_name: plain_name.clone(),
    }) as Arc<dyn CoreToolRuntime>;
    let namespaced_handler = Arc::new(TestHandler {
        tool_name: namespaced_name.clone(),
    }) as Arc<dyn CoreToolRuntime>;
    let registry = ToolRegistry::new(HashMap::from([
        (plain_name.clone(), Arc::clone(&plain_handler)),
        (namespaced_name.clone(), Arc::clone(&namespaced_handler)),
    ]));

    let plain = registry.tool(&plain_name);
    let namespaced = registry.tool(&namespaced_name);
    let missing_namespaced = registry.tool(&codex_tools::ToolName::namespaced(
        "mcp__codex_apps__calendar",
        tool_name,
    ));

    assert_eq!(plain.is_some(), true);
    assert_eq!(namespaced.is_some(), true);
    assert_eq!(missing_namespaced.is_none(), true);
    assert!(
        plain
            .as_ref()
            .is_some_and(|handler| Arc::ptr_eq(handler, &plain_handler))
    );
    assert!(
        namespaced
            .as_ref()
            .is_some_and(|handler| Arc::ptr_eq(handler, &namespaced_handler))
    );
}

#[tokio::test]
async fn function_tools_expose_default_hook_payloads_and_rewrites() -> anyhow::Result<()> {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let tool_name = codex_tools::ToolName::namespaced("functions.", "echo");
    let handler = TestHandler {
        tool_name: tool_name.clone(),
    };
    let invocation = ToolInvocation {
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "message": "hello" }).to_string(),
        },
        ..test_invocation(Arc::new(session), Arc::new(turn), "call-1", tool_name)
    };
    let output =
        crate::tools::context::FunctionToolOutput::from_text("echoed".to_string(), Some(true));

    assert_eq!(
        handler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("functions.echo"),
            tool_input: serde_json::json!({ "message": "hello" }),
        })
    );
    assert_eq!(
        handler.post_tool_use_payload(&invocation, &output),
        Some(PostToolUsePayload {
            tool_name: HookToolName::new("functions.echo"),
            tool_use_id: "call-1".to_string(),
            tool_input: serde_json::json!({ "message": "hello" }),
            tool_response: serde_json::json!("echoed"),
        })
    );

    let invocation = handler
        .with_updated_hook_input(invocation, serde_json::json!({ "message": "rewritten" }))?;
    let ToolPayload::Function { arguments } = invocation.payload else {
        panic!("generic rewritten function payload should remain function-shaped");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&arguments)?,
        serde_json::json!({ "message": "rewritten" })
    );

    Ok(())
}

#[tokio::test]
async fn function_hook_input_defaults_empty_arguments_to_object() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let tool_name = codex_tools::ToolName::plain("echo");
    let handler = TestHandler {
        tool_name: tool_name.clone(),
    };
    let invocation = ToolInvocation {
        payload: ToolPayload::Function {
            arguments: "  ".to_string(),
        },
        ..test_invocation(Arc::new(session), Arc::new(turn), "call-1", tool_name)
    };

    assert_eq!(
        handler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("echo"),
            tool_input: serde_json::json!({}),
        })
    );
}

#[tokio::test]
async fn spawn_agent_function_tools_use_agent_matcher_alias() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    let hook_payloads = [
        codex_tools::ToolName::plain("spawn_agent"),
        codex_tools::ToolName::namespaced(MULTI_AGENT_V1_NAMESPACE, "spawn_agent"),
    ]
    .into_iter()
    .map(|tool_name| {
        let handler = TestHandler {
            tool_name: tool_name.clone(),
        };
        let invocation = ToolInvocation {
            payload: ToolPayload::Function {
                arguments: serde_json::json!({ "message": "inspect this repo" }).to_string(),
            },
            ..test_invocation(Arc::clone(&session), Arc::clone(&turn), "call-1", tool_name)
        };
        handler.pre_tool_use_payload(&invocation)
    })
    .collect::<Vec<_>>();

    assert_eq!(
        hook_payloads,
        vec![
            Some(PreToolUsePayload {
                tool_name: HookToolName::spawn_agent(),
                tool_input: serde_json::json!({ "message": "inspect this repo" }),
            }),
            Some(PreToolUsePayload {
                tool_name: HookToolName::spawn_agent(),
                tool_input: serde_json::json!({ "message": "inspect this repo" }),
            }),
        ]
    );
}

#[tokio::test]
async fn code_mode_wait_does_not_expose_default_hook_payloads() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let output = crate::tools::context::FunctionToolOutput::from_text("ok".to_string(), Some(true));

    let wait = crate::tools::handlers::CodeModeWaitHandler;
    let wait_invocation = test_invocation(
        Arc::new(session),
        Arc::new(turn),
        "wait-call",
        wait.tool_name(),
    );
    assert_eq!(wait.pre_tool_use_payload(&wait_invocation), None);
    assert_eq!(wait.post_tool_use_payload(&wait_invocation, &output), None);
}

#[tokio::test]
async fn write_stdin_does_not_expose_default_pre_tool_use_payload() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;

    let write_stdin = crate::tools::handlers::WriteStdinHandler;
    let invocation = test_invocation(
        Arc::new(session),
        Arc::new(turn),
        "write-stdin-call",
        write_stdin.tool_name(),
    );

    assert_eq!(write_stdin.pre_tool_use_payload(&invocation), None);
}

#[test]
fn post_tool_use_feedback_output_keeps_code_mode_result_typed() {
    let result = AnyToolResult {
        call_id: "call-1".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
        result: Box::new(PostToolUseFeedbackOutput {
            original: Box::new(codex_tools::JsonToolOutput::new(
                serde_json::json!({ "typed": true }),
            )),
            model_visible: crate::tools::context::FunctionToolOutput::from_text(
                "hook feedback".to_string(),
                /*success*/ None,
            ),
        }),
        post_tool_use_payload: None,
    };

    assert_eq!(
        result.into_response(),
        ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                "hook feedback".to_string()
            ),
        }
    );

    let result = AnyToolResult {
        call_id: "call-1".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
        result: Box::new(PostToolUseFeedbackOutput {
            original: Box::new(codex_tools::JsonToolOutput::new(
                serde_json::json!({ "typed": true }),
            )),
            model_visible: crate::tools::context::FunctionToolOutput::from_text(
                "hook feedback".to_string(),
                /*success*/ None,
            ),
        }),
        post_tool_use_payload: None,
    };

    assert_eq!(
        result.code_mode_result(),
        serde_json::json!({ "typed": true })
    );
}

#[tokio::test]
async fn dispatch_notifies_tool_lifecycle_contributors() -> anyhow::Result<()> {
    let (mut session, turn) = crate::session::tests::make_session_and_context().await;
    let records = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::<crate::config::Config>::new();
    builder.tool_lifecycle_contributor(Arc::new(ToolLifecycleRecorder {
        records: Arc::clone(&records),
    }));
    session.services.extensions = Arc::new(builder.build());

    let ok_tool = codex_tools::ToolName::plain("ok_tool");
    let failing_tool = codex_tools::ToolName::plain("failing_tool");
    let ok_handler = Arc::new(LifecycleTestHandler {
        tool_name: ok_tool.clone(),
        result: LifecycleTestResult::Ok { success: false },
    }) as Arc<dyn CoreToolRuntime>;
    let failing_handler = Arc::new(LifecycleTestHandler {
        tool_name: failing_tool.clone(),
        result: LifecycleTestResult::Err,
    }) as Arc<dyn CoreToolRuntime>;
    let registry = ToolRegistry::new(HashMap::from([
        (ok_tool.clone(), ok_handler),
        (failing_tool.clone(), failing_handler),
    ]));
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    registry
        .dispatch_any(test_invocation(
            Arc::clone(&session),
            Arc::clone(&turn),
            "ok-call",
            ok_tool.clone(),
        ))
        .await?;
    let err = match registry
        .dispatch_any(test_invocation(
            Arc::clone(&session),
            Arc::clone(&turn),
            "failing-call",
            failing_tool.clone(),
        ))
        .await
    {
        Ok(_) => panic!("failing handler should return an error"),
        Err(err) => err,
    };
    assert_eq!(err.to_string(), "handler failed");

    let expected = vec![
        RecordedToolLifecycle::Start {
            call_id: "ok-call".to_string(),
            tool_name: ok_tool.clone(),
        },
        RecordedToolLifecycle::Finish {
            call_id: "ok-call".to_string(),
            tool_name: ok_tool,
            outcome: codex_extension_api::ToolCallOutcome::Completed { success: false },
        },
        RecordedToolLifecycle::Start {
            call_id: "failing-call".to_string(),
            tool_name: failing_tool.clone(),
        },
        RecordedToolLifecycle::Finish {
            call_id: "failing-call".to_string(),
            tool_name: failing_tool,
            outcome: codex_extension_api::ToolCallOutcome::Failed {
                handler_executed: true,
            },
        },
    ];
    let actual = records
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .drain(..)
        .collect::<Vec<_>>();
    assert_eq!(expected, actual);

    Ok(())
}

fn test_invocation(
    session: Arc<crate::session::session::Session>,
    turn: Arc<crate::session::turn_context::TurnContext>,
    call_id: &str,
    tool_name: codex_tools::ToolName,
) -> ToolInvocation {
    ToolInvocation {
        session,
        turn,
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(tokio::sync::Mutex::new(
            crate::turn_diff_tracker::TurnDiffTracker::new(),
        )),
        call_id: call_id.to_string(),
        tool_name,
        source: crate::tools::context::ToolCallSource::Direct,
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    }
}
