use std::mem::swap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::features::Feature;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::SessionConfiguredEvent;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::MockServer;

use crate::load_default_config_for_test;
use crate::responses::start_mock_server;
use crate::wait_for_event;

type ConfigMutator = dyn FnOnce(&mut Config) + Send;

/// A collection of different ways the model can output an apply_patch call
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApplyPatchModelOutput {
    Freeform,
    Function,
    Shell,
    ShellViaHeredoc,
    ShellCommandViaHeredoc,
}

/// A collection of different ways the model can output an apply_patch call
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShellModelOutput {
    Shell,
    ShellCommand,
    LocalShell,
    // UnifiedExec has its own set of tests
}

pub struct TestCodexBuilder {
    config_mutators: Vec<Box<ConfigMutator>>,
    auth: CodexAuth,
}

impl TestCodexBuilder {
    pub fn with_config<T>(mut self, mutator: T) -> Self
    where
        T: FnOnce(&mut Config) + Send + 'static,
    {
        self.config_mutators.push(Box::new(mutator));
        self
    }

    pub fn with_auth(mut self, auth: CodexAuth) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_model(self, model: &str) -> Self {
        let new_model = model.to_string();
        self.with_config(move |config| {
            config.model = new_model.clone();
            config.model_family = find_family_for_model(&new_model).expect("model family");
        })
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

        let auth = self.auth.clone();
        let conversation_manager = ConversationManager::with_auth(auth.clone());

        let new_conversation = match resume_from {
            Some(path) => {
                let auth_manager = codex_core::AuthManager::from_auth_for_testing(auth);
                conversation_manager
                    .resume_conversation_from_rollout(config.clone(), path, auth_manager)
                    .await?
            }
            None => {
                conversation_manager
                    .new_conversation(config.clone())
                    .await?
            }
        };

        Ok(TestCodex {
            home,
            cwd,
            config,
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
        if let Ok(cmd) = assert_cmd::Command::cargo_bin("codex") {
            config.codex_linux_sandbox_exe = Some(PathBuf::from(cmd.get_program().to_os_string()));
        }

        let mut mutators = vec![];
        swap(&mut self.config_mutators, &mut mutators);
        for mutator in mutators {
            mutator(&mut config);
        }

        if config.include_apply_patch_tool {
            config.features.enable(Feature::ApplyPatchFreeform);
        } else {
            config.features.disable(Feature::ApplyPatchFreeform);
        }

        Ok((config, cwd))
    }
}

pub struct TestCodex {
    pub home: Arc<TempDir>,
    pub cwd: Arc<TempDir>,
    pub codex: Arc<CodexConversation>,
    pub session_configured: SessionConfiguredEvent,
    pub config: Config,
}

impl TestCodex {
    pub fn cwd_path(&self) -> &Path {
        self.cwd.path()
    }

    pub fn workspace_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.cwd_path().join(rel)
    }

    pub async fn submit_turn(&self, prompt: &str) -> Result<()> {
        self.submit_turn_with_policies(
            prompt,
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
        )
        .await
    }

    pub async fn submit_turn_with_policy(
        &self,
        prompt: &str,
        sandbox_policy: SandboxPolicy,
    ) -> Result<()> {
        self.submit_turn_with_policies(prompt, AskForApproval::Never, sandbox_policy)
            .await
    }

    pub async fn submit_turn_with_policies(
        &self,
        prompt: &str,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
    ) -> Result<()> {
        let session_model = self.session_configured.model.clone();
        self.codex
            .submit(Op::UserTurn {
                items: vec![UserInput::Text {
                    text: prompt.into(),
                }],
                final_output_json_schema: None,
                cwd: self.cwd.path().to_path_buf(),
                approval_policy,
                sandbox_policy,
                model: session_model,
                effort: None,
                summary: ReasoningSummary::Auto,
            })
            .await?;

        wait_for_event(&self.codex, |event| {
            matches!(event, EventMsg::TaskComplete(_))
        })
        .await;
        Ok(())
    }
}

pub struct TestCodexHarness {
    server: MockServer,
    test: TestCodex,
}

impl TestCodexHarness {
    pub async fn new() -> Result<Self> {
        Self::with_builder(test_codex()).await
    }

    pub async fn with_config(mutator: impl FnOnce(&mut Config) + Send + 'static) -> Result<Self> {
        Self::with_builder(test_codex().with_config(mutator)).await
    }

    pub async fn with_builder(mut builder: TestCodexBuilder) -> Result<Self> {
        let server = start_mock_server().await;
        let test = builder.build(&server).await?;
        Ok(Self { server, test })
    }

    pub fn server(&self) -> &MockServer {
        &self.server
    }

    pub fn test(&self) -> &TestCodex {
        &self.test
    }

    pub fn cwd(&self) -> &Path {
        self.test.cwd_path()
    }

    pub fn path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.test.workspace_path(rel)
    }

    pub async fn submit(&self, prompt: &str) -> Result<()> {
        self.test.submit_turn(prompt).await
    }

    pub async fn submit_with_policy(
        &self,
        prompt: &str,
        sandbox_policy: SandboxPolicy,
    ) -> Result<()> {
        self.test
            .submit_turn_with_policy(prompt, sandbox_policy)
            .await
    }

    pub async fn request_bodies(&self) -> Vec<Value> {
        self.server
            .received_requests()
            .await
            .expect("requests")
            .into_iter()
            .map(|req| serde_json::from_slice(&req.body).expect("request body json"))
            .collect()
    }

    pub async fn function_call_output_value(&self, call_id: &str) -> Value {
        let bodies = self.request_bodies().await;
        function_call_output(&bodies, call_id).clone()
    }

    pub async fn function_call_stdout(&self, call_id: &str) -> String {
        self.function_call_output_value(call_id)
            .await
            .get("output")
            .and_then(Value::as_str)
            .expect("output string")
            .to_string()
    }

    pub async fn custom_tool_call_output(&self, call_id: &str) -> String {
        let bodies = self.request_bodies().await;
        custom_tool_call_output(&bodies, call_id)
            .get("output")
            .and_then(Value::as_str)
            .expect("output string")
            .to_string()
    }

    pub async fn apply_patch_output(
        &self,
        call_id: &str,
        output_type: ApplyPatchModelOutput,
    ) -> String {
        match output_type {
            ApplyPatchModelOutput::Freeform => self.custom_tool_call_output(call_id).await,
            ApplyPatchModelOutput::Function
            | ApplyPatchModelOutput::Shell
            | ApplyPatchModelOutput::ShellViaHeredoc
            | ApplyPatchModelOutput::ShellCommandViaHeredoc => {
                self.function_call_stdout(call_id).await
            }
        }
    }
}

fn custom_tool_call_output<'a>(bodies: &'a [Value], call_id: &str) -> &'a Value {
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("custom_tool_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                {
                    return item;
                }
            }
        }
    }
    panic!("custom_tool_call_output {call_id} not found");
}

fn function_call_output<'a>(bodies: &'a [Value], call_id: &str) -> &'a Value {
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                {
                    return item;
                }
            }
        }
    }
    panic!("function_call_output {call_id} not found");
}

pub fn test_codex() -> TestCodexBuilder {
    TestCodexBuilder {
        config_mutators: vec![],
        auth: CodexAuth::from_api_key("dummy"),
    }
}
