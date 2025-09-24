use std::mem::swap;
use std::sync::Arc;

use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::protocol::SessionConfiguredEvent;
use tempfile::TempDir;

use crate::load_default_config_for_test;

type ConfigMutator = dyn FnOnce(&mut Config);

pub struct TestCodexBuilder {
    config_mutators: Vec<Box<ConfigMutator>>,
}

impl TestCodexBuilder {
    pub fn with_config<T>(mut self, mutator: T) -> Self
    where
        T: FnOnce(&mut Config) + 'static,
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
        let mut mutators = vec![];
        swap(&mut self.config_mutators, &mut mutators);

        for mutator in mutators {
            mutator(&mut config)
        }
        let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
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
