use super::*;
use pretty_assertions::assert_eq;

struct TestHandler {
    tool_name: codex_tools::ToolName,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for TestHandler {
    fn tool_name(&self) -> codex_tools::ToolName {
        self.tool_name.clone()
    }

    async fn handle(
        &self,
        _invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        Ok(Box::new(
            crate::tools::context::FunctionToolOutput::from_text("ok".to_string(), Some(true)),
        ))
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

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for LifecycleTestHandler {
    fn tool_name(&self) -> codex_tools::ToolName {
        self.tool_name.clone()
    }

    async fn handle(
        &self,
        _invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        match self.result.clone() {
            LifecycleTestResult::Ok { success } => Ok(Box::new(
                crate::tools::context::FunctionToolOutput::from_text(
                    "ok".to_string(),
                    Some(success),
                ),
            )),
            LifecycleTestResult::Err => Err(FunctionCallError::RespondToModel(
                "handler failed".to_string(),
            )),
        }
    }
}

impl CoreToolRuntime for LifecycleTestHandler {}

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
async fn default_pre_tool_use_payload_uses_function_arguments() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let handler = TestHandler {
        tool_name: codex_tools::ToolName::plain("update_plan"),
    };
    let arguments = serde_json::json!({
        "plan": [{
            "step": "look around",
            "status": "in_progress"
        }]
    });
    let invocation = test_invocation_with_payload(
        session.into(),
        turn.into(),
        "call-plan",
        codex_tools::ToolName::plain("update_plan"),
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    );

    assert_eq!(
        handler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("update_plan"),
            tool_input: arguments,
        })
    );
}

#[tokio::test]
async fn default_with_updated_hook_input_rewrites_function_arguments() -> anyhow::Result<()> {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let handler = TestHandler {
        tool_name: codex_tools::ToolName::plain("update_plan"),
    };
    let invocation = test_invocation(
        session.into(),
        turn.into(),
        "call-plan",
        codex_tools::ToolName::plain("update_plan"),
    );
    let updated_input = serde_json::json!({
        "plan": [{
            "step": "look again",
            "status": "completed"
        }]
    });

    let updated = handler.with_updated_hook_input(invocation, updated_input.clone())?;
    let ToolPayload::Function { arguments } = updated.payload else {
        panic!("expected rewritten function payload");
    };
    let actual: serde_json::Value = serde_json::from_str(&arguments)?;
    assert_eq!(actual, updated_input);

    Ok(())
}

#[tokio::test]
async fn default_with_updated_hook_input_rewrites_tool_search_arguments() -> anyhow::Result<()> {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let handler = TestHandler {
        tool_name: codex_tools::ToolName::plain("tool_search"),
    };
    let invocation = test_invocation_with_payload(
        session.into(),
        turn.into(),
        "call-search",
        codex_tools::ToolName::plain("tool_search"),
        ToolPayload::ToolSearch {
            arguments: codex_protocol::models::SearchToolCallParams {
                query: "first".to_string(),
                limit: None,
            },
        },
    );

    let updated = handler.with_updated_hook_input(
        invocation,
        serde_json::json!({
            "query": "second",
            "limit": 3
        }),
    )?;
    let ToolPayload::ToolSearch { arguments } = updated.payload else {
        panic!("expected rewritten tool_search payload");
    };
    assert_eq!(
        arguments,
        codex_protocol::models::SearchToolCallParams {
            query: "second".to_string(),
            limit: Some(3),
        }
    );

    Ok(())
}

#[tokio::test]
async fn default_post_tool_use_payload_uses_model_visible_output() {
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let handler = TestHandler {
        tool_name: codex_tools::ToolName::plain("update_plan"),
    };
    let invocation = test_invocation_with_payload(
        session.into(),
        turn.into(),
        "call-plan",
        codex_tools::ToolName::plain("update_plan"),
        ToolPayload::Function {
            arguments: serde_json::json!({"plan": []}).to_string(),
        },
    );
    let output = crate::tools::context::FunctionToolOutput::from_text(
        "plan updated".to_string(),
        Some(true),
    );

    assert_eq!(
        handler.post_tool_use_payload(&invocation, &output),
        Some(PostToolUsePayload {
            tool_name: HookToolName::new("update_plan"),
            tool_use_id: "call-plan".to_string(),
            tool_input: serde_json::json!({"plan": []}),
            tool_response: serde_json::json!("plan updated"),
        })
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
    test_invocation_with_payload(
        session,
        turn,
        call_id,
        tool_name,
        ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    )
}

fn test_invocation_with_payload(
    session: Arc<crate::session::session::Session>,
    turn: Arc<crate::session::turn_context::TurnContext>,
    call_id: &str,
    tool_name: codex_tools::ToolName,
    payload: ToolPayload,
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
        payload,
    }
}
