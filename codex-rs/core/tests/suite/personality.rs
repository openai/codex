use codex_core::config::types::Personality;
use codex_core::models_manager::manager::ModelsManager;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const BASE_INSTRUCTIONS_TEMPLATE: &str = include_str!(
    "../../templates/model_instructions/gpt-5.2-codex_instructions_template.md"
);
const FRIENDLY_PERSONALITY: &str = include_str!("../../templates/personalities/friendly.md");

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_personality_updates_base_instructions() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_personality = Some(Personality::Friendly);

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.2-codex", &config);
    let expected =
        BASE_INSTRUCTIONS_TEMPLATE.replace("{{ personality_message }}", FRIENDLY_PERSONALITY);

    assert_eq!(model_info.base_instructions, expected);
}
