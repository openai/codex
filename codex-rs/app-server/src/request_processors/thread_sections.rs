use super::thread_processor::THREAD_LIST_DEFAULT_LIMIT;
use super::thread_processor::THREAD_LIST_MAX_LIMIT;
use super::thread_processor::ThreadRequestProcessor;
use crate::error_code::internal_error;
use crate::error_code::invalid_params;
use crate::error_code::method_not_found;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ThreadSection;
use codex_app_server_protocol::ThreadSectionCreateParams;
use codex_app_server_protocol::ThreadSectionCreateResponse;
use codex_app_server_protocol::ThreadSectionDeleteParams;
use codex_app_server_protocol::ThreadSectionDeleteResponse;
use codex_app_server_protocol::ThreadSectionListParams;
use codex_app_server_protocol::ThreadSectionListResponse;
use codex_app_server_protocol::ThreadSectionUpdateParams;
use codex_app_server_protocol::ThreadSectionUpdateResponse;
use codex_state::PINNED_THREAD_SECTION_ID;
use codex_thread_store::CreateThreadSectionParams as StoreCreateThreadSectionParams;
use codex_thread_store::DeleteThreadSectionParams as StoreDeleteThreadSectionParams;
use codex_thread_store::ListThreadSectionsParams as StoreListThreadSectionsParams;
use codex_thread_store::RenameThreadSectionParams as StoreRenameThreadSectionParams;
use codex_thread_store::ThreadStoreError;

impl ThreadRequestProcessor {
    pub(crate) async fn thread_section_list(
        &self,
        params: ThreadSectionListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "threadSection/list";
        self.ensure_thread_sections_supported(OPERATION)?;
        let limit = params
            .limit
            .map(|value| value as usize)
            .unwrap_or(THREAD_LIST_DEFAULT_LIMIT)
            .clamp(1, THREAD_LIST_MAX_LIMIT);
        let page = self
            .thread_store
            .list_thread_sections(StoreListThreadSectionsParams {
                cursor: params.cursor,
                limit,
            })
            .await
            .map_err(|err| thread_section_store_error(OPERATION, err))?;

        Ok(Some(
            ThreadSectionListResponse {
                data: page
                    .sections
                    .into_iter()
                    .map(|section| ThreadSection {
                        id: section.id,
                        name: section.name,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            }
            .into(),
        ))
    }

    pub(crate) async fn thread_section_create(
        &self,
        params: ThreadSectionCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "threadSection/create";
        self.ensure_thread_sections_supported(OPERATION)?;
        let name = params.name.trim();
        if name.is_empty() {
            return Err(invalid_params("section name must not be empty"));
        }
        let section = self
            .thread_store
            .create_thread_section(StoreCreateThreadSectionParams {
                name: name.to_string(),
            })
            .await
            .map_err(|err| thread_section_store_error(OPERATION, err))?;

        Ok(Some(
            ThreadSectionCreateResponse {
                section: ThreadSection {
                    id: section.id,
                    name: section.name,
                },
            }
            .into(),
        ))
    }

    pub(crate) async fn thread_section_update(
        &self,
        params: ThreadSectionUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "threadSection/update";
        self.ensure_thread_sections_supported(OPERATION)?;
        let name = params.name.trim();
        if name.is_empty() {
            return Err(invalid_params("section name must not be empty"));
        }
        if params.section_id.trim().is_empty() {
            return Err(invalid_params("sectionId must not be empty"));
        }
        if params.section_id == PINNED_THREAD_SECTION_ID {
            return Err(invalid_params(
                "the built-in pinned section cannot be renamed",
            ));
        }
        let section = self
            .thread_store
            .rename_thread_section(StoreRenameThreadSectionParams {
                section_id: params.section_id.clone(),
                name: name.to_string(),
            })
            .await
            .map_err(|err| thread_section_store_error(OPERATION, err))?
            .ok_or_else(|| {
                invalid_params(format!("thread section not found: {}", params.section_id))
            })?;

        Ok(Some(
            ThreadSectionUpdateResponse {
                section: ThreadSection {
                    id: section.id,
                    name: section.name,
                },
            }
            .into(),
        ))
    }

    pub(crate) async fn thread_section_delete(
        &self,
        params: ThreadSectionDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "threadSection/delete";
        self.ensure_thread_sections_supported(OPERATION)?;
        if params.section_id.trim().is_empty() {
            return Err(invalid_params("sectionId must not be empty"));
        }
        if params.section_id == PINNED_THREAD_SECTION_ID {
            return Err(invalid_params(
                "the built-in pinned section cannot be deleted",
            ));
        }
        let deleted = self
            .thread_store
            .delete_thread_section(StoreDeleteThreadSectionParams {
                section_id: params.section_id.clone(),
            })
            .await
            .map_err(|err| thread_section_store_error(OPERATION, err))?;
        if !deleted {
            return Err(invalid_params(format!(
                "thread section not found: {}",
                params.section_id
            )));
        }

        Ok(Some(ThreadSectionDeleteResponse {}.into()))
    }

    fn ensure_thread_sections_supported(
        &self,
        operation: &'static str,
    ) -> Result<(), JSONRPCErrorError> {
        if self.thread_store.supports_thread_sections() {
            Ok(())
        } else {
            Err(unsupported_thread_section_operation(operation))
        }
    }
}

fn unsupported_thread_section_operation(operation: &'static str) -> JSONRPCErrorError {
    method_not_found(format!("{operation} is unavailable without sqlite state"))
}

fn thread_section_store_error(
    operation: &'static str,
    error: ThreadStoreError,
) -> JSONRPCErrorError {
    match error {
        ThreadStoreError::Unsupported { .. } => unsupported_thread_section_operation(operation),
        ThreadStoreError::InvalidRequest { message } => invalid_params(message),
        error @ (ThreadStoreError::ThreadNotFound { .. }
        | ThreadStoreError::Conflict { .. }
        | ThreadStoreError::Internal { .. }) => {
            let action = operation
                .strip_prefix("threadSection/")
                .unwrap_or(operation);
            internal_error(format!("failed to {action} thread section: {error}"))
        }
    }
}
