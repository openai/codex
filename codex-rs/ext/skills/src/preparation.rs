//! Resolves step policy once and prepares immutable catalogs before tool construction.
//! Cache operations and live read-time authorization remain on the thread state.

use crate::HostSkillsSnapshot;
use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::catalog::apply_plugin_denials;
use crate::provider::SkillListQuery;
use crate::provider::attribute_executor_plugins;
use crate::sources::SkillProviders;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;
use crate::step_state::CatalogStatus;
use crate::step_state::SkillsStepState;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_extension_api::StepPreparationInput;

pub(crate) async fn prepare_catalogs(
    thread_state: &SkillsThreadState,
    providers: &SkillProviders,
    input: &StepPreparationInput<'_>,
) -> SkillsStepState {
    let config = thread_state.config();
    let cloud_enabled = config.cloud_skill_enabled && thread_state.cloud_skills_available();
    let discovery_v2 = input.executor_capability_discovery_v2.is_some();
    let mut cloud = cloud_enabled
        .then(|| thread_state.available_cloud_catalog())
        .flatten();
    let (mut executor, host_catalog) = futures::join!(
        discover_executor_catalog(thread_state, providers, input, &config, cloud_enabled),
        discover_host_catalog(providers, input, &config),
    );
    if discovery_v2 {
        apply_plugin_denials(&mut executor, input.selected_plugins);
        if let Some(cloud) = &mut cloud {
            apply_plugin_denials(cloud, input.selected_plugins);
        }
    } else {
        attribute_executor_plugins(&mut executor, input.selected_plugins);
    }
    let cloud_status = if !providers.has_cloud_provider() {
        CatalogStatus::Unavailable
    } else if cloud_enabled {
        CatalogStatus::Enabled
    } else {
        CatalogStatus::Disabled
    };
    SkillsStepState {
        discovery_v2,
        tools_enabled: cloud_enabled
            || input
                .step_store
                .get::<Vec<ResolvedSelectedCapabilityRoot>>()
                .map_or_else(
                    || !input.ready_selected_capability_roots.is_empty(),
                    |roots| !roots.is_empty(),
                )
            || discovery_v2,
        catalog: thread_state.prepare_step_catalog(cloud, executor, discovery_v2),
        host_catalog,
        cloud_status,
        config,
    }
}

async fn discover_executor_catalog(
    thread_state: &SkillsThreadState,
    providers: &SkillProviders,
    input: &StepPreparationInput<'_>,
    config: &SkillsExtensionConfig,
    cloud_enabled: bool,
) -> SkillCatalog {
    // Core supplies captured roots even when none are ready. Descriptor-only callers
    // retain their unbound discovery path.
    let captured_roots = input
        .step_store
        .get::<Vec<ResolvedSelectedCapabilityRoot>>();
    let executor_roots = captured_roots
        .as_ref()
        .map(|roots| {
            roots
                .iter()
                .map(|root| root.selected_root().clone())
                .collect()
        })
        .unwrap_or_else(|| input.ready_selected_capability_roots.to_vec());
    let query = SkillListQuery {
        turn_id: input.turn_id.to_string(),
        executor_roots,
        resolved_executor_roots: captured_roots
            .map(|roots| roots.as_ref().clone())
            .unwrap_or_default(),
        host_snapshot: None,
        include_host_skills: false,
        include_bundled_skills: config.bundled_skills_enabled,
        include_cloud_skills: cloud_enabled,
        mcp_resources: input
            .session_store
            .get::<SkillsSessionState>()
            .and_then(|state| state.mcp_resources.clone()),
        executor_capability_discovery: input.executor_capability_discovery.cloned(),
        executor_capability_discovery_v2: input.executor_capability_discovery_v2.map(Vec::from),
    };
    thread_state
        .refresh_executor_catalog(providers, query)
        .await
}

async fn discover_host_catalog(
    providers: &SkillProviders,
    input: &StepPreparationInput<'_>,
    config: &SkillsExtensionConfig,
) -> Option<SkillCatalog> {
    let host_snapshot = input
        .turn_store
        .get::<HostSkillsSnapshot>()
        .filter(|_| providers.has_host_provider())?;
    if !config.include_instructions && !config.shadow_selection_enabled {
        return Some(SkillCatalog::default());
    }
    Some(
        providers
            .list_host_for_turn(SkillListQuery {
                turn_id: input.turn_id.to_string(),
                executor_roots: Vec::new(),
                resolved_executor_roots: Vec::new(),
                host_snapshot: Some(host_snapshot),
                include_host_skills: true,
                include_bundled_skills: false,
                include_cloud_skills: false,
                mcp_resources: None,
                executor_capability_discovery: None,
                executor_capability_discovery_v2: None,
            })
            .await,
    )
}
