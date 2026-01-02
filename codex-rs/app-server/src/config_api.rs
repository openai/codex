use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::config::ConfigService;
use codex_core::config::ConfigServiceError;
use serde_json::json;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[derive(Clone)]
pub(crate) struct ConfigApi {
    service: ConfigService,
}

impl ConfigApi {
    pub(crate) fn new(codex_home: PathBuf, cli_overrides: Vec<(String, TomlValue)>) -> Self {
        Self {
            service: ConfigService::new(codex_home, cli_overrides),
        }
    }

    pub(crate) async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, JSONRPCErrorError> {
        self.service.read(params).await.map_err(map_error)
    }

    pub(crate) async fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        self.service.write_value(params).await.map_err(map_error)
    }

    pub(crate) async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        self.service.batch_write(params).await.map_err(map_error)
    }
}

fn map_error(err: ConfigServiceError) -> JSONRPCErrorError {
    if let Some(code) = err.write_error_code() {
        return config_write_error(code, err.to_string());
    }

    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: err.to_string(),
        data: None,
    }
}

fn config_write_error(code: ConfigWriteErrorCode, message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: message.into(),
        data: Some(json!({
            "config_write_error_code": code,
        })),
    }
}
