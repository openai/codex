use crate::error_code::INTERNAL_ERROR_CODE;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::external_agent_config::ExternalAgentConfigDetectOptions;
use codex_core::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use codex_core::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use codex_core::external_agent_config::ExternalAgentConfigService;
use codex_utils_absolute_path::AbsolutePathBuf;
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
        let cwds = params
            .cwds
            .map(|cwds| {
                cwds.into_iter()
                    .map(AbsolutePathBuf::relative_to_current_dir)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|err| map_io_error(io::Error::new(io::ErrorKind::InvalidInput, err)))?;
        let items = self
            .migration_service
            .detect(ExternalAgentConfigDetectOptions {
                include_home: params.include_home,
                cwds,
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
                        CoreMigrationItemType::McpServerConfig => {
                            ExternalAgentConfigMigrationItemType::McpServerConfig
                        }
                    },
                    description: migration_item.description,
                    cwd: migration_item.cwd.map(AbsolutePathBuf::into_path_buf),
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
                    .map(|migration_item| {
                        let cwd = migration_item
                            .cwd
                            .map(AbsolutePathBuf::relative_to_current_dir)
                            .transpose()
                            .map_err(|err| {
                                map_io_error(io::Error::new(io::ErrorKind::InvalidInput, err))
                            })?;
                        Ok(CoreMigrationItem {
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
                                ExternalAgentConfigMigrationItemType::McpServerConfig => {
                                    CoreMigrationItemType::McpServerConfig
                                }
                            },
                            description: migration_item.description,
                            cwd,
                        })
                    })
                    .collect::<Result<Vec<_>, JSONRPCErrorError>>()?,
            )
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
