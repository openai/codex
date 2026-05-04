use anyhow::Context;
use anyhow::bail;
use codex_config::config_toml::ConfigToml;
use codex_config::types::MemoriesToml;
use codex_features::AppsMcpPathOverrideConfigToml;
use codex_features::Feature;
use codex_features::FeatureToml;
use codex_features::FeaturesToml;
use codex_features::MultiAgentV2ConfigToml;
use codex_utils_template::Template;
use toml::Value as TomlValue;

use super::Config;

const INTERPOLATED_CONFIG_STRING_FIELDS: &[&str] = &[
    "features.multi_agent_v2.usage_hint_text",
    "features.multi_agent_v2.root_agent_usage_hint_text",
    "features.multi_agent_v2.subagent_usage_hint_text",
];

pub(crate) fn materialized_config_toml(config: &Config) -> anyhow::Result<ConfigToml> {
    let mut materialized: ConfigToml = config
        .config_layer_stack
        .effective_config()
        .try_into()
        .context("failed to deserialize effective config for config interpolation")?;
    apply_resolved_config_fields(config, &mut materialized)?;
    Ok(materialized)
}

pub(crate) fn interpolate_config_string_fields(
    config_toml: &mut ConfigToml,
    interpolation_source: &TomlValue,
) -> anyhow::Result<bool> {
    let mut target_value = TomlValue::try_from(config_toml.clone())
        .context("failed to serialize config for interpolation")?;
    let mut changed = false;

    for field_path in INTERPOLATED_CONFIG_STRING_FIELDS {
        let Some(value) = value_mut_at_path(&mut target_value, field_path) else {
            continue;
        };
        let Some(template_source) = value.as_str() else {
            bail!("interpolated config field `{field_path}` must be a string");
        };
        let template = Template::parse(template_source)
            .with_context(|| format!("failed to parse template in config field `{field_path}`"))?;
        let rendered = render_template(&template, interpolation_source, field_path)?;
        if rendered != template_source {
            *value = TomlValue::String(rendered);
            changed = true;
        }
    }

    if changed {
        *config_toml = target_value
            .try_into()
            .context("failed to deserialize interpolated config")?;
    }

    Ok(changed)
}

pub(crate) fn apply_resolved_config_fields(
    config: &Config,
    config_toml: &mut ConfigToml,
) -> anyhow::Result<()> {
    config_toml.web_search = Some(config.web_search_mode.value());
    config_toml.model_provider = Some(config.model_provider_id.clone());
    config_toml.plan_mode_reasoning_effort = config.plan_mode_reasoning_effort;
    config_toml.model_verbosity = config.model_verbosity;
    config_toml.include_permissions_instructions = Some(config.include_permissions_instructions);
    config_toml.include_apps_instructions = Some(config.include_apps_instructions);
    config_toml.include_environment_context = Some(config.include_environment_context);
    config_toml.background_terminal_max_timeout = Some(config.background_terminal_max_timeout);

    // Feature aliases and feature configs need to be written in their resolved
    // form; otherwise replay can drift when a legacy key maps to the same
    // runtime feature.
    let features = config_toml
        .features
        .get_or_insert_with(FeaturesToml::default);
    features.materialize_resolved_enabled(config.features.get());
    let mut multi_agent_v2: MultiAgentV2ConfigToml =
        resolved_config_to_toml(&config.multi_agent_v2, "features.multi_agent_v2")?;
    multi_agent_v2.enabled = Some(config.features.enabled(Feature::MultiAgentV2));
    features.multi_agent_v2 = Some(FeatureToml::Config(multi_agent_v2));
    features.apps_mcp_path_override = Some(FeatureToml::Config(AppsMcpPathOverrideConfigToml {
        enabled: Some(config.features.enabled(Feature::AppsMcpPathOverride)),
        path: config.apps_mcp_path_override.clone(),
    }));

    config_toml.memories = Some(resolved_config_to_toml::<MemoriesToml>(
        &config.memories,
        "memories",
    )?);

    let agents = config_toml.agents.get_or_insert_with(Default::default);
    // Multi-agent v2 owns thread fanout through its feature config. Preserve
    // the legacy agents.max_threads setting only when v2 is disabled.
    agents.max_threads = if config.features.enabled(Feature::MultiAgentV2) {
        None
    } else {
        config.agent_max_threads
    };
    agents.max_depth = Some(config.agent_max_depth);
    agents.job_max_runtime_seconds = config.agent_job_max_runtime_seconds;
    agents.interrupt_message = Some(config.agent_interrupt_message_enabled);

    config_toml
        .skills
        .get_or_insert_with(Default::default)
        .include_instructions = Some(config.include_skill_instructions);

    Ok(())
}

fn render_template(
    template: &Template,
    interpolation_source: &TomlValue,
    field_path: &str,
) -> anyhow::Result<String> {
    let variables =
        template
            .placeholders()
            .map(|placeholder| {
                let value = lookup_scalar_path(interpolation_source, placeholder).with_context(|| {
                format!("failed to render config field `{field_path}` placeholder `{placeholder}`")
            })?;
                Ok((placeholder.to_string(), value))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

    template
        .render(
            variables
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )
        .with_context(|| format!("failed to render config field `{field_path}`"))
}

fn lookup_scalar_path(value: &TomlValue, path: &str) -> anyhow::Result<String> {
    let resolved = value_at_path(value, path)
        .with_context(|| format!("template placeholder `{path}` does not exist"))?;

    match resolved {
        TomlValue::String(value) => Ok(value.clone()),
        TomlValue::Integer(value) => Ok(value.to_string()),
        TomlValue::Float(value) => Ok(value.to_string()),
        TomlValue::Boolean(value) => Ok(value.to_string()),
        _ => bail!(
            "template placeholder `{path}` must resolve to a scalar string, integer, float, or boolean"
        ),
    }
}

fn value_at_path<'a>(value: &'a TomlValue, path: &str) -> Option<&'a TomlValue> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.as_table()?.get(segment)?;
    }
    Some(current)
}

fn value_mut_at_path<'a>(value: &'a mut TomlValue, path: &str) -> Option<&'a mut TomlValue> {
    let mut current = value;
    let mut segments = path.split('.').peekable();

    while let Some(segment) = segments.next() {
        let table = current.as_table_mut()?;
        if segments.peek().is_none() {
            return table.get_mut(segment);
        }
        current = table.get_mut(segment)?;
    }

    Some(current)
}

fn resolved_config_to_toml<Toml>(
    value: &impl serde::Serialize,
    label: &'static str,
) -> anyhow::Result<Toml>
where
    Toml: serde::de::DeserializeOwned + serde::Serialize,
{
    crate::config_lock::toml_round_trip(value, label).map_err(anyhow::Error::from)
}
