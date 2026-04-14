use crate::error_code::INTERNAL_ERROR_CODE;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::PluginMarketplaceMigration;
use codex_app_server_protocol::PluginMarketplaceSource;
use codex_app_server_protocol::PluginsMigrationDetails;
use codex_core::external_agent_config::ExternalAgentConfigDetectOptions;
use codex_core::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use codex_core::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use codex_core::external_agent_config::ExternalAgentConfigService;
use codex_core::external_agent_config::PluginMarketplaceMigration as CorePluginMarketplaceMigration;
use codex_core::external_agent_config::PluginMarketplaceSource as CorePluginMarketplaceSource;
use codex_core::external_agent_config::PluginsMigrationDetails as CorePluginsMigrationDetails;
use std::io;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigApi {
    migration_service: ExternalAgentConfigService,
}

impl ExternalAgentConfigApi {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self {
            migration_service: ExternalAgentConfigService::new(codex_home),
        }
    }

    pub(crate) async fn detect(
        &self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse, JSONRPCErrorError> {
        let items = self
            .migration_service
            .detect(ExternalAgentConfigDetectOptions {
                include_home: params.include_home,
                cwds: params.cwds,
            })
            .map_err(map_io_error)?;

        Ok(ExternalAgentConfigDetectResponse {
            items: items
                .into_iter()
                .map(|migration_item| ExternalAgentConfigMigrationItem {
                    item_type: match migration_item.item_type {
                        CoreMigrationItemType::Config => {
                            ExternalAgentConfigMigrationItemType::Config
                        }
                        CoreMigrationItemType::Skills => {
                            ExternalAgentConfigMigrationItemType::Skills
                        }
                        CoreMigrationItemType::AgentsMd => {
                            ExternalAgentConfigMigrationItemType::AgentsMd
                        }
                        CoreMigrationItemType::Plugins => {
                            ExternalAgentConfigMigrationItemType::Plugins
                        }
                        CoreMigrationItemType::McpServerConfig => {
                            ExternalAgentConfigMigrationItemType::McpServerConfig
                        }
                    },
                    description: migration_item.description,
                    cwd: migration_item.cwd,
                    details: migration_item.details.map(map_details_to_api),
                })
                .collect(),
        })
    }

    pub(crate) async fn import(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> Result<ExternalAgentConfigImportResponse, JSONRPCErrorError> {
        self.migration_service
            .import(
                params
                    .migration_items
                    .into_iter()
                    .map(|migration_item| CoreMigrationItem {
                        item_type: match migration_item.item_type {
                            ExternalAgentConfigMigrationItemType::Config => {
                                CoreMigrationItemType::Config
                            }
                            ExternalAgentConfigMigrationItemType::Skills => {
                                CoreMigrationItemType::Skills
                            }
                            ExternalAgentConfigMigrationItemType::AgentsMd => {
                                CoreMigrationItemType::AgentsMd
                            }
                            ExternalAgentConfigMigrationItemType::Plugins => {
                                CoreMigrationItemType::Plugins
                            }
                            ExternalAgentConfigMigrationItemType::McpServerConfig => {
                                CoreMigrationItemType::McpServerConfig
                            }
                        },
                        description: migration_item.description,
                        cwd: migration_item.cwd,
                        details: migration_item.details.map(map_details_to_core),
                    })
                    .collect(),
            )
            .await
            .map_err(map_io_error)?;

        Ok(ExternalAgentConfigImportResponse {})
    }
}

fn map_io_error(err: io::Error) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: err.to_string(),
        data: None,
    }
}

fn map_plugin_marketplace_to_api(
    details: CorePluginMarketplaceMigration,
) -> PluginMarketplaceMigration {
    PluginMarketplaceMigration {
        name: details.name,
        source: map_plugin_marketplace_source_to_api(details.source),
        repo: details.repo,
        ref_name: details.ref_name,
    }
}

fn map_plugin_marketplace_to_core(
    details: PluginMarketplaceMigration,
) -> CorePluginMarketplaceMigration {
    CorePluginMarketplaceMigration {
        name: details.name,
        source: map_plugin_marketplace_source_to_core(details.source),
        repo: details.repo,
        ref_name: details.ref_name,
    }
}

fn map_plugin_marketplace_source_to_api(
    source: CorePluginMarketplaceSource,
) -> PluginMarketplaceSource {
    match source {
        CorePluginMarketplaceSource::Github => PluginMarketplaceSource::Github,
    }
}

fn map_plugin_marketplace_source_to_core(
    source: PluginMarketplaceSource,
) -> CorePluginMarketplaceSource {
    match source {
        PluginMarketplaceSource::Github => CorePluginMarketplaceSource::Github,
    }
}

fn map_details_to_api(details: CorePluginsMigrationDetails) -> PluginsMigrationDetails {
    PluginsMigrationDetails {
        marketplaces: details
            .marketplaces
            .into_iter()
            .map(map_plugin_marketplace_to_api)
            .collect(),
        plugin_ids: details.plugin_ids,
    }
}

fn map_details_to_core(details: PluginsMigrationDetails) -> CorePluginsMigrationDetails {
    CorePluginsMigrationDetails {
        marketplaces: details
            .marketplaces
            .into_iter()
            .map(map_plugin_marketplace_to_core)
            .collect(),
        plugin_ids: details.plugin_ids,
    }
}
