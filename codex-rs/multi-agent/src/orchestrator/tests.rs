use super::*;
use codex_core::CodexAuth;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::SessionSource;
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::test]
async fn delegate_started_event_carries_owner() {
    let tmp = tempdir().expect("tempdir");
    let global = tmp.path().join("codex");
    std::fs::create_dir_all(global.join("log")).unwrap();
    std::fs::create_dir_all(global.join("sessions")).unwrap();
    std::fs::create_dir_all(global.join("history")).unwrap();
    std::fs::create_dir_all(global.join("mcp")).unwrap();
    std::fs::create_dir_all(global.join("tmp")).unwrap();

    let orchestrator = Arc::new(AgentOrchestrator::new(
        &global,
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test")),
        SessionSource::Cli,
        CliConfigOverrides::default(),
        ConfigOverrides::default(),
        vec![AgentId::parse("critic").unwrap()],
        1,
        ShadowConfig::disabled(),
    ));

    let owner_id = "owner-conv".to_string();
    let parent_run_id = "parent-run".to_string();
    orchestrator
        .run_owner_conversations
        .lock()
        .await
        .insert(parent_run_id.clone(), owner_id.clone());

    let request = DelegateRequest {
        agent_id: AgentId::parse("critic").unwrap(),
        prompt: DelegatePrompt::new("hello"),
        user_initial: Vec::new(),
        parent_run_id: Some(parent_run_id),
        mode: DelegateInvocationMode::Immediate,
        caller_conversation_id: None,
        conversation_id: None,
    };

    let mut events = orchestrator.subscribe().await;
    let run_id = orchestrator.delegate(request).await.unwrap();

    while let Some(event) = events.recv().await {
        if let DelegateEvent::Started {
            run_id: started_run,
            owner_conversation_id,
            ..
        } = event
        {
            assert_eq!(started_run, run_id);
            assert_eq!(owner_conversation_id, owner_id);
            assert_eq!(
                orchestrator
                    .owner_conversation_for_run(&run_id)
                    .await
                    .as_deref(),
                Some(owner_conversation_id.as_str())
            );
            break;
        }
    }
}

#[test]
fn paginate_session_list_returns_remaining_entries() {
    use std::time::Duration;
    use std::time::SystemTime;

    let base = SystemTime::UNIX_EPOCH;
    let summaries = vec![
        DelegateSessionSummary {
            conversation_id: "conv-3".to_string(),
            agent_id: AgentId::parse("gamma").unwrap(),
            last_interacted_at: base + Duration::from_secs(30),
            cwd: PathBuf::from("/tmp/gamma"),
            mode: DelegateSessionMode::Standard,
        },
        DelegateSessionSummary {
            conversation_id: "conv-2".to_string(),
            agent_id: AgentId::parse("beta").unwrap(),
            last_interacted_at: base + Duration::from_secs(20),
            cwd: PathBuf::from("/tmp/beta"),
            mode: DelegateSessionMode::Standard,
        },
        DelegateSessionSummary {
            conversation_id: "conv-1".to_string(),
            agent_id: AgentId::parse("alpha").unwrap(),
            last_interacted_at: base + Duration::from_secs(10),
            cwd: PathBuf::from("/tmp/alpha"),
            mode: DelegateSessionMode::Standard,
        },
    ];

    let (first_page, cursor) =
        paginate_session_summaries(&summaries, None, 2).expect("first page ok");
    assert_eq!(
        first_page
            .into_iter()
            .map(|summary| summary.conversation_id.as_str())
            .collect::<Vec<_>>(),
        vec!["conv-3", "conv-2"]
    );
    let cursor = cursor.expect("cursor for next page");

    let (second_page, next_cursor) =
        paginate_session_summaries(&summaries, Some(cursor), 2).expect("second page ok");
    assert_eq!(
        second_page
            .into_iter()
            .map(|summary| summary.conversation_id.as_str())
            .collect::<Vec<_>>(),
        vec!["conv-1"],
        "expected final session to appear on second page"
    );
    assert!(next_cursor.is_none());
}

#[tokio::test]
async fn follow_up_shadow_events_do_not_duplicate() {
    let temp_home = tempdir().expect("tempdir");
    let global = temp_home.path().join("codex");
    for dir in ["log", "sessions", "history", "mcp", "tmp"] {
        std::fs::create_dir_all(global.join(dir)).expect("create dir");
    }

    let orchestrator = Arc::new(AgentOrchestrator::new(
        &global,
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test")),
        SessionSource::Cli,
        CliConfigOverrides::default(),
        ConfigOverrides::default(),
        vec![AgentId::parse("critic").unwrap()],
        1,
        ShadowConfig::apply_defaults(true, None, None, false),
    ));

    let agent_id = AgentId::parse("critic").unwrap();
    let conversation_id = "conv-follow-up";
    orchestrator
        .shadow_manager
        .register_session(conversation_id, &agent_id)
        .await
        .expect("register session");

    let event = Event {
        id: "event-1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "delegate output".into(),
        }),
    };

    orchestrator
        .record_shadow_event_internal(
            Some(&agent_id),
            conversation_id,
            &event,
            ShadowRecordMode::Normal,
        )
        .await;

    let baseline = orchestrator
        .shadow_manager
        .session_summary(conversation_id)
        .await
        .expect("summary")
        .metrics
        .events;
    assert!(baseline > 0);

    orchestrator
        .record_shadow_event_internal(
            Some(&agent_id),
            conversation_id,
            &event,
            ShadowRecordMode::FollowUp,
        )
        .await;

    let after = orchestrator
        .shadow_manager
        .session_summary(conversation_id)
        .await
        .expect("summary")
        .metrics
        .events;
    assert_eq!(after, baseline);
}

#[tokio::test]
async fn follow_up_should_preserve_parent_before_registration() {
    let temp_home = tempdir().expect("tempdir");
    let global = temp_home.path().join("codex");
    for dir in ["log", "sessions", "history", "mcp", "tmp"] {
        std::fs::create_dir_all(global.join(dir)).expect("create dir");
    }

    let orchestrator = AgentOrchestrator::new(
        &global,
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test")),
        SessionSource::Cli,
        CliConfigOverrides::default(),
        ConfigOverrides::default(),
        vec![AgentId::parse("critic").unwrap()],
        2,
        ShadowConfig::disabled(),
    );

    let conversation_id = "reuse-conv".to_string();
    let original_parent = "run-parent".to_string();
    let new_run = "run-follow-up".to_string();

    orchestrator
        .conversation_runs
        .lock()
        .await
        .insert(conversation_id.clone(), original_parent.clone());

    let resolved = orchestrator
        .parent_run_for_follow_up(&conversation_id, None)
        .await;
    assert_eq!(
        resolved.as_deref(),
        Some(original_parent.as_str()),
        "follow-up resolution should see the existing parent before registration"
    );

    orchestrator
        .register_run_conversation(&new_run, &conversation_id)
        .await;

    assert_eq!(
        resolved.as_deref(),
        Some(original_parent.as_str()),
        "captured parent run id should remain intact for follow-up bookkeeping"
    );
}
