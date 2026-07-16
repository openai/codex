use crate::migration_source::DetectedSourcePlugins;
use crate::migration_source::PluginDetectionContext;
use crate::model::MigrationDetails;
use crate::model::PluginsMigration;
use crate::source_cla;
use crate::source_cur;
use serde_json::Value as JsonValue;
use std::io;

pub(crate) fn detect_cla_plugins(
    context: &PluginDetectionContext<'_>,
) -> Option<DetectedSourcePlugins> {
    let settings = context.settings?;
    let import_sources = source_cla::marketplace_import_sources(
        settings,
        context.external_agent_home,
        context.source_root,
    );
    let details = source_cla::extract_plugin_migration_details(
        settings,
        &import_sources,
        context.configured_plugin_ids,
        context.configured_marketplace_plugins,
    )?;
    Some(DetectedSourcePlugins {
        description: format!(
            "Migrate enabled plugins from {}",
            context.source_settings.display()
        ),
        details,
    })
}

pub(crate) fn can_detect_cla_plugins(settings: Option<&JsonValue>) -> bool {
    settings.is_some()
}

pub(crate) fn detect_cur_plugins(
    context: &PluginDetectionContext<'_>,
) -> io::Result<Option<DetectedSourcePlugins>> {
    let mut plugins = Vec::new();
    for marketplace in source_cur::cached_marketplace_plugins(context.external_agent_home)? {
        let configured_marketplace = context
            .configured_marketplace_plugins
            .get(&marketplace.name);
        let plugin_names = marketplace
            .plugin_names
            .into_iter()
            .filter(|plugin_name| {
                !context
                    .configured_plugin_ids
                    .contains(&format!("{plugin_name}@{}", marketplace.name))
                    && configured_marketplace.is_none_or(|plugins| plugins.contains(plugin_name))
            })
            .collect::<Vec<_>>();
        if !plugin_names.is_empty() {
            plugins.push(PluginsMigration {
                marketplace_name: marketplace.name,
                plugin_names,
            });
        }
    }
    if plugins.is_empty() {
        return Ok(None);
    }
    Ok(Some(DetectedSourcePlugins {
        description: format!(
            "Migrate cached plugins from {}",
            context.external_agent_home.join("plugins/cache").display()
        ),
        details: MigrationDetails {
            plugins,
            ..Default::default()
        },
    }))
}
