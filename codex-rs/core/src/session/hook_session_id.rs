use std::collections::HashSet;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ThreadStore;
use tracing::warn;

use crate::agent::AgentControl;

pub(crate) async fn resolve_hook_session_id(
    thread_id: ThreadId,
    session_source: &SessionSource,
    agent_control: &AgentControl,
    thread_store: &dyn ThreadStore,
) -> ThreadId {
    let Some(mut parent_thread_id) = thread_spawn_parent_thread_id(session_source) else {
        return thread_id;
    };
    let mut visited = HashSet::from([thread_id]);

    loop {
        if !visited.insert(parent_thread_id) {
            warn!(%thread_id, %parent_thread_id, "detected a cycle while resolving hook session id");
            return parent_thread_id;
        }

        let parent_source = match agent_control
            .get_agent_config_snapshot(parent_thread_id)
            .await
        {
            Some(snapshot) => Some(snapshot.session_source),
            None => {
                match thread_store
                    .read_thread(ReadThreadParams {
                        thread_id: parent_thread_id,
                        include_archived: true,
                        include_history: false,
                    })
                    .await
                {
                    Ok(stored_thread) => Some(stored_thread.source),
                    Err(error) => {
                        warn!(
                            %thread_id,
                            %parent_thread_id,
                            ?error,
                            "failed to load parent thread metadata while resolving hook session id"
                        );
                        return parent_thread_id;
                    }
                }
            }
        };

        let Some(next_session_source) = parent_source else {
            return parent_thread_id;
        };
        let Some(next_parent_thread_id) = thread_spawn_parent_thread_id(&next_session_source)
        else {
            return parent_thread_id;
        };
        parent_thread_id = next_parent_thread_id;
    }
}

fn thread_spawn_parent_thread_id(session_source: &SessionSource) -> Option<ThreadId> {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) => Some(*parent_thread_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use codex_login::CodexAuth;
    use codex_protocol::protocol::Op;
    use codex_protocol::user_input::UserInput;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use crate::CodexThread;
    use crate::ThreadManager;
    use crate::config::Config;
    use crate::config::ConfigBuilder;

    fn text_input(text: &str) -> Op {
        vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }]
        .into()
    }

    struct HookSessionIdHarness {
        _home: TempDir,
        config: Config,
        manager: ThreadManager,
        control: AgentControl,
    }

    impl HookSessionIdHarness {
        async fn new() -> Self {
            let home = TempDir::new().expect("create temp dir");
            let config = ConfigBuilder::without_managed_config_for_tests()
                .codex_home(home.path().to_path_buf())
                .build()
                .await
                .expect("load default test config");
            let manager = ThreadManager::with_models_provider_and_home_for_tests(
                CodexAuth::from_api_key("dummy"),
                config.model_provider.clone(),
                config.codex_home.to_path_buf(),
                Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
            );
            let control = manager.agent_control();
            Self {
                _home: home,
                config,
                manager,
                control,
            }
        }

        async fn start_thread(&self) -> (ThreadId, Arc<CodexThread>) {
            let new_thread = self
                .manager
                .start_thread(self.config.clone())
                .await
                .expect("start thread");
            (new_thread.thread_id, new_thread.thread)
        }
    }

    #[tokio::test]
    async fn thread_spawn_subagents_share_the_root_hook_session_id() {
        let harness = HookSessionIdHarness::new().await;
        let (root_thread_id, root_thread) = harness.start_thread().await;
        let worker_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello worker"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: root_thread_id,
                    depth: 1,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
            )
            .await
            .expect("worker spawn should succeed");
        let tester_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello tester"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: worker_thread_id,
                    depth: 2,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
            )
            .await
            .expect("tester spawn should succeed");
        let worker_thread = harness
            .manager
            .get_thread(worker_thread_id)
            .await
            .expect("worker thread should exist");
        let tester_thread = harness
            .manager
            .get_thread(tester_thread_id)
            .await
            .expect("tester thread should exist");

        assert_eq!(root_thread.codex.session.hook_session_id, root_thread_id);
        assert_eq!(worker_thread.codex.session.hook_session_id, root_thread_id);
        assert_eq!(tester_thread.codex.session.hook_session_id, root_thread_id);
    }

    #[tokio::test]
    async fn resumed_thread_spawn_subagent_restores_the_root_hook_session_id() {
        let harness = HookSessionIdHarness::new().await;
        let (root_thread_id, root_thread) = harness.start_thread().await;
        let worker_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello worker"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: root_thread_id,
                    depth: 1,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
            )
            .await
            .expect("worker spawn should succeed");
        let tester_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello tester"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: worker_thread_id,
                    depth: 2,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
            )
            .await
            .expect("tester spawn should succeed");
        let report = harness
            .manager
            .shutdown_all_threads_bounded(Duration::from_secs(5))
            .await;
        assert_eq!(report.submit_failed, Vec::<ThreadId>::new());
        assert_eq!(report.timed_out, Vec::<ThreadId>::new());

        let resumed_tester_thread_id = harness
            .control
            .resume_agent_from_rollout(
                harness.config.clone(),
                tester_thread_id,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: worker_thread_id,
                    depth: 2,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                }),
            )
            .await
            .expect("resume tester thread should succeed");
        assert_eq!(resumed_tester_thread_id, tester_thread_id);

        let resumed_tester_thread = harness
            .manager
            .get_thread(tester_thread_id)
            .await
            .expect("resumed tester thread should exist");

        assert_eq!(root_thread.codex.session.hook_session_id, root_thread_id);
        assert_eq!(
            resumed_tester_thread.codex.session.hook_session_id,
            root_thread_id
        );
    }
}
