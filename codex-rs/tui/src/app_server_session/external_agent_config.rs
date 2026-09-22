//! External setup migration uses the connected server's filesystem and tracks this client's import.

use super::*;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;

impl AppServerSession {
    pub(crate) async fn external_agent_config_detect(
        &mut self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::ExternalAgentConfigDetect { request_id, params })
            .await
            .wrap_err("externalAgentConfig/detect failed during external agent import")
    }

    pub(crate) async fn external_agent_config_import(
        &mut self,
        migration_items: Vec<ExternalAgentConfigMigrationItem>,
        migration_source: String,
    ) -> Result<()> {
        if self.external_agent_config_import_in_progress() {
            color_eyre::eyre::bail!(EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE);
        }
        let request_id = self.next_request_id();
        let response: ExternalAgentConfigImportResponse = self
            .client
            .request_typed(ClientRequest::ExternalAgentConfigImport {
                request_id,
                params: ExternalAgentConfigImportParams {
                    migration_items,
                    source: Some("cli".to_string()),
                    provider_id: Some(migration_source.clone()),
                    migration_source: Some(migration_source),
                },
            })
            .await
            .wrap_err("externalAgentConfig/import failed during external agent import")?;
        // Notifications remain queued until the event loop regains this mutable session.
        // Shared daemons also broadcast completions for imports started by other clients.
        *self
            .external_agent_config_import_id
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(response.import_id);
        Ok(())
    }

    pub(crate) fn external_agent_config_import_in_progress(&self) -> bool {
        self.external_agent_config_import_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    pub(crate) fn consume_external_agent_config_import_completion(&self, import_id: &str) -> bool {
        let mut pending = self
            .external_agent_config_import_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pending.as_deref() != Some(import_id) {
            return false;
        }
        *pending = None;
        true
    }
}

#[cfg(test)]
#[path = "external_agent_config_tests.rs"]
mod tests;
