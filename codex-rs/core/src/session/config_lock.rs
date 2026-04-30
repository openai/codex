use anyhow::Context;
use codex_config::config_toml::ConfigToml;
use codex_config::types::MemoriesToml;
use codex_features::AppsMcpPathOverrideConfigToml;
use codex_features::Feature;
use codex_features::FeatureToml;
use codex_features::FeaturesToml;
use codex_protocol::ThreadId;

use crate::config_lock::config_lock_metadata;
use crate::config_lock::toml_round_trip;
use crate::config_lock::validate_config_lock_replay;

use super::SessionConfiguration;

pub(crate) async fn validate_config_lock_if_configured(
    session_configuration: &SessionConfiguration,
) -> anyhow::Result<()> {
    let Some(expected) = session_configuration
        .original_config_do_not_use
        .config_lock
        .as_ref()
    else {
        return Ok(());
    };
    let actual = session_configuration.to_config_lock_toml()?;
    validate_config_lock_replay(expected, &actual)
        .context("config lock replay validation failed")?;
    Ok(())
}

pub(crate) async fn export_config_lock_if_configured(
    session_configuration: &SessionConfiguration,
    conversation_id: ThreadId,
) -> anyhow::Result<()> {
    let config = session_configuration.original_config_do_not_use.as_ref();
    let Some(export_dir) = config.config_snapshot_export_dir.as_ref() else {
        return Ok(());
    };

    let lock = session_configuration.to_config_lock_toml()?;
    let lock = toml::to_string_pretty(&lock).context("failed to serialize config lock")?;
    let path = export_dir.join(format!("{conversation_id}.config.lock.toml"));

    tokio::fs::create_dir_all(export_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create config lock export directory {}",
                export_dir.display()
            )
        })?;
    tokio::fs::write(&path, lock)
        .await
        .with_context(|| format!("failed to write config lock to {}", path.display()))?;

    Ok(())
}

impl SessionConfiguration {
    pub(crate) fn to_config_lock_toml(&self) -> anyhow::Result<ConfigToml> {
        let mut lock_config = session_configuration_to_lock_config_toml(self)?;
        lock_config.config_lock = Some(config_lock_metadata(&self.thread_config_snapshot()));
        Ok(lock_config)
    }
}

fn session_configuration_to_lock_config_toml(
    sc: &SessionConfiguration,
) -> anyhow::Result<ConfigToml> {
    let config = sc.original_config_do_not_use.as_ref();
    let mut lock_config: ConfigToml = config
        .config_layer_stack
        .effective_config()
        .try_into()
        .context("failed to deserialize effective config for config lock")?;

    lock_config.model = Some(sc.collaboration_mode.model().to_string());
    lock_config.model_reasoning_effort = sc.collaboration_mode.reasoning_effort();
    lock_config.model_reasoning_summary = sc.model_reasoning_summary;
    lock_config.service_tier = sc.service_tier;
    lock_config.instructions = Some(sc.base_instructions.clone());
    lock_config.developer_instructions = sc.developer_instructions.clone();
    lock_config.compact_prompt = sc.compact_prompt.clone();
    lock_config.personality = sc.personality;
    lock_config.approval_policy = Some(sc.approval_policy.value());
    lock_config.approvals_reviewer = Some(sc.approvals_reviewer);
    lock_config.permission_profile = Some(sc.permission_profile.get().clone());
    lock_config.web_search = Some(config.web_search_mode.value());
    lock_config.model_provider = Some(config.model_provider_id.clone());
    lock_config.model_reasoning_effort = config.model_reasoning_effort;
    lock_config.plan_mode_reasoning_effort = config.plan_mode_reasoning_effort;
    lock_config.model_verbosity = config.model_verbosity;
    lock_config.include_permissions_instructions = Some(config.include_permissions_instructions);
    lock_config.include_apps_instructions = Some(config.include_apps_instructions);
    lock_config.include_environment_context = Some(config.include_environment_context);
    lock_config.background_terminal_max_timeout = Some(config.background_terminal_max_timeout);

    lock_config.profile = None;
    lock_config.profiles.clear();
    lock_config.config_lock = None;
    lock_config.config_snapshot_export_dir = None;
    lock_config.config_lock_file = None;
    lock_config.model_instructions_file = None;
    lock_config.experimental_instructions_file = None;
    lock_config.experimental_compact_prompt_file = None;
    lock_config.model_catalog_json = None;
    lock_config.sandbox_mode = None;
    lock_config.sandbox_workspace_write = None;
    lock_config.default_permissions = None;
    lock_config.permissions = None;
    lock_config.experimental_use_unified_exec_tool = None;
    lock_config.experimental_use_freeform_apply_patch = None;

    let features = lock_config
        .features
        .get_or_insert_with(FeaturesToml::default);
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
    lock_config.memories = Some(resolved_config_to_toml::<MemoriesToml>(
        &config.memories,
        "memories",
    )?);

    let agents = lock_config.agents.get_or_insert_with(Default::default);
    agents.max_threads = if config.features.enabled(Feature::MultiAgentV2) {
        None
    } else {
        config.agent_max_threads
    };
    agents.max_depth = Some(config.agent_max_depth);
    agents.job_max_runtime_seconds = config.agent_job_max_runtime_seconds;
    agents.interrupt_message = Some(config.agent_interrupt_message_enabled);

    lock_config
        .skills
        .get_or_insert_with(Default::default)
        .include_instructions = Some(config.include_skill_instructions);

    Ok(lock_config)
}

fn resolved_config_to_toml<Toml>(
    value: &impl serde::Serialize,
    label: &'static str,
) -> anyhow::Result<Toml>
where
    Toml: serde::de::DeserializeOwned + serde::Serialize,
{
    toml_round_trip(value, label).map_err(anyhow::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_features::MultiAgentV2ConfigToml;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn lock_contains_prompts_and_materializes_features() {
        let mut sc = crate::session::tests::make_session_configuration_for_tests().await;
        sc.base_instructions = "resolved instructions".to_string();
        sc.developer_instructions = Some("resolved developer instructions".to_string());
        sc.compact_prompt = Some("resolved compact prompt".to_string());

        let lock = sc.to_config_lock_toml().expect("lock should serialize");

        assert_eq!(lock.instructions, Some(sc.base_instructions.clone()));
        assert_eq!(lock.developer_instructions, sc.developer_instructions);
        assert_eq!(lock.compact_prompt, sc.compact_prompt);
        assert_eq!(lock.model, Some(sc.collaboration_mode.model().to_string()));
        assert_eq!(
            lock.model_reasoning_effort,
            sc.collaboration_mode.reasoning_effort()
        );
        assert_eq!(
            lock.permission_profile,
            Some(sc.permission_profile.get().clone())
        );
        assert_eq!(lock.profile, None);
        assert!(lock.profiles.is_empty());
        assert_eq!(lock.config_snapshot_export_dir, None);
        assert_eq!(lock.config_lock_file, None);
        assert_eq!(
            lock.memories,
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

        let features = lock
            .features
            .as_ref()
            .expect("lock should materialize feature states");
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

        let metadata = lock
            .config_lock
            .as_ref()
            .expect("lock should include metadata");
        assert_eq!(metadata.version, crate::config_lock::CONFIG_LOCK_VERSION);
        assert_eq!(metadata.cwd, sc.cwd);
    }

    #[tokio::test]
    async fn lock_validation_rejects_prompt_drift() {
        let sc = crate::session::tests::make_session_configuration_for_tests().await;
        let actual = sc.to_config_lock_toml().expect("lock should serialize");
        let mut expected = actual.clone();
        expected
            .instructions
            .as_mut()
            .expect("lock should include instructions")
            .push_str("\nchanged");

        let error =
            validate_config_lock_replay(&expected, &actual).expect_err("prompt drift should fail");
        let message = error.to_string();
        assert!(
            message.contains("replayed effective config does not match config lock"),
            "{message}"
        );
        assert!(message.contains("config.instructions"), "{message}");
    }

    #[tokio::test]
    async fn lock_validation_reports_config_diff() {
        let sc = crate::session::tests::make_session_configuration_for_tests().await;
        let expected = sc.to_config_lock_toml().expect("lock should serialize");
        let mut actual = expected.clone();
        actual.model = Some("different-model".to_string());

        let error =
            validate_config_lock_replay(&expected, &actual).expect_err("config drift should fail");
        let message = error.to_string();
        assert!(
            message.contains("replayed effective config does not match config lock"),
            "{message}"
        );
        assert!(message.contains("config.model"), "{message}");
    }
}
