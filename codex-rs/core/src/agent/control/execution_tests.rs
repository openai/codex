use crate::agent::AgentControl;
use crate::config::test_config;
use codex_features::Feature;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

#[tokio::test]
async fn v2_execution_slots_count_active_subagent_turns() {
    let control = AgentControl::default();
    let mut config = test_config().await;
    let _ = config.features.enable(Feature::MultiAgentV2);
    config.multi_agent_v2.max_concurrent_threads_per_session = 2;
    let source = SessionSource::SubAgent(SubAgentSource::Other("worker".to_string()));

    let first = control
        .reserve_v2_execution_slot(&config, MultiAgentVersion::V2, &source)
        .expect("first active turn should fit")
        .expect("v2 subagent should reserve a slot");
    let Err(err) = control.reserve_v2_execution_slot(&config, MultiAgentVersion::V2, &source)
    else {
        panic!("second active turn should exceed the derived non-root cap");
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    drop(first);
    control
        .reserve_v2_execution_slot(&config, MultiAgentVersion::V2, &source)
        .expect("slot should be released when the running task drops")
        .expect("v2 subagent should reserve a slot");
}

#[tokio::test]
async fn v2_execution_slots_ignore_root_and_v1_turns() {
    let control = AgentControl::default();
    let mut config = test_config().await;
    let _ = config.features.enable(Feature::MultiAgentV2);
    config.multi_agent_v2.max_concurrent_threads_per_session = 1;

    assert!(
        control
            .reserve_v2_execution_slot(&config, MultiAgentVersion::V2, &SessionSource::Cli)
            .expect("root should not count")
            .is_none()
    );
    assert!(
        control
            .reserve_v2_execution_slot(
                &config,
                MultiAgentVersion::V1,
                &SessionSource::SubAgent(SubAgentSource::Other("worker".to_string())),
            )
            .expect("v1 should not count")
            .is_none()
    );
}
