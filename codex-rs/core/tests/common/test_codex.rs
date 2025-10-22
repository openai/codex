use std::mem::swap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::protocol::SessionConfiguredEvent;
use tempfile::TempDir;

use crate::load_default_config_for_test;

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
        let home = Arc::new(TempDir::new()?);
        self.build_with_home(server, home, None).await
    }

    pub async fn resume(
        &mut self,
        server: &wiremock::MockServer,
        home: Arc<TempDir>,
        rollout_path: PathBuf,
    ) -> anyhow::Result<TestCodex> {
        self.build_with_home(server, home, Some(rollout_path)).await
    }

    async fn build_with_home(
        &mut self,
        server: &wiremock::MockServer,
        home: Arc<TempDir>,
        resume_from: Option<PathBuf>,
    ) -> anyhow::Result<TestCodex> {
        let (config, cwd) = self.prepare_config(server, &home).await?;
        let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));

        let new_conversation = match resume_from {
            Some(path) => {
                let auth_manager = codex_core::AuthManager::from_auth_for_testing(
                    CodexAuth::from_api_key("dummy"),
                );
                conversation_manager
                    .resume_conversation_from_rollout(config, path, auth_manager)
                    .await?
            }
            None => conversation_manager.new_conversation(config).await?,
        };

        Ok(TestCodex {
            home,
            cwd,
            codex: new_conversation.conversation,
            session_configured: new_conversation.session_configured,
        })
    }

    async fn prepare_config(
        &mut self,
        server: &wiremock::MockServer,
        home: &TempDir,
    ) -> anyhow::Result<(Config, Arc<TempDir>)> {
        let model_provider = ModelProviderInfo {
            base_url: Some(format!("{}/v1", server.uri())),
            ..built_in_model_providers()["openai"].clone()
        };
        let cwd = Arc::new(TempDir::new()?);
        let mut config = load_default_config_for_test(home);
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
            mutator(&mut config);
        }

        Ok((config, cwd))
    }
}

pub struct TestCodex {
    pub home: Arc<TempDir>,
    pub cwd: Arc<TempDir>,
    pub codex: Arc<CodexConversation>,
    pub session_configured: SessionConfiguredEvent,
}

pub fn test_codex() -> TestCodexBuilder {
    TestCodexBuilder {
        config_mutators: vec![],
    }
}
