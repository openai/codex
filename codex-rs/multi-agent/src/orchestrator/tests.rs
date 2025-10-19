use super::*;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::SessionSource;
use codex_core::CodexAuth;
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
        Arc::new(AuthManager::from_auth(CodexAuth::from_api_key("test"))),
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
