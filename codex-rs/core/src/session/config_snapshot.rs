use anyhow::Context;
use codex_config::config_toml::ConfigToml;
use codex_config::types::MemoriesToml;
use codex_features::AppsMcpPathOverrideConfigToml;
use codex_features::Feature;
use codex_features::FeatureToml;
use codex_features::FeaturesToml;
use codex_protocol::ThreadId;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::SessionConfiguration;

pub(crate) async fn export_config_snapshot_if_configured(
    session_configuration: &SessionConfiguration,
    conversation_id: ThreadId,
) -> anyhow::Result<()> {
    let config = session_configuration.original_config_do_not_use.as_ref();
    let Some(export_dir) = config.config_snapshot_export_dir.as_ref() else {
        return Ok(());
    };

    let snapshot = session_configuration.to_config_snapshot_toml()?;
    let snapshot = toml::to_string_pretty(&snapshot)
        .context("failed to serialize effective session config snapshot")?;
    let path = export_dir.join(format!("{conversation_id}.config.toml"));

    tokio::fs::create_dir_all(export_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create config snapshot export directory {}",
                export_dir.display()
            )
        })?;
    tokio::fs::write(&path, snapshot).await.with_context(|| {
        format!(
            "failed to write effective session config snapshot to {}",
            path.display()
        )
    })?;

    Ok(())
}

impl SessionConfiguration {
    pub(crate) fn to_config_snapshot_toml(&self) -> anyhow::Result<ConfigToml> {
        session_configuration_to_snapshot_config_toml(self)
    }
}

fn session_configuration_to_snapshot_config_toml(
    sc: &SessionConfiguration,
) -> anyhow::Result<ConfigToml> {
    let config = sc.original_config_do_not_use.as_ref();
    let mut snapshot: ConfigToml = config
        .config_layer_stack
        .effective_config()
        .try_into()
        .context("failed to deserialize effective config for snapshot")?;

    snapshot.model = Some(sc.collaboration_mode.model().to_string());
    snapshot.model_reasoning_effort = sc.collaboration_mode.reasoning_effort();
    snapshot.model_reasoning_summary = sc.model_reasoning_summary;
    snapshot.service_tier = sc.service_tier;
    snapshot.instructions = Some(sc.base_instructions.clone());
    snapshot.developer_instructions = sc.developer_instructions.clone();
    snapshot.compact_prompt = sc.compact_prompt.clone();
    snapshot.personality = sc.personality;
    snapshot.approval_policy = Some(sc.approval_policy.value());
    snapshot.approvals_reviewer = Some(sc.approvals_reviewer);
    snapshot.permission_profile = Some(sc.permission_profile.get().clone());
    snapshot.web_search = Some(config.web_search_mode.value());

    snapshot.profile = None;
    snapshot.profiles.clear();
    snapshot.config_snapshot_export_dir = None;
    snapshot.model_instructions_file = None;
    snapshot.experimental_instructions_file = None;
    snapshot.experimental_compact_prompt_file = None;
    snapshot.model_catalog_json = None;
    snapshot.sandbox_mode = None;
    snapshot.sandbox_workspace_write = None;
    snapshot.default_permissions = None;
    snapshot.permissions = None;
    snapshot.experimental_use_unified_exec_tool = None;
    snapshot.experimental_use_freeform_apply_patch = None;

    let features = snapshot.features.get_or_insert_with(FeaturesToml::default);
    features.materialize_resolved_enabled(config.features.get());
    features.multi_agent_v2 = Some(FeatureToml::Config(resolved_config_to_toml(
        &config.multi_agent_v2,
        "features.multi_agent_v2",
    )?));
    if let Some(FeatureToml::Config(multi_agent_v2)) = features.multi_agent_v2.as_mut() {
        multi_agent_v2.enabled = Some(config.features.enabled(Feature::MultiAgentV2));
    }
    features.apps_mcp_path_override = Some(FeatureToml::Config(AppsMcpPathOverrideConfigToml {
        enabled: Some(config.features.enabled(Feature::AppsMcpPathOverride)),
        path: config.apps_mcp_path_override.clone(),
    }));
    snapshot.memories = Some(resolved_config_to_toml::<MemoriesToml, _>(
        &config.memories,
        "memories",
    )?);

    let agents = snapshot.agents.get_or_insert_with(Default::default);
    agents.max_threads = if config.features.enabled(Feature::MultiAgentV2) {
        None
    } else {
        config.agent_max_threads
    };
    agents.max_depth = Some(config.agent_max_depth);
    agents.job_max_runtime_seconds = config.agent_job_max_runtime_seconds;
    agents.interrupt_message = Some(config.agent_interrupt_message_enabled);

    snapshot
        .skills
        .get_or_insert_with(Default::default)
        .include_instructions = Some(config.include_skill_instructions);

    Ok(snapshot)
}

fn resolved_config_to_toml<Toml, Runtime>(
    value: &Runtime,
    label: &'static str,
) -> anyhow::Result<Toml>
where
    Toml: DeserializeOwned,
    Runtime: Serialize,
{
    let value = toml::Value::try_from(value)
        .with_context(|| format!("failed to serialize resolved {label} config"))?;
    value
        .try_into()
        .with_context(|| format!("failed to convert resolved {label} config to snapshot TOML"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_features::MultiAgentV2ConfigToml;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn snapshot_overlays_resolved_session_values_and_materializes_features() {
        let mut sc = crate::session::tests::make_session_configuration_for_tests().await;
        sc.base_instructions = "resolved instructions".to_string();
        sc.developer_instructions = Some("resolved developer instructions".to_string());
        sc.compact_prompt = Some("resolved compact prompt".to_string());

        let snapshot = sc
            .to_config_snapshot_toml()
            .expect("snapshot config should serialize");

        assert_eq!(
            snapshot.model,
            Some(sc.collaboration_mode.model().to_string())
        );
        assert_eq!(
            snapshot.model_reasoning_effort,
            sc.collaboration_mode.reasoning_effort()
        );
        assert_eq!(snapshot.instructions, Some(sc.base_instructions.clone()));
        assert_eq!(snapshot.developer_instructions, sc.developer_instructions);
        assert_eq!(snapshot.compact_prompt, sc.compact_prompt);
        assert_eq!(
            snapshot.permission_profile,
            Some(sc.permission_profile.get().clone())
        );
        assert_eq!(snapshot.profile, None);
        assert!(snapshot.profiles.is_empty());
        assert_eq!(snapshot.config_snapshot_export_dir, None);
        assert_eq!(
            snapshot.memories,
            Some(MemoriesToml {
                disable_on_external_context: Some(
                    sc.original_config_do_not_use
                        .memories
                        .disable_on_external_context
                ),
                generate_memories: Some(sc.original_config_do_not_use.memories.generate_memories),
                use_memories: Some(sc.original_config_do_not_use.memories.use_memories),
                max_raw_memories_for_consolidation: Some(
                    sc.original_config_do_not_use
                        .memories
                        .max_raw_memories_for_consolidation
                ),
                max_unused_days: Some(sc.original_config_do_not_use.memories.max_unused_days),
                max_rollout_age_days: Some(
                    sc.original_config_do_not_use.memories.max_rollout_age_days
                ),
                max_rollouts_per_startup: Some(
                    sc.original_config_do_not_use
                        .memories
                        .max_rollouts_per_startup
                ),
                min_rollout_idle_hours: Some(
                    sc.original_config_do_not_use
                        .memories
                        .min_rollout_idle_hours
                ),
                min_rate_limit_remaining_percent: Some(
                    sc.original_config_do_not_use
                        .memories
                        .min_rate_limit_remaining_percent
                ),
                extract_model: sc.original_config_do_not_use.memories.extract_model.clone(),
                consolidation_model: sc
                    .original_config_do_not_use
                    .memories
                    .consolidation_model
                    .clone(),
            })
        );

        let features = snapshot
            .features
            .as_ref()
            .expect("snapshot should materialize feature states");
        let feature_entries = features.entries();
        for spec in codex_features::FEATURES {
            assert_eq!(
                feature_entries.get(spec.key),
                Some(&sc.original_config_do_not_use.features.enabled(spec.id)),
                "{}",
                spec.key
            );
        }

        let multi_agent_v2 = features
            .multi_agent_v2
            .as_ref()
            .expect("multi_agent_v2 config should be materialized");
        assert!(matches!(
            multi_agent_v2,
            FeatureToml::Config(MultiAgentV2ConfigToml {
                enabled: Some(false),
                max_concurrent_threads_per_session: Some(_),
                min_wait_timeout_ms: Some(_),
                usage_hint_enabled: Some(_),
                hide_spawn_agent_metadata: Some(_),
                ..
            })
        ));
    }
}
