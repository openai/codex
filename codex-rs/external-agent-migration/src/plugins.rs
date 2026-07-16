use codex_analytics::PluginInstallSource;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginInstallError;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::marketplace::MarketplaceError;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_core_plugins::marketplace_add::MarketplaceAddRequest;
use codex_core_plugins::marketplace_add::add_marketplace;
use codex_core_plugins::marketplace_add::is_local_marketplace_source;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use crate::migration_source::MarketplaceImportSource;
use crate::model::MigrationDetails;
use crate::model::PluginImportOutcome;
use crate::reporting::plugin_import_raw_error;
use crate::reporting::record_plugin_import_errors;
use crate::scope::MigrationScope;
use crate::service::ExternalAgentConfigService;
use crate::utils::invalid_data_error;

impl ExternalAgentConfigService {
    fn marketplace_import_sources(
        &self,
        cwd: Option<&Path>,
    ) -> io::Result<BTreeMap<String, MarketplaceImportSource>> {
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(BTreeMap::new());
        };
        let source_root = scope
            .repo_root()
            .unwrap_or(self.external_agent_home.as_path());
        let source_settings = self.source_settings(&scope);
        self.source.marketplace_import_sources(
            self.external_agent_home.as_path(),
            source_root,
            &source_settings,
        )
    }

    pub(super) fn partition_plugin_migration_details(
        &self,
        cwd: Option<&Path>,
        details: MigrationDetails,
    ) -> io::Result<(Option<MigrationDetails>, Option<MigrationDetails>)> {
        let import_sources = self.marketplace_import_sources(cwd)?;

        let mut local_plugins = Vec::new();
        let mut remote_plugins = Vec::new();
        for plugin_group in details.plugins {
            let is_local = import_sources
                .get(&plugin_group.marketplace_name)
                .and_then(|import_source| {
                    is_local_marketplace_source(
                        &import_source.source,
                        import_source.ref_name.clone(),
                    )
                    .ok()
                })
                .unwrap_or(false);

            if is_local {
                local_plugins.push(plugin_group);
            } else {
                remote_plugins.push(plugin_group);
            }
        }

        let local_details = (!local_plugins.is_empty()).then_some(MigrationDetails {
            plugins: local_plugins,
            ..Default::default()
        });
        let remote_details = (!remote_plugins.is_empty()).then_some(MigrationDetails {
            plugins: remote_plugins,
            ..Default::default()
        });

        Ok((local_details, remote_details))
    }

    pub async fn import_plugins(
        &self,
        cwd: Option<&Path>,
        details: Option<MigrationDetails>,
    ) -> io::Result<PluginImportOutcome> {
        let Some(MigrationDetails { plugins, .. }) = details else {
            return Err(invalid_data_error(
                "plugins migration item is missing details".to_string(),
            ));
        };
        let config = ConfigBuilder::default()
            .codex_home(self.codex_home.clone())
            .fallback_cwd(Some(
                cwd.map(Path::to_path_buf)
                    .unwrap_or_else(|| self.codex_home.clone()),
            ))
            .build()
            .await
            .map_err(|err| io::Error::other(format!("failed to load config: {err}")))?;
        let requirements = config.config_layer_stack.requirements().clone();
        let mut outcome = PluginImportOutcome::default();
        let plugins_manager = PluginsManager::new(self.codex_home.clone())
            .with_plugin_install_source(PluginInstallSource::ExternalAgentMigration);
        if let Some(analytics_events_client) = self.analytics_events_client.clone() {
            plugins_manager.set_analytics_events_client(analytics_events_client);
        }
        let configured_marketplace_paths = plugins_manager
            .list_marketplaces_for_config(
                &config.plugins_config_input(),
                &[],
                /*include_openai_curated*/ true,
            )
            .map_err(|err| {
                invalid_data_error(format!("failed to list configured marketplaces: {err}"))
            })?
            .marketplaces
            .into_iter()
            .map(|marketplace| (marketplace.name, marketplace.path))
            .collect::<BTreeMap<_, _>>();
        let import_sources = self.marketplace_import_sources(cwd)?;
        for plugin_group in plugins {
            let marketplace_name = plugin_group.marketplace_name.clone();
            let plugin_names = plugin_group.plugin_names;
            let plugin_ids = plugin_names
                .iter()
                .map(|plugin_name| format!("{plugin_name}@{marketplace_name}"))
                .collect::<Vec<_>>();
            let marketplace_path = if let Some(marketplace_path) =
                configured_marketplace_paths.get(&marketplace_name)
            {
                outcome
                    .succeeded_marketplaces
                    .push(marketplace_name.clone());
                marketplace_path.clone()
            } else {
                let Some(import_source) = import_sources.get(&marketplace_name).cloned() else {
                    let message = format!(
                        "external agent plugin marketplace source was not found: {marketplace_name}"
                    );
                    record_plugin_import_errors(
                        &mut outcome,
                        cwd,
                        &plugin_ids,
                        "plugin_import",
                        message,
                    );
                    outcome.failed_marketplaces.push(marketplace_name);
                    outcome.failed_plugin_ids.extend(plugin_ids);
                    continue;
                };
                let request = MarketplaceAddRequest {
                    source: import_source.source,
                    ref_name: import_source.ref_name,
                    sparse_paths: Vec::new(),
                };
                match add_marketplace(self.codex_home.clone(), requirements.clone(), request).await
                {
                    Ok(add_marketplace_outcome) => {
                        let Some(marketplace_path) = find_marketplace_manifest_path(
                            add_marketplace_outcome.installed_root.as_path(),
                        ) else {
                            let message = format!(
                                "plugin marketplace manifest was not found after install: {marketplace_name}"
                            );
                            record_plugin_import_errors(
                                &mut outcome,
                                cwd,
                                &plugin_ids,
                                "plugin_import",
                                message,
                            );
                            outcome.failed_marketplaces.push(marketplace_name);
                            outcome.failed_plugin_ids.extend(plugin_ids);
                            continue;
                        };
                        outcome
                            .succeeded_marketplaces
                            .push(marketplace_name.clone());
                        marketplace_path
                    }
                    Err(err) => {
                        record_plugin_import_errors(
                            &mut outcome,
                            cwd,
                            &plugin_ids,
                            "plugin_import",
                            err.to_string(),
                        );
                        outcome.failed_marketplaces.push(marketplace_name);
                        outcome.failed_plugin_ids.extend(plugin_ids);
                        continue;
                    }
                }
            };
            let install_config = match ConfigBuilder::default()
                .codex_home(self.codex_home.clone())
                .fallback_cwd(Some(
                    cwd.map(Path::to_path_buf)
                        .unwrap_or_else(|| self.codex_home.clone()),
                ))
                .build()
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    record_plugin_import_errors(
                        &mut outcome,
                        cwd,
                        &plugin_ids,
                        "plugin_import",
                        format!("failed to reload config after adding marketplace: {err}"),
                    );
                    outcome.failed_plugin_ids.extend(plugin_ids);
                    continue;
                }
            };
            for plugin_name in plugin_names {
                match plugins_manager
                    .install_plugin(
                        &install_config.config_layer_stack,
                        PluginInstallRequest {
                            plugin_name: plugin_name.clone(),
                            marketplace_path: marketplace_path.clone(),
                        },
                    )
                    .await
                {
                    Ok(_) => outcome
                        .succeeded_plugin_ids
                        .push(format!("{plugin_name}@{marketplace_name}")),
                    Err(err) => {
                        let plugin_id = format!("{plugin_name}@{marketplace_name}");
                        outcome.failed_plugin_ids.push(plugin_id.clone());
                        let sub_error_type = err.sub_error_type();
                        let mut raw_error = plugin_import_raw_error(
                            cwd,
                            "plugin_import",
                            err.to_string(),
                            Some(plugin_id),
                        );
                        raw_error.sub_error_type = sub_error_type;
                        if matches!(
                            err,
                            PluginInstallError::Marketplace(
                                MarketplaceError::PluginNotFound { .. }
                            )
                        ) {
                            raw_error.error_type = Some("plugin_not_found".to_string());
                        }
                        outcome.raw_errors.push(raw_error);
                    }
                }
            }
        }

        Ok(outcome)
    }
}
