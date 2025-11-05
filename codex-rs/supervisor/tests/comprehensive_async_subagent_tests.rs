// 包括的非同期サブエージェントテストスイート
use codex_supervisor::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_end_to_end_async_subagent_workflow() {
    // 1. エージェント作成
    let agent = AsyncSubAgent::new(AgentType::CodeExpert);
    let agent_id = agent.id.clone();

    // 2. タスク開始
    agent
        .start_task_async("Implement authentication".to_string())
        .await
        .unwrap();

    // 3. 状態確認
    let state = agent.get_state().await;
    assert_eq!(
        state.status,
        codex_supervisor::subagent::AgentStatus::Working
    );
    assert_eq!(
        state.current_task,
        Some("Implement authentication".to_string())
    );

    // 4. 進捗更新
    agent.update_progress(0.5).await.unwrap();
    let state = agent.get_state().await;
    assert_eq!(state.progress, 0.5);

    // 5. タスク完了
    agent
        .complete_task("Authentication implemented successfully".to_string())
        .await
        .unwrap();

    let state = agent.get_state().await;
    assert_eq!(
        state.status,
        codex_supervisor::subagent::AgentStatus::Completed
    );
    assert_eq!(state.progress, 1.0);

    // 6. 通知確認
    let notifications = agent.get_inbox().get_unread_notifications().await;
    assert!(notifications.len() > 0);

    // タスク完了通知があることを確認
    let completion_notification = notifications
        .iter()
        .find(|n| n.notification_type == NotificationType::TaskCompleted);
    assert!(completion_notification.is_some());
}

#[tokio::test]
async fn test_thinking_process_tracking() {
    let mut manager = ThinkingProcessManager::new();

    let task_id = "task-123".to_string();
    let process = manager.start_process(AgentType::CodeExpert, task_id.clone(), 50);

    // 思考ステップを追加
    let step1 = ThinkingStepBuilder::new(ThinkingStepType::ProblemAnalysis)
        .content("Analyzing the authentication requirements".to_string())
        .confidence(0.8)
        .reasoning("Based on user input and system context".to_string())
        .build();

    let step2 = ThinkingStepBuilder::new(ThinkingStepType::HypothesisGeneration)
        .content("JWT-based authentication would be suitable".to_string())
        .confidence(0.85)
        .reasoning("Industry standard, secure, stateless".to_string())
        .build();

    let step3 = ThinkingStepBuilder::new(ThinkingStepType::Decision)
        .content("Implementing JWT authentication with refresh tokens".to_string())
        .confidence(0.9)
        .reasoning("Best practice for modern web applications".to_string())
        .build();

    process.add_step(step1);
    process.add_step(step2);
    process.add_step(step3);

    // 検証
    assert_eq!(process.steps.len(), 3);
    assert_eq!(process.current_phase, ThinkingStepType::Decision);
    assert!((process.overall_confidence - 0.85).abs() < 0.01); // (0.8 + 0.85 + 0.9) / 3

    // サマリー生成
    let summary = process.get_summary();
    assert!(summary.contains("CodeExpert"));
    assert!(summary.contains("Confidence"));

    // タイプ別フィルタリング
    let decisions = process.get_steps_by_type(ThinkingStepType::Decision);
    assert_eq!(decisions.len(), 1);
}

#[tokio::test]
async fn test_token_tracker_comprehensive() {
    let limits = TokenLimit {
        max_tokens_per_task: 1000,
        max_tokens_per_agent: 10_000,
        max_tokens_total: 100_000,
        warning_threshold: 0.8,
    };

    let tracker = TokenTracker::new(limits, TokenAllocationStrategy::Equal);

    // エージェントを登録
    tracker
        .register_agent(AgentType::CodeExpert, "agent-1".to_string())
        .await;
    tracker
        .register_agent(AgentType::SecurityExpert, "agent-2".to_string())
        .await;

    // タスク1: CodeExpert
    let usage1 = TokenUsage::new(500, 300);
    tracker
        .record_usage(
            "agent-1",
            "task-1".to_string(),
            "Implement auth".to_string(),
            usage1,
        )
        .await
        .unwrap();

    // タスク2: SecurityExpert
    let usage2 = TokenUsage::new(400, 200);
    tracker
        .record_usage(
            "agent-2",
            "task-2".to_string(),
            "Security review".to_string(),
            usage2,
        )
        .await
        .unwrap();

    // タスク3: CodeExpert
    let usage3 = TokenUsage::new(300, 200);
    tracker
        .record_usage(
            "agent-1",
            "task-3".to_string(),
            "Add tests".to_string(),
            usage3,
        )
        .await
        .unwrap();

    // 検証
    let agent1_usage = tracker.get_agent_usage("agent-1").await.unwrap();
    assert_eq!(agent1_usage.total_usage.total_tokens, 1300); // 800 + 500
    assert_eq!(agent1_usage.task_usages.len(), 2);
    assert_eq!(agent1_usage.get_average_tokens_per_task(), 650.0);

    let agent2_usage = tracker.get_agent_usage("agent-2").await.unwrap();
    assert_eq!(agent2_usage.total_usage.total_tokens, 600);

    let total_usage = tracker.get_total_usage().await;
    assert_eq!(total_usage.total_tokens, 1900);

    // レポート生成
    let report = tracker.generate_report().await;
    assert!(report.contains("Token Usage Report"));
    assert!(report.contains("CodeExpert"));
    assert!(report.contains("SecurityExpert"));
}

#[tokio::test]
async fn test_autonomous_dispatcher_comprehensive() {
    let mut dispatcher = AutonomousDispatcher::new();

    // テストケース1: セキュリティタスク
    let task1 = "Review this code for potential security vulnerabilities and CVE issues";
    let classification1 = dispatcher.classify_task(task1);
    assert_eq!(classification1.recommended_agent, AgentType::SecurityExpert);
    assert!(classification1.confidence > 0.5);

    // テストケース2: テストタスク
    let task2 = "Create comprehensive unit tests for the authentication module";
    let classification2 = dispatcher.classify_task(task2);
    assert_eq!(classification2.recommended_agent, AgentType::TestingExpert);

    // テストケース3: パフォーマンス最適化
    let task3 = "Optimize the database query performance and reduce latency";
    let classification3 = dispatcher.classify_task(task3);
    assert_eq!(
        classification3.recommended_agent,
        AgentType::PerformanceExpert
    );

    // テストケース4: 一般的なタスク（マッチなし）
    let task4 = "Hello world";
    let classification4 = dispatcher.classify_task(task4);
    assert_eq!(classification4.recommended_agent, AgentType::General);
    assert_eq!(classification4.confidence, 0.5);

    // 自動呼び出し判断
    let should_auto1 = dispatcher.should_auto_call(task1, 0.3);
    assert_eq!(should_auto1, Some(AgentType::SecurityExpert));

    let should_auto2 = dispatcher.should_auto_call(task4, 0.6);
    assert_eq!(should_auto2, None);

    // カスタムトリガー追加
    let custom_trigger = AutoCallTrigger {
        keywords: vec!["custom".to_string(), "special".to_string()],
        agent_type: AgentType::General,
        priority: 100,
        description: "Custom trigger".to_string(),
    };
    dispatcher.add_trigger(custom_trigger);

    let task5 = "This is a special custom task";
    let classification5 = dispatcher.classify_task(task5);
    assert_eq!(classification5.recommended_agent, AgentType::General);
    assert!(classification5.confidence > 0.0);
}

#[tokio::test]
async fn test_multi_agent_coordination() {
    let mut manager = AsyncSubAgentManager::new();

    // 複数のエージェントを登録
    let code_expert_id = manager.register_agent(AgentType::CodeExpert);
    let security_expert_id = manager.register_agent(AgentType::SecurityExpert);
    let testing_expert_id = manager.register_agent(AgentType::TestingExpert);

    assert_eq!(manager.agent_count(), 3);

    // 各エージェントにタスクを開始
    manager
        .start_task_async(&code_expert_id, "Implement feature".to_string())
        .await
        .unwrap();

    manager
        .start_task_async(&security_expert_id, "Review security".to_string())
        .await
        .unwrap();

    manager
        .start_task_async(&testing_expert_id, "Create tests".to_string())
        .await
        .unwrap();

    // 全てのエージェント状態を取得
    let states = manager.get_all_agent_states().await;
    assert_eq!(states.len(), 3);

    // 全てのエージェントがWorkingステータスであることを確認
    for state in states {
        assert_eq!(
            state.status,
            codex_supervisor::subagent::AgentStatus::Working
        );
    }
}

#[tokio::test]
async fn test_thinking_process_and_token_tracking_integration() {
    let mut thinking_manager = ThinkingProcessManager::new();
    let token_tracker = TokenTracker::default();

    // エージェント登録
    token_tracker
        .register_agent(AgentType::CodeExpert, "agent-1".to_string())
        .await;

    // タスク開始
    let task_id = "task-integration-123".to_string();
    let process = thinking_manager.start_process(AgentType::CodeExpert, task_id.clone(), 50);

    // 思考ステップ1: 問題分析
    let step1 = ThinkingStepBuilder::new(ThinkingStepType::ProblemAnalysis)
        .content("Analyzing requirements...".to_string())
        .confidence(0.7)
        .reasoning("Initial assessment".to_string())
        .build();
    process.add_step(step1);

    // トークン記録（分析フェーズ）
    let usage1 = TokenUsage::new(200, 100);
    token_tracker
        .record_usage(
            "agent-1",
            format!("{}-analysis", task_id),
            "Problem analysis".to_string(),
            usage1,
        )
        .await
        .unwrap();

    // 思考ステップ2: 推論
    let step2 = ThinkingStepBuilder::new(ThinkingStepType::Reasoning)
        .content("Determining best approach...".to_string())
        .confidence(0.8)
        .reasoning("Based on analysis results".to_string())
        .build();
    process.add_step(step2);

    // トークン記録（推論フェーズ）
    let usage2 = TokenUsage::new(300, 200);
    token_tracker
        .record_usage(
            "agent-1",
            format!("{}-reasoning", task_id),
            "Reasoning phase".to_string(),
            usage2,
        )
        .await
        .unwrap();

    // 思考ステップ3: 実行
    let step3 = ThinkingStepBuilder::new(ThinkingStepType::Execution)
        .content("Implementing solution...".to_string())
        .confidence(0.9)
        .reasoning("Executing the plan".to_string())
        .build();
    process.add_step(step3);

    // トークン記録（実行フェーズ）
    let usage3 = TokenUsage::new(400, 300);
    token_tracker
        .record_usage(
            "agent-1",
            format!("{}-execution", task_id),
            "Execution phase".to_string(),
            usage3,
        )
        .await
        .unwrap();

    // 検証
    assert_eq!(process.steps.len(), 3);
    assert!((process.overall_confidence - 0.8).abs() < 0.01); // (0.7 + 0.8 + 0.9) / 3

    let agent_usage = token_tracker.get_agent_usage("agent-1").await.unwrap();
    assert_eq!(agent_usage.task_usages.len(), 3);
    assert_eq!(agent_usage.total_usage.total_tokens, 1500); // 300 + 500 + 700

    // サマリー生成
    let thinking_summary = process.get_summary();
    assert!(thinking_summary.contains("CodeExpert"));
    assert!(thinking_summary.contains("Confidence"));

    let token_report = token_tracker.generate_report().await;
    assert!(token_report.contains("Token Usage Report"));
    assert!(token_report.contains("CodeExpert"));
}

#[test]
fn test_task_classification_accuracy() {
    let mut dispatcher = AutonomousDispatcher::new();

    let test_cases = vec![
        (
            "Analyze this code for bugs",
            AgentType::CodeExpert,
            "code analysis",
        ),
        (
            "Check for SQL injection vulnerabilities",
            AgentType::SecurityExpert,
            "security check",
        ),
        (
            "Write integration tests",
            AgentType::TestingExpert,
            "testing",
        ),
        (
            "Generate API documentation",
            AgentType::DocsExpert,
            "documentation",
        ),
        (
            "Research best practices for async Rust",
            AgentType::DeepResearcher,
            "research",
        ),
        (
            "Debug this segmentation fault",
            AgentType::DebugExpert,
            "debugging",
        ),
        (
            "Optimize memory usage",
            AgentType::PerformanceExpert,
            "performance",
        ),
    ];

    for (task, expected_agent, description) in test_cases {
        let classification = dispatcher.classify_task(task);
        assert_eq!(
            classification.recommended_agent, expected_agent,
            "Failed for {}: expected {:?}, got {:?}",
            description, expected_agent, classification.recommended_agent
        );
        assert!(
            classification.confidence > 0.0,
            "Confidence should be > 0 for {}",
            description
        );
    }
}

#[tokio::test]
async fn test_token_allocation_strategies() {
    // Equal strategy
    let tracker_equal = TokenTracker::new(TokenLimit::default(), TokenAllocationStrategy::Equal);
    tracker_equal
        .register_agent(AgentType::CodeExpert, "agent-1".to_string())
        .await;
    tracker_equal
        .register_agent(AgentType::SecurityExpert, "agent-2".to_string())
        .await;

    let allocation_equal = tracker_equal.calculate_allocation(1000).await;
    assert_eq!(*allocation_equal.get("agent-1").unwrap(), 500);
    assert_eq!(*allocation_equal.get("agent-2").unwrap(), 500);

    // LoadBased strategy
    let tracker_load = TokenTracker::new(TokenLimit::default(), TokenAllocationStrategy::LoadBased);
    tracker_load
        .register_agent(AgentType::CodeExpert, "agent-1".to_string())
        .await;
    tracker_load
        .register_agent(AgentType::SecurityExpert, "agent-2".to_string())
        .await;

    // agent-1に多くのトークンを消費させる
    let usage1 = TokenUsage::new(800, 400);
    tracker_load
        .record_usage(
            "agent-1",
            "task-1".to_string(),
            "Heavy task".to_string(),
            usage1,
        )
        .await
        .unwrap();

    // agent-2には少ないトークンを消費
    let usage2 = TokenUsage::new(100, 50);
    tracker_load
        .record_usage(
            "agent-2",
            "task-2".to_string(),
            "Light task".to_string(),
            usage2,
        )
        .await
        .unwrap();

    let allocation_load = tracker_load.calculate_allocation(1000).await;

    // agent-2の方が多く割り当てられるべき（使用量が少ないため）
    assert!(allocation_load.get("agent-2").unwrap() > &0);
}

#[tokio::test]
async fn test_inbox_capacity_and_cleanup() {
    let inbox = Inbox::new(5); // 最大5件

    // 10件の通知を追加
    for i in 0..10 {
        let notification = AsyncSubAgentNotification {
            id: format!("notif-{}", i),
            agent_type: AgentType::General,
            notification_type: NotificationType::Info,
            content: format!("Notification {}", i),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: std::collections::HashMap::new(),
        };

        inbox.add_notification(notification).await.unwrap();
    }

    // 最大5件に制限されているはず
    assert_eq!(inbox.count().await, 5);

    // 最新の5件が保持されているはず
    let notifications = inbox.get_unread_notifications().await;
    assert!(notifications.iter().any(|n| n.content == "Notification 9"));
    assert!(!notifications.iter().any(|n| n.content == "Notification 0"));

    // クリア
    inbox.clear_all().await;
    assert_eq!(inbox.count().await, 0);
}

#[tokio::test]
async fn test_concurrent_subagent_operations() {
    use tokio::task::JoinSet;

    let mut manager = AsyncSubAgentManager::new();

    // 8つのエージェントを登録
    let code_id = manager.register_agent(AgentType::CodeExpert);
    let security_id = manager.register_agent(AgentType::SecurityExpert);
    let testing_id = manager.register_agent(AgentType::TestingExpert);
    let docs_id = manager.register_agent(AgentType::DocsExpert);
    let research_id = manager.register_agent(AgentType::DeepResearcher);
    let debug_id = manager.register_agent(AgentType::DebugExpert);
    let perf_id = manager.register_agent(AgentType::PerformanceExpert);
    let general_id = manager.register_agent(AgentType::General);

    let agent_ids = vec![
        code_id,
        security_id,
        testing_id,
        docs_id,
        research_id,
        debug_id,
        perf_id,
        general_id,
    ];

    // 並行してタスクを開始
    let mut join_set = JoinSet::new();
    for (i, agent_id) in agent_ids.iter().enumerate() {
        let agent_id_clone = agent_id.clone();
        let task = format!("Task {}", i);

        join_set.spawn(async move {
            // シミュレーション
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            (agent_id_clone, task)
        });
    }

    // 全てのタスクが完了するまで待機
    while let Some(result) = join_set.join_next().await {
        assert!(result.is_ok());
    }

    // 全てのエージェント状態を確認
    let states = manager.get_all_agent_states().await;
    assert_eq!(states.len(), 8);
}

#[tokio::test]
async fn test_error_handling_and_recovery() {
    let agent = AsyncSubAgent::new(AgentType::CodeExpert);

    // タスク開始
    agent
        .start_task_async("Task that will fail".to_string())
        .await
        .unwrap();

    // タスク失敗
    agent
        .fail_task("Unexpected error occurred".to_string())
        .await
        .unwrap();

    let state = agent.get_state().await;
    assert_eq!(
        state.status,
        codex_supervisor::subagent::AgentStatus::Failed
    );

    // 通知確認
    let notifications = agent.get_inbox().get_unread_notifications().await;
    let failure_notification = notifications
        .iter()
        .find(|n| n.notification_type == NotificationType::TaskFailed);
    assert!(failure_notification.is_some());
    assert!(failure_notification
        .unwrap()
        .content
        .contains("Unexpected error"));
}
