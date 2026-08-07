mod aliases;
pub mod catalog;
mod catalog_prompt;
mod config;
mod dynamic_skill_selector;
mod extension;
mod fragments;
mod host_aliases;
mod host_outcome;
mod host_prompt;
mod host_roots;
mod host_service;
mod host_snapshot;
mod loader;
pub mod provider;
mod render;
mod render_observability;
mod selection;
mod shadow_selection_experiment;
mod sources;
mod state;
mod tools;
mod warnings;
mod world_state;
mod world_state_catalogs;

pub use config::SkillsExtensionConfig;
pub use extension::install;
pub use extension::install_with_providers;
pub use extension::install_with_providers_and_metrics;
pub use host_outcome::SkillLoadOutcome;
pub use host_prompt::HostSkillPrompts;
pub use host_prompt::InjectedHostSkillPrompts;
pub use host_service::HostSkillsLoadInput;
pub use host_service::HostSkillsService;
pub use host_service::bundled_skills_enabled_from_stack;
pub use host_snapshot::HostSkillsSnapshot;
pub use loader::HostSkillRoot;
pub use provider::ExecutorSkillProvider;
pub use provider::HostSkillProvider;
pub use provider::OrchestratorSkillProvider;
pub use provider::SkillProvider;
pub use sources::SkillProviderSource;
pub use sources::SkillProviders;

/// Recognizes persisted explicit skill prompts without exposing their fragment implementation.
pub fn is_skill_prompt_fragment(text: &str) -> bool {
    <fragments::SkillInstructions as codex_extension_api::ContextualUserFragment>::matches_text(
        text,
    )
}
