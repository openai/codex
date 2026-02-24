//! Test-only helpers exposed for cross-crate integration tests.
//!
//! Production code should not depend on this module.
//! We prefer this to using a crate feature to avoid building multiple
//! permutations of the crate.

use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use once_cell::sync::Lazy;

use crate::AuthManager;
use crate::CodexAuth;
use crate::ModelProviderInfo;
use crate::ThreadManager;
use crate::config::Config;
use crate::models_manager::collaboration_mode_presets;
use crate::models_manager::manager::ModelsManager;
use crate::thread_manager;
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
    collaboration_mode_presets::builtin_collaboration_mode_presets()
}
