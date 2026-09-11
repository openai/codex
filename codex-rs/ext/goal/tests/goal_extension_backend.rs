#![recursion_limit = "256"]
#![allow(clippy::expect_used)]

use codex_utils_absolute_path::test_support::PathExt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;
use std::time::Duration;

use codex_analytics::AnalyticsEventsClient;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::FunctionCallError;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::ThreadResumeInput;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolPayload;
use codex_extension_api::TurnErrorInput;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_goal_extension::GoalExtensionConfig;
use codex_goal_extension::GoalObjectiveUpdate;
use codex_goal_extension::GoalRuntimeHandle;
use codex_goal_extension::GoalService;
use codex_goal_extension::GoalSetRequest;
use codex_goal_extension::GoalTokenBudgetUpdate;
use codex_goal_extension::install_with_backend;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TruncationPolicy;
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
    let invocation = tool_call(
        "create_goal",
        "call-create-goal",
        json!({
            "objective": "ship goal extension backend",
            "token_budget": 123,
        }),
    );
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
async fn installed_goal_tools_apply_maximum_token_budget() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.thread_store.insert(GoalExtensionConfig {
        enabled: true,
        max_goal_token_budget: Some(100),
    });
    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");

    let result = create_tool
        .handle(tool_call(
            "create_goal",
            "call-oversized-goal",
            json!({ "objective": "oversized goal", "token_budget": 101 }),
        ))
        .await;
    let error = match result {
        Ok(_) => panic!("goal budget above the configured maximum should fail"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        FunctionCallError::RespondToModel(
            "goal token budget 101 exceeds the maximum allowed goal token budget of 100"
                .to_string()
        )
    );
    assert_eq!(
        runtime.thread_goals().get_thread_goal(thread_id).await?,
        None
    );

    let invocation = tool_call(
        "create_goal",
        "call-default-goal-budget",
        json!({ "objective": "default goal budget" }),
    );
    let output = create_tool.handle(invocation.clone()).await?;
    assert_eq!(
        output.code_mode_result(&invocation.payload)["goal"]["tokenBudget"],
        json!(100)
    );
    Ok(())
}

#[tokio::test]
async fn ephemeral_goal_tools_preserve_specs_but_reject_execution() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    let tools = installed_tools_with_start(
        runtime.clone(),
        thread_id,
        SessionSource::Cli,
        /*persistent_thread_state_available*/ false,
    )
    .await;

    let parent_tools = installed_tools(runtime, ThreadId::new()).await;
    assert_eq!(
        tools.iter().map(|tool| tool.spec()).collect::<Vec<_>>(),
        parent_tools
            .iter()
            .map(|tool| tool.spec())
            .collect::<Vec<_>>(),
    );
    for tool in tools {
        let Err(error) = tool
            .handle(tool_call(&tool.tool_name().name, "ephemeral", json!({})))
            .await
        else {
            panic!("ephemeral goal execution should fail");
        };
        assert_eq!(
            error,
            FunctionCallError::RespondToModel(
                "Goal tools require a persistent thread.".to_string()
            )
        );
    }
    Ok(())
}

#[tokio::test]
async fn goal_tools_hidden_for_review_subagents() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    let tools = installed_tools_with_start(
        runtime,
        thread_id,
        SessionSource::SubAgent(SubAgentSource::Review),
        /*persistent_thread_state_available*/ true,
    )
    .await;

    assert_eq!(Vec::<String>::new(), tool_names(&tools));
    Ok(())
}

#[tokio::test]
async fn installed_goal_tools_only_replace_complete_goal() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime, thread_id).await?;
    let tools = harness.tools();

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
            "cannot create a new goal because this thread has an unfinished goal; complete the existing goal first"
                .to_string()
        )
    );

    let update_tool = tool_by_name(&tools, "update_goal");
    update_tool
        .handle(tool_call(
            "update_goal",
            "call-complete-goal",
            json!({ "status": "complete" }),
        ))
        .await?;

    let invocation = tool_call(
        "create_goal",
        "call-create-goal-3",
        json!({ "objective": "replacement goal" }),
    );
    let output = create_tool.handle(invocation.clone()).await?;
    let result = output.code_mode_result(&invocation.payload);

    assert_eq!(json!("replacement goal"), result["goal"]["objective"]);
    assert_eq!(json!("active"), result["goal"]["status"]);
    assert_eq!(json!(0), result["goal"]["tokensUsed"]);
    Ok(())
}

#[tokio::test]
async fn create_goal_resets_baseline_before_turn_stop_accounting() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness
        .start_turn(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 100, /*cached_input_tokens*/ 10,
                /*output_tokens*/ 30, /*reasoning_output_tokens*/ 5,
                /*total_tokens*/ 135,
            ),
        )
        .await;
    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 120, /*cached_input_tokens*/ 14,
                /*output_tokens*/ 42, /*reasoning_output_tokens*/ 8,
                /*total_tokens*/ 162,
            ),
        )
        .await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 127, /*cached_input_tokens*/ 16,
                /*output_tokens*/ 52, /*reasoning_output_tokens*/ 10,
                /*total_tokens*/ 189,
            ),
        )
        .await;
    harness.stop_turn("turn-1").await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(15, goal.tokens_used);
    assert_eq!(ThreadGoalStatus::Active, protocol_status(goal.status));
    Ok(())
}

#[tokio::test]
async fn tool_finish_accounts_active_goal_progress_and_emits_event() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 20, /*cached_input_tokens*/ 5, /*output_tokens*/ 8,
                /*reasoning_output_tokens*/ 2, /*total_tokens*/ 30,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell", "shell")
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(23, goal.tokens_used);

    assert_eq!(
        vec![CapturedGoalEvent {
            event_id: "call-shell".to_string(),
            turn_id: Some("turn-1".to_string()),
            status: ThreadGoalStatus::Active,
            tokens_used: 23,
        }],
        harness.sink.goal_events()
    );
    Ok(())
}

#[tokio::test]
async fn parallel_tool_finish_accounts_active_goal_progress_once() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness
        .start_turn(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 100, /*cached_input_tokens*/ 0,
                /*output_tokens*/ 0, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 100,
            ),
        )
        .await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 130, /*cached_input_tokens*/ 0,
                /*output_tokens*/ 0, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 130,
            ),
        )
        .await;

    tokio::join!(
        harness.notify_tool_finish("turn-1", "call-shell-1", "shell"),
        harness.notify_tool_finish("turn-1", "call-shell-2", "shell"),
    );

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(30, goal.tokens_used);

    assert_eq!(
        vec![CapturedGoalEvent {
            event_id: "call-shell-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            status: ThreadGoalStatus::Active,
            tokens_used: 30,
        }],
        harness.sink.goal_events()
    );
    Ok(())
}

#[tokio::test]
async fn spawned_descendant_usage_exhausts_root_goal_budget_once() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let child = harness.spawn_child(ThreadId::new()).await?;
    child.start_turn("child-turn", &TokenUsage::default()).await;
    let grandchild = child
        .spawn_child_with_source(ThreadId::new(), SubAgentSource::Review)
        .await?;
    grandchild
        .start_turn("grandchild-turn", &TokenUsage::default())
        .await;
    let tools = harness.tools();
    tool_by_name(&tools, "create_goal")
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "account for the entire agent tree", "token_budget": 62 }),
        ))
        .await?;

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 12, /*cached_input_tokens*/ 2, /*output_tokens*/ 4,
                /*reasoning_output_tokens*/ 0, /*total_tokens*/ 16,
            ),
        )
        .await;
    let first_child_usage = input_token_usage(/*input_tokens*/ 23);
    child
        .record_token_usage_with_last("child-turn", &first_child_usage, &first_child_usage)
        .await;
    child
        .record_token_usage_with_last(
            "child-turn",
            &input_token_usage(/*input_tokens*/ 36),
            &input_token_usage(/*input_tokens*/ 13),
        )
        .await;
    let grandchild_usage = input_token_usage(/*input_tokens*/ 12);
    grandchild
        .record_token_usage_with_last("grandchild-turn", &grandchild_usage, &grandchild_usage)
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell", "shell")
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(62, goal.tokens_used);
    assert_eq!(codex_state::ThreadGoalStatus::BudgetLimited, goal.status);
    Ok(())
}

#[tokio::test]
async fn grandchild_usage_rolls_up_after_parent_runtime_unloads() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    tool_by_name(&tools, "create_goal")
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "account for evicted agent trees" }),
        ))
        .await?;

    let child = harness.spawn_child(ThreadId::new()).await?;
    child.stop_thread().await;
    let grandchild = child.spawn_child(ThreadId::new()).await?;
    grandchild
        .start_turn("grandchild-turn", &TokenUsage::default())
        .await;
    let usage = input_token_usage(/*input_tokens*/ 23);
    grandchild
        .record_token_usage_with_last("grandchild-turn", &usage, &usage)
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell", "shell")
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(23, goal.tokens_used);
    Ok(())
}

#[tokio::test]
async fn subagent_usage_resets_when_root_goal_is_replaced() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;
    let child = harness.spawn_child(ThreadId::new()).await?;
    child.start_turn("child-turn", &TokenUsage::default()).await;

    let previous_usage = input_token_usage(/*input_tokens*/ 10);
    child
        .record_token_usage_with_last("child-turn", &previous_usage, &previous_usage)
        .await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-first-goal",
            json!({ "objective": "first goal" }),
        ))
        .await?;
    child
        .record_token_usage_with_last(
            "child-turn",
            &input_token_usage(/*input_tokens*/ 35),
            &input_token_usage(/*input_tokens*/ 25),
        )
        .await;
    let completion = tool_call(
        "update_goal",
        "call-complete-first-goal",
        json!({ "status": "complete" }),
    );
    let completed = tool_by_name(&tools, "update_goal")
        .handle(completion.clone())
        .await?;
    assert_eq!(
        json!(25),
        completed.code_mode_result(&completion.payload)["goal"]["tokensUsed"]
    );

    child
        .record_token_usage_with_last(
            "child-turn",
            &input_token_usage(/*input_tokens*/ 55),
            &input_token_usage(/*input_tokens*/ 20),
        )
        .await;
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-second-goal",
            json!({ "objective": "replacement goal" }),
        ))
        .await?;
    child
        .record_token_usage_with_last(
            "child-turn",
            &input_token_usage(/*input_tokens*/ 64),
            &input_token_usage(/*input_tokens*/ 9),
        )
        .await;
    harness.stop_turn("turn-1").await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("replacement goal should exist"))?;
    assert_eq!("replacement goal", goal.objective);
    assert_eq!(9, goal.tokens_used);
    Ok(())
}

#[tokio::test]
async fn budget_limited_goal_keeps_accruing_until_turn_stop() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({
                "objective": "ship goal extension backend",
                "token_budget": 25,
            }),
        ))
        .await?;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 20, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 10, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 30,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell", "shell")
        .await;
    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 24, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 16, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 40,
            ),
        )
        .await;
    harness.stop_turn("turn-1").await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(35, goal.tokens_used);
    assert_eq!(codex_state::ThreadGoalStatus::BudgetLimited, goal.status);

    assert_eq!(
        vec![
            CapturedGoalEvent {
                event_id: "call-shell".to_string(),
                turn_id: Some("turn-1".to_string()),
                status: ThreadGoalStatus::BudgetLimited,
                tokens_used: 25,
            },
            CapturedGoalEvent {
                event_id: "turn-1:turn-stop".to_string(),
                turn_id: Some("turn-1".to_string()),
                status: ThreadGoalStatus::BudgetLimited,
                tokens_used: 35,
            },
        ],
        harness.sink.goal_events()
    );

    Ok(())
}

#[tokio::test]
async fn budget_limited_goal_keeps_accounting_after_later_tool_finish() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({
                "objective": "ship goal extension backend",
                "token_budget": 25,
            }),
        ))
        .await?;

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 20, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 10, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 30,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell-1", "shell")
        .await;
    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 24, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 16, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 40,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell-2", "shell")
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(35, goal.tokens_used);
    assert_eq!(codex_state::ThreadGoalStatus::BudgetLimited, goal.status);
    Ok(())
}

#[tokio::test]
async fn turn_error_usage_limit_accounts_progress_and_clears_accounting() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 20, /*cached_input_tokens*/ 5, /*output_tokens*/ 8,
                /*reasoning_output_tokens*/ 2, /*total_tokens*/ 30,
            ),
        )
        .await;
    harness
        .notify_turn_error("turn-1", CodexErrorInfo::UsageLimitExceeded)
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(23, goal.tokens_used);
    assert_eq!(codex_state::ThreadGoalStatus::UsageLimited, goal.status);
    assert_eq!(
        vec![
            CapturedGoalEvent {
                event_id: "turn-1:usage-limit-progress".to_string(),
                turn_id: Some("turn-1".to_string()),
                status: ThreadGoalStatus::Active,
                tokens_used: 23,
            },
            CapturedGoalEvent {
                event_id: "turn-1:usage-limit".to_string(),
                turn_id: Some("turn-1".to_string()),
                status: ThreadGoalStatus::UsageLimited,
                tokens_used: 23,
            },
        ],
        harness.sink.goal_events()
    );

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 50, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 20, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 70,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell-after-usage-limit", "shell")
        .await;
    harness.stop_turn("turn-1").await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(23, goal.tokens_used);
    assert_eq!(codex_state::ThreadGoalStatus::UsageLimited, goal.status);
    Ok(())
}

#[tokio::test]
async fn turn_error_blocks_goal() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    tool_by_name(&tools, "create_goal")
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;

    harness
        .notify_turn_error("turn-1", CodexErrorInfo::Other)
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(codex_state::ThreadGoalStatus::Blocked, goal.status);
    Ok(())
}

#[tokio::test]
async fn failed_execution_turns_block_goal_unless_a_tool_succeeds() -> anyhow::Result<()> {
    for (recovery_turn, status_only_turn, blocking_turn) in
        [(None, None, 3), (Some(2), None, 5), (None, Some(2), 4)]
    {
        let runtime = test_runtime().await?;
        let thread_id = test_thread_id()?;
        seed_thread_metadata(runtime.as_ref(), thread_id).await?;
        let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
        harness.start_turn("turn-1", &TokenUsage::default()).await;

        let tools = harness.tools();
        tool_by_name(&tools, "create_goal")
            .handle(tool_call(
                "create_goal",
                "call-create-goal",
                json!({ "objective": "ship goal extension backend" }),
            ))
            .await?;

        for turn in 1..=blocking_turn {
            let turn_id = format!("turn-{turn}");
            if turn > 1 {
                harness.start_turn(&turn_id, &TokenUsage::default()).await;
            }
            if status_only_turn != Some(turn) {
                harness
                    .notify_tool_finish_with_outcome(
                        &turn_id,
                        &format!("call-exec-{turn}"),
                        "exec",
                        ToolCallOutcome::Failed {
                            handler_executed: true,
                        },
                    )
                    .await;
            }
            if recovery_turn == Some(turn) {
                harness
                    .notify_tool_finish(&turn_id, "call-recovery", "shell")
                    .await;
            }
            harness.stop_turn(&turn_id).await;

            let goal = runtime
                .thread_goals()
                .get_thread_goal(thread_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
            let expected = if turn == blocking_turn {
                codex_state::ThreadGoalStatus::Blocked
            } else {
                codex_state::ThreadGoalStatus::Active
            };
            assert_eq!(expected, goal.status);
        }
    }

    Ok(())
}

#[tokio::test]
async fn usage_limit_budget_limited_goal_accounts_remaining_progress() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({
                "objective": "ship goal extension backend",
                "token_budget": 25,
            }),
        ))
        .await?;

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 20, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 10, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 30,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell", "shell")
        .await;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 24, /*cached_input_tokens*/ 5,
                /*output_tokens*/ 16, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 40,
            ),
        )
        .await;
    harness
        .runtime_handle()
        .usage_limit_active_goal_for_turn("turn-1")
        .await
        .map_err(anyhow::Error::msg)?;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(35, goal.tokens_used);
    assert_eq!(codex_state::ThreadGoalStatus::UsageLimited, goal.status);
    assert_eq!(
        vec![
            CapturedGoalEvent {
                event_id: "turn-1:usage-limit-progress".to_string(),
                turn_id: Some("turn-1".to_string()),
                status: ThreadGoalStatus::BudgetLimited,
                tokens_used: 35,
            },
            CapturedGoalEvent {
                event_id: "turn-1:usage-limit".to_string(),
                turn_id: Some("turn-1".to_string()),
                status: ThreadGoalStatus::UsageLimited,
                tokens_used: 35,
            },
        ],
        harness.sink.goal_events()
    );
    Ok(())
}

#[tokio::test]
async fn usage_limit_plan_turn_does_not_stop_goal() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;

    harness
        .start_turn_with_mode("turn-plan", ModeKind::Plan, &TokenUsage::default())
        .await;
    harness.sink.clear();
    harness
        .runtime_handle()
        .usage_limit_active_goal_for_turn("turn-plan")
        .await
        .map_err(anyhow::Error::msg)?;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(codex_state::ThreadGoalStatus::Active, goal.status);
    assert_eq!(Vec::<CapturedGoalEvent>::new(), harness.sink.goal_events());
    Ok(())
}

#[tokio::test]
async fn usage_limit_stale_turn_does_not_stop_current_goal() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;
    harness.stop_turn("turn-1").await;
    harness.start_turn("turn-2", &TokenUsage::default()).await;
    harness.sink.clear();

    harness
        .runtime_handle()
        .usage_limit_active_goal_for_turn("turn-1")
        .await
        .map_err(anyhow::Error::msg)?;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(codex_state::ThreadGoalStatus::Active, goal.status);
    assert_eq!(Vec::<CapturedGoalEvent>::new(), harness.sink.goal_events());
    Ok(())
}

#[tokio::test]
async fn update_goal_can_stop_and_accounts_final_progress() -> anyhow::Result<()> {
    for (status, token_budget, expected_status, state_status) in [
        (
            ThreadGoalStatus::Blocked,
            100_i64,
            ThreadGoalStatus::Blocked,
            codex_state::ThreadGoalStatus::Blocked,
        ),
        (
            ThreadGoalStatus::Paused,
            100,
            ThreadGoalStatus::Paused,
            codex_state::ThreadGoalStatus::Paused,
        ),
        (
            ThreadGoalStatus::Paused,
            20,
            ThreadGoalStatus::BudgetLimited,
            codex_state::ThreadGoalStatus::BudgetLimited,
        ),
    ] {
        let runtime = test_runtime().await?;
        let thread_id = test_thread_id()?;
        seed_thread_metadata(runtime.as_ref(), thread_id).await?;
        let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
        harness.start_turn("turn-1", &TokenUsage::default()).await;

        let tools = harness.tools();
        let create_tool = tool_by_name(&tools, "create_goal");
        create_tool
            .handle(tool_call(
                "create_goal",
                "call-create-goal",
                json!({ "objective": "ship goal extension backend", "token_budget": token_budget }),
            ))
            .await?;
        harness.sink.clear();

        harness
            .record_token_usage(
                "turn-1",
                &token_usage(
                    /*input_tokens*/ 20, /*cached_input_tokens*/ 5,
                    /*output_tokens*/ 8, /*reasoning_output_tokens*/ 2,
                    /*total_tokens*/ 30,
                ),
            )
            .await;
        let update_tool = tool_by_name(&tools, "update_goal");
        let invocation = tool_call(
            "update_goal",
            "call-update-goal",
            json!({ "status": status }),
        );
        let output = update_tool.handle(invocation.clone()).await?;
        let result = output.code_mode_result(&invocation.payload);

        assert_eq!(
            result,
            json!({
                "goal": {
                    "threadId": thread_id,
                    "objective": "ship goal extension backend",
                    "status": expected_status,
                    "tokenBudget": token_budget,
                    "tokensUsed": 23,
                    "timeUsedSeconds": 0,
                    "createdAt": result["goal"]["createdAt"],
                    "updatedAt": result["goal"]["updatedAt"],
                },
                "remainingTokens": (token_budget - 23).max(0),
                "completionBudgetReport": serde_json::Value::Null,
            })
        );

        let goal = runtime
            .thread_goals()
            .get_thread_goal(thread_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
        assert_eq!(23, goal.tokens_used);
        assert_eq!(state_status, goal.status);

        assert_eq!(
            vec![
                CapturedGoalEvent {
                    event_id: "call-update-goal".to_string(),
                    turn_id: Some("turn-1".to_string()),
                    status: if token_budget > 23 {
                        ThreadGoalStatus::Active
                    } else {
                        ThreadGoalStatus::BudgetLimited
                    },
                    tokens_used: 23,
                },
                CapturedGoalEvent {
                    event_id: "call-update-goal".to_string(),
                    turn_id: Some("turn-1".to_string()),
                    status: expected_status,
                    tokens_used: 23,
                },
            ],
            harness.sink.goal_events()
        );
        harness
            .record_token_usage(
                "turn-1",
                &token_usage(
                    /*input_tokens*/ 40, /*cached_input_tokens*/ 5,
                    /*output_tokens*/ 10, /*reasoning_output_tokens*/ 2,
                    /*total_tokens*/ 52,
                ),
            )
            .await;
        harness
            .notify_tool_finish("turn-1", "call-shell", "shell")
            .await;
        harness.stop_turn("turn-1").await;
        assert_eq!(
            Some(goal),
            runtime.thread_goals().get_thread_goal(thread_id).await?
        );
    }
    Ok(())
}

#[tokio::test]
async fn update_goal_rejects_resume_and_system_limit_statuses() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let tools = installed_tools(runtime, thread_id).await;
    let update_tool = tool_by_name(&tools, "update_goal");
    for status in ["active", "budgetLimited", "usageLimited"] {
        let result = update_tool
            .handle(tool_call(
                "update_goal",
                "call-update-goal",
                json!({ "status": status }),
            ))
            .await;
        let error = match result {
            Ok(_) => panic!("agent must not set {status}"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            FunctionCallError::RespondToModel(
                "update_goal can only mark the existing goal complete, blocked, or paused at the user's explicit request; resume, budget-limited, and usage-limited status changes are controlled by the user or system".to_string()
            )
        );
    }
    Ok(())
}

#[tokio::test]
async fn external_goal_mutation_start_accounts_active_goal_progress() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 20, /*cached_input_tokens*/ 5, /*output_tokens*/ 8,
                /*reasoning_output_tokens*/ 2, /*total_tokens*/ 30,
            ),
        )
        .await;
    harness
        .runtime_handle()
        .prepare_external_goal_mutation()
        .await
        .map_err(anyhow::Error::msg)?;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(23, goal.tokens_used);
    assert_eq!(
        vec![CapturedGoalEvent {
            event_id: "turn-1:external-goal-mutation".to_string(),
            turn_id: Some("turn-1".to_string()),
            status: ThreadGoalStatus::Active,
            tokens_used: 23,
        }],
        harness.sink.goal_events()
    );
    Ok(())
}

#[tokio::test]
async fn goal_service_external_set_active_preserves_concurrent_usage() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness
        .start_turn(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 100, /*cached_input_tokens*/ 0,
                /*output_tokens*/ 0, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 100,
            ),
        )
        .await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "old objective" }),
        ))
        .await?;
    let child = harness.spawn_child(ThreadId::new()).await?;
    child.start_turn("child-turn", &TokenUsage::default()).await;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 120, /*cached_input_tokens*/ 0,
                /*output_tokens*/ 0, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 120,
            ),
        )
        .await;
    let outcome = harness
        .goal_service
        .set_thread_goal(
            runtime.as_ref(),
            GoalSetRequest {
                thread_id,
                objective: GoalObjectiveUpdate::Set("new objective"),
                status: Some(ThreadGoalStatus::Active),
                token_budget: GoalTokenBudgetUpdate::Keep,
                max_goal_token_budget: None,
            },
        )
        .await?;
    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 125, /*cached_input_tokens*/ 0,
                /*output_tokens*/ 0, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 125,
            ),
        )
        .await;
    let child_usage = input_token_usage(/*input_tokens*/ 23);
    child
        .record_token_usage_with_last("child-turn", &child_usage, &child_usage)
        .await;
    outcome.apply_runtime_effects(&harness.goal_service).await;

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 130, /*cached_input_tokens*/ 0,
                /*output_tokens*/ 0, /*reasoning_output_tokens*/ 0,
                /*total_tokens*/ 130,
            ),
        )
        .await;
    harness
        .notify_tool_finish("turn-1", "call-shell", "shell")
        .await;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(53, goal.tokens_used);
    Ok(())
}

#[tokio::test]
async fn thread_stop_unregisters_goal_runtime_from_service() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;

    let tools = harness.tools();
    let create_tool = tool_by_name(&tools, "create_goal");
    create_tool
        .handle(tool_call(
            "create_goal",
            "call-create-goal",
            json!({ "objective": "ship goal extension backend" }),
        ))
        .await?;
    harness.sink.clear();

    harness
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 10, /*cached_input_tokens*/ 0, /*output_tokens*/ 0,
                /*reasoning_output_tokens*/ 0, /*total_tokens*/ 10,
            ),
        )
        .await;
    harness.stop_thread().await;

    assert!(
        harness
            .goal_service
            .clear_thread_goal(runtime.as_ref(), thread_id)
            .await?
    );
    assert_eq!(Vec::<CapturedGoalEvent>::new(), harness.sink.goal_events());
    Ok(())
}

#[tokio::test]
async fn thread_resume_rehydrates_active_goal_idle_accounting() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    runtime
        .thread_goals()
        .replace_thread_goal(
            thread_id,
            "ship goal extension backend",
            codex_state::ThreadGoalStatus::Active,
            /*token_budget*/ None,
        )
        .await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;

    harness.resume_thread().await;
    let child = harness.spawn_child(ThreadId::new()).await?;
    child.start_turn("child-turn", &TokenUsage::default()).await;
    let usage = input_token_usage(/*input_tokens*/ 23);
    child
        .record_token_usage_with_last("child-turn", &usage, &usage)
        .await;
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    harness
        .runtime_handle()
        .prepare_external_goal_mutation()
        .await
        .map_err(anyhow::Error::msg)?;

    let goal = runtime
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    assert_eq!(ThreadGoalStatus::Active, protocol_status(goal.status));
    assert_eq!(23, goal.tokens_used);
    assert!(
        goal.time_used_seconds >= 1,
        "resumed idle accounting should add elapsed wall-clock time"
    );
    assert_eq!(
        vec![CapturedGoalEvent {
            event_id: format!("{thread_id}:external-goal-mutation"),
            turn_id: None,
            status: ThreadGoalStatus::Active,
            tokens_used: 23,
        }],
        harness.sink.goal_events()
    );
    Ok(())
}

#[tokio::test]
async fn goal_service_sets_gets_and_clears_thread_goal() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let api = GoalService::new();

    let set = api
        .set_thread_goal(
            runtime.as_ref(),
            GoalSetRequest {
                thread_id,
                objective: GoalObjectiveUpdate::Set(" ship goal API ownership "),
                status: None,
                token_budget: GoalTokenBudgetUpdate::Set(Some(123)),
                max_goal_token_budget: None,
            },
        )
        .await?;
    let get = api
        .get_thread_goal(runtime.as_ref(), thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("goal should exist"))?;
    let metadata = runtime
        .get_thread(thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("seeded thread metadata should exist"))?;

    assert_eq!(set.goal, get);
    assert_eq!("ship goal API ownership", get.objective);
    assert_eq!(ThreadGoalStatus::Active, get.status);
    assert_eq!(Some(123), get.token_budget);
    assert_eq!(Some("ship goal API ownership"), metadata.preview.as_deref());

    assert!(api.clear_thread_goal(runtime.as_ref(), thread_id).await?);
    assert_eq!(
        None,
        api.get_thread_goal(runtime.as_ref(), thread_id).await?
    );
    assert!(!api.clear_thread_goal(runtime.as_ref(), thread_id).await?);
    Ok(())
}

#[tokio::test]
async fn goal_service_enforces_maximum_token_budget_on_creation_and_updates() -> anyhow::Result<()>
{
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let service = GoalService::new();

    let goal = service
        .set_thread_goal(
            runtime.as_ref(),
            GoalSetRequest {
                thread_id,
                objective: GoalObjectiveUpdate::Set("bounded goal"),
                status: None,
                token_budget: GoalTokenBudgetUpdate::Keep,
                max_goal_token_budget: Some(100),
            },
        )
        .await?;
    assert_eq!(goal.goal.token_budget, Some(100));

    let error = service
        .set_thread_goal(
            runtime.as_ref(),
            GoalSetRequest {
                thread_id,
                objective: GoalObjectiveUpdate::Keep,
                status: None,
                token_budget: GoalTokenBudgetUpdate::Set(Some(101)),
                max_goal_token_budget: Some(100),
            },
        )
        .await
        .expect_err("goal budget above the configured maximum should fail");
    assert_eq!(
        error.to_string(),
        "goal token budget 101 exceeds the maximum allowed goal token budget of 100"
    );
    assert_eq!(
        service
            .get_thread_goal(runtime.as_ref(), thread_id)
            .await?
            .expect("goal should remain unchanged")
            .token_budget,
        Some(100)
    );

    let goal = service
        .set_thread_goal(
            runtime.as_ref(),
            GoalSetRequest {
                thread_id,
                objective: GoalObjectiveUpdate::Keep,
                status: None,
                token_budget: GoalTokenBudgetUpdate::Set(Some(99)),
                max_goal_token_budget: Some(100),
            },
        )
        .await?;
    assert_eq!(goal.goal.token_budget, Some(99));

    let goal = service
        .set_thread_goal(
            runtime.as_ref(),
            GoalSetRequest {
                thread_id,
                objective: GoalObjectiveUpdate::Keep,
                status: None,
                token_budget: GoalTokenBudgetUpdate::Set(None),
                max_goal_token_budget: Some(100),
            },
        )
        .await?;
    assert_eq!(goal.goal.token_budget, Some(100));
    Ok(())
}

async fn installed_tools(
    runtime: Arc<codex_state::StateRuntime>,
    thread_id: ThreadId,
) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
    installed_tools_with_start(
        runtime,
        thread_id,
        SessionSource::Cli,
        /*persistent_thread_state_available*/ true,
    )
    .await
}

async fn installed_tools_with_start(
    runtime: Arc<codex_state::StateRuntime>,
    thread_id: ThreadId,
    session_source: SessionSource,
    persistent_thread_state_available: bool,
) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    let goal_service = Arc::new(GoalService::new());
    install_with_backend(
        &mut builder,
        runtime,
        AnalyticsEventsClient::disabled(),
        /*metrics_client*/ None,
        Weak::new(),
        goal_service,
        |_| GoalExtensionConfig {
            enabled: true,
            max_goal_token_budget: None,
        },
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = ExtensionData::new(thread_id.to_string());
    for contributor in registry.thread_lifecycle_contributors() {
        contributor
            .on_thread_start(ThreadStartInput {
                config: &(),
                session_source: &session_source,
                persistent_thread_state_available,
                environments: &[],
                mcp_resource_client: None,
                extension_metrics: None,
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

fn tool_names(tools: &[Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>]) -> Vec<String> {
    tools.iter().map(|tool| tool.tool_name().name).collect()
}

struct GoalExtensionHarness {
    registry: Arc<codex_extension_api::ExtensionRegistry<()>>,
    session_store: ExtensionData,
    thread_store: ExtensionData,
    goal_service: Arc<GoalService>,
    sink: Arc<RecordingEventSink>,
}

impl GoalExtensionHarness {
    async fn new(
        runtime: Arc<codex_state::StateRuntime>,
        thread_id: ThreadId,
    ) -> anyhow::Result<Self> {
        let sink = Arc::new(RecordingEventSink::default());
        let mut builder = ExtensionRegistryBuilder::<()>::with_event_sink(sink.clone());
        let goal_service = Arc::new(GoalService::new());
        install_with_backend(
            &mut builder,
            runtime,
            AnalyticsEventsClient::disabled(),
            /*metrics_client*/ None,
            Weak::new(),
            Arc::clone(&goal_service),
            |_| GoalExtensionConfig {
                enabled: true,
                max_goal_token_budget: None,
            },
        );
        let registry = Arc::new(builder.build());
        let session_store = ExtensionData::new(thread_id.to_string());
        let thread_store = ExtensionData::new(thread_id.to_string());
        let session_source = SessionSource::Cli;
        for contributor in registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_start(ThreadStartInput {
                    config: &(),
                    session_source: &session_source,
                    persistent_thread_state_available: true,
                    environments: &[],
                    mcp_resource_client: None,
                    extension_metrics: None,
                    session_store: &session_store,
                    thread_store: &thread_store,
                })
                .await;
        }
        Ok(Self {
            registry,
            session_store,
            thread_store,
            goal_service,
            sink,
        })
    }

    async fn spawn_child(&self, thread_id: ThreadId) -> anyhow::Result<Self> {
        let session_source = SubAgentSource::ThreadSpawn {
            parent_thread_id: ThreadId::from_string(self.thread_store.level_id())?,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        };
        self.spawn_child_with_source(thread_id, session_source)
            .await
    }

    async fn spawn_child_with_source(
        &self,
        thread_id: ThreadId,
        session_source: SubAgentSource,
    ) -> anyhow::Result<Self> {
        let session_store = ExtensionData::new(self.session_store.level_id());
        let thread_store = ExtensionData::new(thread_id.to_string());
        let session_source = SessionSource::SubAgent(session_source);
        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_start(ThreadStartInput {
                    config: &(),
                    session_source: &session_source,
                    persistent_thread_state_available: true,
                    environments: &[],
                    mcp_resource_client: None,
                    extension_metrics: None,
                    session_store: &session_store,
                    thread_store: &thread_store,
                })
                .await;
        }
        Ok(Self {
            registry: Arc::clone(&self.registry),
            session_store,
            thread_store,
            goal_service: Arc::clone(&self.goal_service),
            sink: Arc::clone(&self.sink),
        })
    }

    fn tools(&self) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        self.registry
            .tool_contributors()
            .iter()
            .flat_map(|contributor| contributor.tools(&self.session_store, &self.thread_store))
            .collect()
    }

    async fn start_turn(&self, turn_id: &str, usage: &TokenUsage) {
        self.start_turn_with_mode(turn_id, ModeKind::Default, usage)
            .await;
    }

    async fn start_turn_with_mode(&self, turn_id: &str, mode: ModeKind, usage: &TokenUsage) {
        let turn_store = ExtensionData::new(turn_id);
        let mut collaboration_mode = default_collaboration_mode();
        collaboration_mode.mode = mode;
        for contributor in self.registry.turn_lifecycle_contributors() {
            contributor
                .on_turn_start(TurnStartInput {
                    turn_id,
                    collaboration_mode: &collaboration_mode,
                    token_usage_at_turn_start: usage,
                    session_store: &self.session_store,
                    thread_store: &self.thread_store,
                    turn_store: &turn_store,
                })
                .await;
        }
    }

    async fn stop_turn(&self, turn_id: &str) {
        let turn_store = ExtensionData::new(turn_id);
        for contributor in self.registry.turn_lifecycle_contributors() {
            contributor
                .on_turn_stop(TurnStopInput {
                    session_store: &self.session_store,
                    thread_store: &self.thread_store,
                    turn_store: &turn_store,
                })
                .await;
        }
    }

    async fn record_token_usage(&self, turn_id: &str, usage: &TokenUsage) {
        self.record_token_usage_with_last(turn_id, usage, &TokenUsage::default())
            .await;
    }

    async fn record_token_usage_with_last(
        &self,
        turn_id: &str,
        usage: &TokenUsage,
        last_usage: &TokenUsage,
    ) {
        let turn_store = ExtensionData::new(turn_id);
        let token_usage = TokenUsageInfo {
            total_token_usage: usage.clone(),
            last_token_usage: last_usage.clone(),
            model_context_window: None,
        };
        for contributor in self.registry.token_usage_contributors() {
            contributor
                .on_token_usage(
                    &self.session_store,
                    &self.thread_store,
                    &turn_store,
                    &token_usage,
                )
                .await;
        }
    }

    async fn resume_thread(&self) {
        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_resume(ThreadResumeInput {
                    session_store: &self.session_store,
                    thread_store: &self.thread_store,
                })
                .await;
        }
    }

    async fn stop_thread(&self) {
        for contributor in self.registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_stop(ThreadStopInput {
                    session_store: &self.session_store,
                    thread_store: &self.thread_store,
                })
                .await;
        }
    }

    async fn notify_tool_finish(&self, turn_id: &str, call_id: &str, tool_name: &str) {
        self.notify_tool_finish_with_outcome(
            turn_id,
            call_id,
            tool_name,
            ToolCallOutcome::Completed { success: true },
        )
        .await;
    }

    async fn notify_tool_finish_with_outcome(
        &self,
        turn_id: &str,
        call_id: &str,
        tool_name: &str,
        outcome: ToolCallOutcome,
    ) {
        let turn_store = ExtensionData::new(turn_id);
        let tool_name = codex_extension_api::ToolName::plain(tool_name);
        for contributor in self.registry.tool_lifecycle_contributors() {
            contributor
                .on_tool_finish(ToolFinishInput {
                    session_store: &self.session_store,
                    thread_store: &self.thread_store,
                    turn_store: &turn_store,
                    turn_id,
                    call_id,
                    tool_name: &tool_name,
                    source: ToolCallSource::Direct,
                    outcome,
                })
                .await;
        }
    }

    async fn notify_turn_error(&self, turn_id: &str, error: CodexErrorInfo) {
        let turn_store = ExtensionData::new(turn_id);
        for contributor in self.registry.turn_lifecycle_contributors() {
            contributor
                .on_turn_error(TurnErrorInput {
                    turn_id,
                    error: error.clone(),
                    session_store: &self.session_store,
                    thread_store: &self.thread_store,
                    turn_store: &turn_store,
                })
                .await;
        }
    }

    fn runtime_handle(&self) -> Arc<GoalRuntimeHandle> {
        self.thread_store
            .get::<GoalRuntimeHandle>()
            .expect("goal runtime handle should exist")
    }
}

fn tool_by_name<'a>(
    tools: &'a [Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>],
    name: &str,
) -> &'a Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>> {
    tools
        .iter()
        .find(|tool| tool.tool_name().namespace.is_none() && tool.tool_name().name == name)
        .expect("requested goal tool should exist")
}

fn tool_call(tool_name: &str, call_id: &str, arguments: serde_json::Value) -> ToolCall<'static> {
    ToolCall {
        turn_id: "turn-1".to_string(),
        call_id: call_id.to_string(),
        tool_name: codex_extension_api::ToolName::plain(tool_name),
        model: "gpt-test".to_string(),
        codex_turn_metadata: None,
        truncation_policy: TruncationPolicy::Bytes(1024),
        source: ToolCallSource::Direct,
        conversation_history: codex_extension_api::ConversationHistory::default(),
        turn_item_emitter: Arc::new(NoopTurnItemEmitter),
        environments: Vec::new(),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}

async fn test_runtime() -> anyhow::Result<Arc<codex_state::StateRuntime>> {
    let tempdir = TempDir::new()?;
    codex_state::StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(tempdir.keep().as_path().abs()),
        "test-provider".to_string(),
    )
    .await
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
            .sqlite()
            .home()
            .join(format!("rollout-{thread_id}.jsonl")),
        chrono::Utc::now(),
        SessionSource::Cli,
    );
    runtime.upsert_thread(&builder.build("test-provider")).await
}

#[derive(Debug, Default)]
struct RecordingEventSink {
    events: Mutex<Vec<Event>>,
}

impl RecordingEventSink {
    fn goal_events(&self) -> Vec<CapturedGoalEvent> {
        self.events()
            .iter()
            .filter_map(|event| match &event.msg {
                EventMsg::ThreadGoalUpdated(updated) => Some(CapturedGoalEvent {
                    event_id: event.id.clone(),
                    turn_id: updated.turn_id.clone(),
                    status: updated.goal.status,
                    tokens_used: updated.goal.tokens_used,
                }),
                _ => None,
            })
            .collect()
    }

    fn clear(&self) {
        self.events().clear();
    }

    fn events(&self) -> std::sync::MutexGuard<'_, Vec<Event>> {
        self.events.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ExtensionEventSink for RecordingEventSink {
    fn emit(&self, event: Event) {
        self.events().push(event);
    }

    fn emit_warning(&self, _warning: ExtensionWarning) {
        panic!("goal extension tests do not emit warnings");
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedGoalEvent {
    event_id: String,
    turn_id: Option<String>,
    status: ThreadGoalStatus,
    tokens_used: i64,
}

fn default_collaboration_mode() -> CollaborationMode {
    CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: "gpt-5".to_string(),
            reasoning_effort: None,
            developer_instructions: None,
        },
    }
}

fn token_usage(
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
    total_tokens: i64,
) -> TokenUsage {
    TokenUsage {
        input_tokens,
        cached_input_tokens,
        cache_write_input_tokens: 0,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
        codex_rollout_budget_units: None,
    }
}

fn input_token_usage(input_tokens: i64) -> TokenUsage {
    TokenUsage {
        input_tokens,
        total_tokens: input_tokens,
        ..TokenUsage::default()
    }
}

fn protocol_status(status: codex_state::ThreadGoalStatus) -> ThreadGoalStatus {
    match status {
        codex_state::ThreadGoalStatus::Active => ThreadGoalStatus::Active,
        codex_state::ThreadGoalStatus::Paused => ThreadGoalStatus::Paused,
        codex_state::ThreadGoalStatus::Blocked => ThreadGoalStatus::Blocked,
        codex_state::ThreadGoalStatus::UsageLimited => ThreadGoalStatus::UsageLimited,
        codex_state::ThreadGoalStatus::BudgetLimited => ThreadGoalStatus::BudgetLimited,
        codex_state::ThreadGoalStatus::Complete => ThreadGoalStatus::Complete,
    }
}
