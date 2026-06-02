use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;

use crate::provider::SkillListQuery;
use crate::providers::SkillProviders;
use crate::state::SkillsExtensionConfig;
use crate::state::SkillsTurnState;

#[derive(Clone, Debug)]
struct SkillsExtension {
    providers: SkillProviders,
}

impl Default for SkillsExtension {
    fn default() -> Self {
        Self {
            providers: SkillProviders::default(),
        }
    }
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<Config> for SkillsExtension {
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        // TODO(skills-extension): this is only the thread-level config snapshot.
        // Skills are loaded per turn today because cwd, plugin roots, config
        // layers, and the primary environment filesystem can change between
        // turns. The real migration needs a turn-preparation hook before model
        // input construction, not just thread startup.
        input
            .thread_store
            .insert(SkillsExtensionConfig::from_config(input.config));
    }
}

impl ConfigContributor<Config> for SkillsExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        // TODO(skills-extension): update any cached/listing state that depends
        // on skill config overrides, bundled skills, or include_instructions.
        thread_store.insert(SkillsExtensionConfig::from_config(new_config));
    }
}

impl ContextContributor for SkillsExtension {
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> Pin<Box<dyn Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            let Some(config) = thread_store.get::<SkillsExtensionConfig>() else {
                return Vec::new();
            };
            if !config.include_instructions {
                return Vec::new();
            }

            // TODO(skills-extension): render the available-skills developer
            // block from the merged per-turn SkillCatalog. This should
            // preserve the existing bounded metadata budget, root aliasing,
            // warning behavior, and telemetry side effects.
            //
            // TODO(skills-extension): avoid using raw PromptFragment strings
            // for final skills context if the extension API grows typed
            // contextual fragments. Existing skill blocks are typed so resume
            // and history filtering can recognize them reliably.
            //
            // TODO(skills-extension): ContextContributor currently cannot see
            // the turn_store, so it cannot read the per-turn catalog seeded by
            // the turn provider path below. This is the main extension-api gap
            // to close before skills can move out of codex-core.
            Vec::new()
        })
    }
}

#[async_trait::async_trait]
impl TurnLifecycleContributor for SkillsExtension {
    async fn on_turn_start(&self, input: TurnStartInput<'_>) {
        // TODO(skills-extension): replace this lifecycle callback with a real
        // turn-input contributor in codex-extension-api. This placeholder only
        // demonstrates where provider aggregation belongs; it cannot resolve
        // real skills because this hook does not receive cwd, executor
        // selections, effective plugins/materialized plugin skill roots,
        // connector slug counts, user input, cancellation, analytics, or a
        // response-item output channel.
        let query = SkillListQuery::placeholder_for_turn(input.turn_id);
        let catalog = self
            .providers
            .list_for_turn(query)
            .await
            .unwrap_or_default();

        input.turn_store.insert(SkillsTurnState {
            catalog,
            entrypoints_injected: false,
        });

        // TODO(skills-extension): after catalog resolution, collect explicit
        // skill mentions from structured UserInput and text mentions.
        //
        // TODO(skills-extension): inject selected entrypoints as typed
        // contextual user fragments, preserving <skill>...</skill> history
        // recognition and bounded body size limits.
        //
        // TODO(skills-extension): move explicit $skill mention resolution,
        // SKILL.md reads, skill body injection, and MCP dependency prompting
        // out of codex-core's turn assembly once that hook exists.
    }
}

/// Installs the skills extension contributor sketch.
///
/// TODO(skills-extension): pass host capabilities here rather than letting the
/// extension depend on Session. The final extension needs capability objects for
/// loading skill roots, emitting warnings, tracking analytics, prompting for MCP
/// dependency install, refreshing MCP servers, and serving app-server catalog
/// requests.
///
/// TODO(skills-extension): plugin handling should stay outside the runtime
/// skills model. Plugins are bundle/install units; once installed or refreshed,
/// their skill descriptors/roots should be handed to this extension just like
/// any other host-owned skill source.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>) {
    let extension = Arc::new(SkillsExtension::default());
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.turn_lifecycle_contributor(extension);
}
