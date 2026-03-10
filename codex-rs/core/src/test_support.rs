//! Test-only helpers exposed for cross-crate integration tests.
//!
//! Production code should not depend on this module.
//! We prefer this to using a crate feature to avoid building multiple
//! permutations of the crate.

use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::AuthManager;
use crate::CodexAuth;
use crate::ModelProviderInfo;
use crate::ThreadManager;
use crate::built_in_model_providers;
use crate::codex::make_session_and_context_for_tests_with_agent_control;
use crate::config::AgentRoleConfig;
use crate::config::Config;
use crate::function_tool::FunctionCallError;
use crate::models_manager::collaboration_mode_presets;
use crate::models_manager::manager::ModelsManager;
use crate::thread_manager;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::MultiAgentHandler;
use crate::tools::registry::ToolHandler;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::unified_exec;

static TEST_MODEL_PRESETS: Lazy<Vec<ModelPreset>> = Lazy::new(|| {
    let file_contents = include_str!("../models.json");
    let mut response: ModelsResponse = serde_json::from_str(file_contents)
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    response.models.sort_by(|a, b| a.priority.cmp(&b.priority));
    let mut presets: Vec<ModelPreset> = response.models.into_iter().map(Into::into).collect();
    ModelPreset::mark_default_by_picker_visibility(&mut presets);
    presets
});

pub fn set_thread_manager_test_mode(enabled: bool) {
    thread_manager::set_thread_manager_test_mode_for_tests(enabled);
}

pub fn set_deterministic_process_ids(enabled: bool) {
    unified_exec::set_deterministic_process_ids_for_tests(enabled);
}

pub fn auth_manager_from_auth(auth: CodexAuth) -> Arc<AuthManager> {
    AuthManager::from_auth_for_testing(auth)
}

pub fn auth_manager_from_auth_with_home(auth: CodexAuth, codex_home: PathBuf) -> Arc<AuthManager> {
    AuthManager::from_auth_for_testing_with_home(auth, codex_home)
}

pub fn thread_manager_with_models_provider(
    auth: CodexAuth,
    provider: ModelProviderInfo,
) -> ThreadManager {
    ThreadManager::with_models_provider_for_tests(auth, provider)
}

pub fn thread_manager_with_models_provider_and_home(
    auth: CodexAuth,
    provider: ModelProviderInfo,
    codex_home: PathBuf,
) -> ThreadManager {
    ThreadManager::with_models_provider_and_home_for_tests(auth, provider, codex_home)
}

pub fn models_manager_with_provider(
    codex_home: PathBuf,
    auth_manager: Arc<AuthManager>,
    provider: ModelProviderInfo,
) -> ModelsManager {
    ModelsManager::with_provider_for_tests(codex_home, auth_manager, provider)
}

pub fn get_model_offline(model: Option<&str>) -> String {
    ModelsManager::get_model_offline_for_tests(model)
}

pub fn construct_model_info_offline(model: &str, config: &Config) -> ModelInfo {
    ModelsManager::construct_model_info_offline_for_tests(model, config)
}

pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &TEST_MODEL_PRESETS
}

pub fn builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    collaboration_mode_presets::builtin_collaboration_mode_presets(
        collaboration_mode_presets::CollaborationModesConfig::default(),
    )
}

#[derive(Clone, Debug, Default)]
pub struct SpawnAgentTestSetup {
    pub requested_model: Option<String>,
    pub requested_reasoning_effort: Option<ReasoningEffort>,
    pub inherited_model: Option<String>,
    pub inherited_reasoning_effort: Option<ReasoningEffort>,
    pub role_name: Option<String>,
    pub role_model: Option<String>,
    pub role_reasoning_effort: Option<ReasoningEffort>,
}

pub async fn spawn_agent_snapshot_for_tests(
    setup: SpawnAgentTestSetup,
) -> Result<crate::ThreadConfigSnapshot, String> {
    #[derive(Deserialize)]
    struct SpawnAgentResult {
        agent_id: String,
    }

    let manager = ThreadManager::with_models_provider_for_tests(
        CodexAuth::from_api_key("dummy"),
        built_in_model_providers()["openai"].clone(),
    );
    let (session, mut turn) =
        make_session_and_context_for_tests_with_agent_control(manager.agent_control()).await;

    if let Some(role_name) = &setup.role_name {
        let Some(role_model) = setup.role_model.as_ref() else {
            panic!("role_model should be set when role_name is set");
        };
        let Some(role_reasoning_effort) = setup.role_reasoning_effort else {
            panic!("role_reasoning_effort should be set when role_name is set");
        };
        tokio::fs::create_dir_all(&turn.config.codex_home)
            .await
            .unwrap_or_else(|err| panic!("codex home should be created: {err}"));
        let role_path = turn.config.codex_home.join(format!("{role_name}.toml"));
        tokio::fs::write(
            &role_path,
            format!(
                "model = \"{role_model}\"\nmodel_reasoning_effort = \"{role_reasoning_effort}\"\n"
            ),
        )
        .await
        .unwrap_or_else(|err| panic!("role config should be written: {err}"));

        let mut config = (*turn.config).clone();
        config.agent_roles.insert(
            role_name.clone(),
            AgentRoleConfig {
                description: None,
                config_file: Some(role_path),
                nickname_candidates: None,
            },
        );
        turn.config = Arc::new(config);
    }

    if let Some(inherited_model) = setup.inherited_model.as_ref() {
        let mut config = (*turn.config).clone();
        config.model = Some(inherited_model.clone());
        if let Some(inherited_reasoning_effort) = setup.inherited_reasoning_effort {
            config.model_reasoning_effort = Some(inherited_reasoning_effort);
            turn.reasoning_effort = Some(inherited_reasoning_effort);
        }
        turn.model_info = session
            .services
            .models_manager
            .get_model_info(inherited_model, &config)
            .await;
        turn.config = Arc::new(config);
    }

    let mut arguments = serde_json::Map::from_iter([(
        "message".to_string(),
        Value::String("inspect this repo".to_string()),
    )]);
    if let Some(role_name) = setup.role_name {
        arguments.insert("agent_type".to_string(), Value::String(role_name));
    }
    if let Some(requested_model) = setup.requested_model {
        arguments.insert("model".to_string(), Value::String(requested_model));
    }
    if let Some(requested_reasoning_effort) = setup.requested_reasoning_effort {
        arguments.insert(
            "reasoning_effort".to_string(),
            serde_json::to_value(requested_reasoning_effort)
                .unwrap_or_else(|err| panic!("reasoning effort should serialize: {err}")),
        );
    }

    let output = MultiAgentHandler
        .handle(ToolInvocation {
            session,
            turn: Arc::new(turn),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
            call_id: "call-1".to_string(),
            tool_name: "spawn_agent".to_string(),
            payload: ToolPayload::Function {
                arguments: Value::Object(arguments).to_string(),
            },
        })
        .await
        .map_err(|err| match err {
            FunctionCallError::RespondToModel(message) => message,
            _ => err.to_string(),
        })?;

    let FunctionToolOutput { body, .. } = output;
    let output_text = codex_protocol::models::function_call_output_content_items_to_text(&body)
        .unwrap_or_default();
    let result: SpawnAgentResult = serde_json::from_str(&output_text)
        .unwrap_or_else(|err| panic!("spawn_agent result should be json: {err}"));
    let agent_id = ThreadId::from_string(&result.agent_id)
        .unwrap_or_else(|err| panic!("agent_id should be valid: {err}"));

    let thread = manager
        .get_thread(agent_id)
        .await
        .unwrap_or_else(|err| panic!("spawned agent thread should exist: {err}"));
    Ok(thread.config_snapshot().await)
}
