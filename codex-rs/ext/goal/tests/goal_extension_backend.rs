use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolPayload;
use codex_goal_extension::install_with_backend;
use codex_protocol::ThreadId;
use codex_protocol::ToolName;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn installed_goal_tools_create_goal_and_fill_empty_preview() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let tools = installed_tools(runtime.clone(), thread_id).await;

    let create_tool = tool_by_name(&tools, "create_goal");
    let invocation = ToolCall {
        call_id: "call-create-goal".to_string(),
        tool_name: ToolName::plain("create_goal"),
        payload: ToolPayload::Function {
            arguments: json!({
                "objective": "ship goal extension backend",
                "token_budget": 123,
            })
            .to_string(),
        },
    };
    let output = create_tool.handle(invocation.clone()).await?;
    let result = output.code_mode_result(&invocation.payload);
    assert_eq!(
        result,
        json!({
            "goal": {
                "threadId": thread_id,
                "objective": "ship goal extension backend",
                "status": "active",
                "tokenBudget": 123,
                "tokensUsed": 0,
                "timeUsedSeconds": 0,
                "createdAt": result["goal"]["createdAt"],
                "updatedAt": result["goal"]["updatedAt"],
            },
            "remainingTokens": 123,
            "completionBudgetReport": serde_json::Value::Null,
        })
    );

    let metadata = runtime
        .get_thread(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("seeded thread metadata should exist"))?;
    assert_eq!(
        metadata.preview.as_deref(),
        Some("ship goal extension backend")
    );
    Ok(())
}

#[tokio::test]
async fn installed_goal_tools_reject_duplicate_goal_creation() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let tools = installed_tools(runtime, thread_id).await;

    let create_tool = tool_by_name(&tools, "create_goal");
    let first = tool_call(
        "create_goal",
        "call-create-goal-1",
        json!({ "objective": "first goal" }),
    );
    create_tool.handle(first).await?;

    let second = tool_call(
        "create_goal",
        "call-create-goal-2",
        json!({ "objective": "second goal" }),
    );
    let err = match create_tool.handle(second).await {
        Ok(_) => panic!("duplicate create should fail"),
        Err(err) => err,
    };

    assert_eq!(
        err,
        FunctionCallError::RespondToModel(
            "cannot create a new goal because this thread already has a goal; use update_goal only when the existing goal is complete"
                .to_string()
        )
    );
    Ok(())
}

async fn installed_tools(
    runtime: Arc<codex_state::StateRuntime>,
    thread_id: ThreadId,
) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    install_with_backend(&mut builder, runtime, |_| true);
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = ExtensionData::new(thread_id.to_string());
    for contributor in registry.thread_lifecycle_contributors() {
        contributor
            .on_thread_start(ThreadStartInput {
                config: &(),
                session_store: &session_store,
                thread_store: &thread_store,
            })
            .await;
    }

    registry
        .tool_contributors()
        .iter()
        .flat_map(|contributor| contributor.tools(&session_store, &thread_store))
        .collect()
}

fn tool_by_name<'a>(
    tools: &'a [Arc<dyn ToolExecutor<ToolCall>>],
    name: &str,
) -> &'a Arc<dyn ToolExecutor<ToolCall>> {
    tools
        .iter()
        .find(|tool| tool.tool_name().namespace.is_none() && tool.tool_name().name == name)
        .unwrap_or_else(|| panic!("missing tool {name}"))
}

fn tool_call(tool_name: &str, call_id: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        call_id: call_id.to_string(),
        tool_name: ToolName::plain(tool_name),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}

async fn test_runtime() -> anyhow::Result<Arc<codex_state::StateRuntime>> {
    let tempdir = TempDir::new()?;
    codex_state::StateRuntime::init(tempdir.keep(), "test-provider".to_string()).await
}

fn test_thread_id() -> anyhow::Result<ThreadId> {
    ThreadId::from_string("11111111-1111-4111-8111-111111111111").map_err(anyhow::Error::msg)
}

async fn seed_thread_metadata(
    runtime: &codex_state::StateRuntime,
    thread_id: ThreadId,
) -> anyhow::Result<()> {
    let builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        runtime
            .codex_home()
            .join(format!("rollout-{thread_id}.jsonl")),
        chrono::Utc::now(),
        SessionSource::Cli,
    );
    runtime.upsert_thread(&builder.build("test-provider")).await
}
