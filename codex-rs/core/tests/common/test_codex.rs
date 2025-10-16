use std::mem::swap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::delegate_tool::DelegateEventReceiver;
use codex_core::delegate_tool::DelegateToolAdapter;
use codex_core::delegate_tool::DelegateToolError;
use codex_core::delegate_tool::DelegateToolEvent;
use codex_core::delegate_tool::DelegateToolRequest;
use codex_core::delegate_tool::DelegateToolRun;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::SessionSource;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use crate::load_default_config_for_test;

#[derive(Default)]
struct TestDelegateAdapter {
    sender: Mutex<Option<mpsc::UnboundedSender<DelegateToolEvent>>>,
    counter: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl DelegateToolAdapter for TestDelegateAdapter {
    async fn subscribe(&self) -> DelegateEventReceiver {
        let (tx, rx) = mpsc::unbounded_channel();
        *self.sender.lock().await = Some(tx);
        rx
    }

    async fn delegate(
        &self,
        request: DelegateToolRequest,
    ) -> Result<DelegateToolRun, DelegateToolError> {
        let idx = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let run_id = format!("test-run-{idx}");
        if let Some(sender) = self.sender.lock().await.as_ref() {
            let _ = sender.send(DelegateToolEvent::Completed {
                run_id: run_id.clone(),
                agent_id: request.agent_id.clone(),
                output: Some(request.prompt.clone()),
                duration: std::time::Duration::from_millis(1),
            });
        }
        Ok(DelegateToolRun {
            run_id,
            agent_id: request.agent_id,
        })
    }
}

type ConfigMutator = dyn FnOnce(&mut Config) + Send;

pub struct TestCodexBuilder {
    config_mutators: Vec<Box<ConfigMutator>>,
}

impl TestCodexBuilder {
    pub fn with_config<T>(mut self, mutator: T) -> Self
    where
        T: FnOnce(&mut Config) + Send + 'static,
    {
        self.config_mutators.push(Box::new(mutator));
        self
    }

    pub async fn build(&mut self, server: &wiremock::MockServer) -> anyhow::Result<TestCodex> {
        // Build config pointing to the mock server and spawn Codex.
        let model_provider = ModelProviderInfo {
            base_url: Some(format!("{}/v1", server.uri())),
            ..built_in_model_providers()["openai"].clone()
        };
        let home = TempDir::new()?;
        let cwd = TempDir::new()?;
        let mut config = load_default_config_for_test(&home);
        config.cwd = cwd.path().to_path_buf();
        config.model_provider = model_provider;
        config.codex_linux_sandbox_exe = Some(PathBuf::from(
            assert_cmd::Command::cargo_bin("codex")?
                .get_program()
                .to_os_string(),
        ));

        let mut mutators = vec![];
        swap(&mut self.config_mutators, &mut mutators);

        for mutator in mutators {
            mutator(&mut config)
        }

        let delegate_adapter: Option<Arc<dyn DelegateToolAdapter>> = if config.include_delegate_tool
        {
            Some(Arc::new(TestDelegateAdapter::default()))
        } else {
            None
        };

        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
        let conversation_manager = ConversationManager::with_delegate(
            auth_manager.clone(),
            SessionSource::Exec,
            delegate_adapter,
        );
        let NewConversation {
            conversation,
            session_configured,
            ..
        } = conversation_manager.new_conversation(config).await?;

        Ok(TestCodex {
            home,
            cwd,
            codex: conversation,
            session_configured,
        })
    }
}

pub struct TestCodex {
    pub home: TempDir,
    pub cwd: TempDir,
    pub codex: Arc<CodexConversation>,
    pub session_configured: SessionConfiguredEvent,
}

pub fn test_codex() -> TestCodexBuilder {
    TestCodexBuilder {
        config_mutators: vec![],
    }
}
