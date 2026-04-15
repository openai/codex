use crate::error_code::INTERNAL_ERROR_CODE;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_core::external_agent_config::ExternalAgentConfigDetectOptions;
use codex_core::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use codex_core::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use codex_core::external_agent_config::ExternalAgentConfigService;
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
                    details: migration_item.details.map(|details| MigrationDetails {
                        plugins: details
                            .plugins
                            .into_iter()
                            .map(|plugin| PluginsMigration {
                                marketplace_name: plugin.marketplace_name,
                                plugin_ids: plugin.plugin_ids,
                            })
                            .collect(),
                    }),
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
                        details: migration_item.details.map(|details| {
                            codex_core::external_agent_config::MigrationDetails {
                                plugins: details
                                    .plugins
                                    .into_iter()
                                    .map(|plugin| {
                                        codex_core::external_agent_config::PluginsMigration {
                                            marketplace_name: plugin.marketplace_name,
                                            plugin_ids: plugin.plugin_ids,
                                        }
                                    })
                                    .collect(),
                            }
                        }),
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
