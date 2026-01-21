use codex_core::config::types::Personality;
use codex_core::models_manager::manager::ModelsManager;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const GPT_5_2_CODEX_INSTRUCTIONS: &str = include_str!("../../gpt-5.2-codex_prompt.md");

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_personality_does_not_mutate_base_instructions_without_template() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_personality = Some(Personality::Friendly);

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.2-codex", &config);
    assert_eq!(model_info.base_instructions, GPT_5_2_CODEX_INSTRUCTIONS);
    assert_eq!(
        model_info.get_model_instructions(config.model_personality),
        GPT_5_2_CODEX_INSTRUCTIONS
    );
}
