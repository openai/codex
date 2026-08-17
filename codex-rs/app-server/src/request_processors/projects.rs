use std::collections::HashSet;
use std::sync::Arc;

use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::Project;
use codex_app_server_protocol::ProjectChangeType;
use codex_app_server_protocol::ProjectChangedNotification;
use codex_app_server_protocol::ProjectCreateParams;
use codex_app_server_protocol::ProjectCreateResponse;
use codex_app_server_protocol::ProjectDeleteParams;
use codex_app_server_protocol::ProjectDeleteResponse;
use codex_app_server_protocol::ProjectImportParams;
use codex_app_server_protocol::ProjectImportResponse;
use codex_app_server_protocol::ProjectListParams;
use codex_app_server_protocol::ProjectListResponse;
use codex_app_server_protocol::ProjectMoveParams;
use codex_app_server_protocol::ProjectMoveResponse;
use codex_app_server_protocol::ProjectReadParams;
use codex_app_server_protocol::ProjectReadResponse;
use codex_app_server_protocol::ProjectRoot;
use codex_app_server_protocol::ProjectUpdateParams;
use codex_app_server_protocol::ProjectUpdateResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadProjectUpdatedNotification;
use codex_thread_store::CreateProjectParams as StoreCreateProjectParams;
use codex_thread_store::ListProjectsParams as StoreListProjectsParams;
use codex_thread_store::MoveProjectParams as StoreMoveProjectParams;
use codex_thread_store::ProjectMoveOutcome;
use codex_thread_store::StoredProject;
use codex_thread_store::StoredProjectRoot;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::UpdateProjectParams as StoreUpdateProjectParams;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

use super::thread_processor::THREAD_LIST_DEFAULT_LIMIT;
use super::thread_processor::THREAD_LIST_MAX_LIMIT;
use crate::error_code::internal_error;
use crate::error_code::invalid_params;
use crate::error_code::method_not_found;
use crate::outgoing_message::OutgoingMessageSender;

#[derive(Clone)]
pub(crate) struct ProjectRequestProcessor {
    thread_store: Arc<dyn ThreadStore>,
    outgoing: Arc<OutgoingMessageSender>,
    thread_list_state_permit: Arc<Semaphore>,
}

impl ProjectRequestProcessor {
    pub(crate) fn new(
        thread_store: Arc<dyn ThreadStore>,
        outgoing: Arc<OutgoingMessageSender>,
        thread_list_state_permit: Arc<Semaphore>,
    ) -> Self {
        Self {
            thread_store,
            outgoing,
            thread_list_state_permit,
        }
    }

    pub(crate) async fn project_list(
        &self,
        params: ProjectListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let page = self
            .thread_store
            .list_projects(StoreListProjectsParams {
                cursor: params.cursor,
                limit: params
                    .limit
                    .map(|limit| limit as usize)
                    .unwrap_or(THREAD_LIST_DEFAULT_LIMIT)
                    .clamp(1, THREAD_LIST_MAX_LIMIT),
            })
            .await
            .map_err(|error| project_store_error("project/list", error))?;
        Ok(Some(
            ProjectListResponse {
                data: page
                    .projects
                    .into_iter()
                    .map(api_project)
                    .collect::<Result<_, _>>()?,
                next_cursor: page.next_cursor,
            }
            .into(),
        ))
    }

    pub(crate) async fn project_read(
        &self,
        params: ProjectReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let project = self
            .thread_store
            .read_project(params.project_id.clone())
            .await
            .map_err(|error| project_store_error("project/read", error))?
            .ok_or_else(|| invalid_params(format!("project not found: {}", params.project_id)))?;
        Ok(Some(
            ProjectReadResponse {
                project: api_project(project)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn project_create(
        &self,
        params: ProjectCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let (project, created, _) = self
            .create_project(
                params.name,
                params.roots,
                params.metadata,
                Vec::new(),
                params.idempotency_key,
                "project/create",
            )
            .await?;
        if created {
            self.notify_project_changed(&project.id, ProjectChangeType::Created)
                .await;
        }
        Ok(Some(ProjectCreateResponse { project }.into()))
    }

    pub(crate) async fn project_import(
        &self,
        params: ProjectImportParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let _thread_list_state_permit = self.acquire_thread_list_state_permit().await?;
        let (project, created, thread_ids) = self
            .create_project(
                params.name,
                params.roots,
                params.metadata,
                params.threads.unwrap_or_default(),
                params.idempotency_key,
                "project/import",
            )
            .await?;
        if created {
            self.notify_project_changed(&project.id, ProjectChangeType::Created)
                .await;
            self.notify_thread_projects(thread_ids, Some(project.id.clone()))
                .await;
        }
        Ok(Some(ProjectImportResponse { project }.into()))
    }

    pub(crate) async fn project_update(
        &self,
        params: ProjectUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let name = params.name.map(validate_name).transpose()?;
        let roots = params.roots.map(validate_roots).transpose()?;
        let updated = self
            .thread_store
            .update_project(StoreUpdateProjectParams {
                project_id: params.project_id.clone(),
                name,
                roots,
                metadata: params.metadata,
            })
            .await
            .map_err(|error| project_store_error("project/update", error))?
            .ok_or_else(|| invalid_params(format!("project not found: {}", params.project_id)))?;
        let project = api_project(updated.project)?;
        if updated.changed {
            self.notify_project_changed(&project.id, ProjectChangeType::Updated)
                .await;
        }
        Ok(Some(ProjectUpdateResponse { project }.into()))
    }

    pub(crate) async fn project_move(
        &self,
        params: ProjectMoveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let project_id = params.project_id;
        let outcome = self
            .thread_store
            .move_project(StoreMoveProjectParams {
                project_id: project_id.clone(),
                before_project_id: params.before_project_id,
            })
            .await
            .map_err(|error| project_store_error("project/move", error))?
            .ok_or_else(|| invalid_params(format!("project not found: {project_id}")))?;
        if outcome == ProjectMoveOutcome::Moved {
            self.notify_project_changed(&project_id, ProjectChangeType::Updated)
                .await;
        }
        Ok(Some(ProjectMoveResponse {}.into()))
    }

    pub(crate) async fn project_delete(
        &self,
        params: ProjectDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let _thread_list_state_permit = self.acquire_thread_list_state_permit().await?;
        let deleted = self
            .thread_store
            .delete_project(params.project_id.clone())
            .await
            .map_err(|error| project_store_error("project/delete", error))?
            .ok_or_else(|| invalid_params(format!("project not found: {}", params.project_id)))?;
        self.notify_project_changed(&params.project_id, ProjectChangeType::Deleted)
            .await;
        self.notify_thread_projects(deleted.affected_active_thread_ids, /*project_id*/ None)
            .await;
        Ok(Some(ProjectDeleteResponse {}.into()))
    }

    async fn create_project(
        &self,
        name: String,
        roots: Vec<ProjectRoot>,
        metadata: Option<std::collections::BTreeMap<String, String>>,
        thread_ids: Vec<String>,
        idempotency_key: String,
        operation: &'static str,
    ) -> Result<(Project, bool, Vec<String>), JSONRPCErrorError> {
        let idempotency_key = validate_idempotency_key(idempotency_key)?;
        let thread_ids = validate_thread_ids(thread_ids)?;
        let created = self
            .thread_store
            .create_project(StoreCreateProjectParams {
                name: validate_name(name)?,
                roots: validate_roots(roots)?,
                metadata: metadata.unwrap_or_default(),
                thread_ids: thread_ids.clone(),
                idempotency_key,
            })
            .await
            .map_err(|error| project_store_error(operation, error))?;
        Ok((api_project(created.project)?, created.created, thread_ids))
    }

    async fn notify_project_changed(&self, project_id: &str, change_type: ProjectChangeType) {
        self.outgoing
            .send_server_notification(ServerNotification::ProjectChanged(
                ProjectChangedNotification {
                    project_id: project_id.to_string(),
                    change_type,
                },
            ))
            .await;
    }

    async fn notify_thread_projects(&self, thread_ids: Vec<String>, project_id: Option<String>) {
        for thread_id in thread_ids {
            self.outgoing
                .send_server_notification(ServerNotification::ThreadProjectUpdated(
                    ThreadProjectUpdatedNotification {
                        thread_id,
                        project_id: project_id.clone(),
                    },
                ))
                .await;
        }
    }

    async fn acquire_thread_list_state_permit(
        &self,
    ) -> Result<SemaphorePermit<'_>, JSONRPCErrorError> {
        self.thread_list_state_permit
            .acquire()
            .await
            .map_err(|err| {
                internal_error(format!("failed to acquire thread list state permit: {err}"))
            })
    }
}

fn validate_name(name: String) -> Result<String, JSONRPCErrorError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(invalid_params("project name must not be empty"));
    }
    Ok(name)
}

fn validate_idempotency_key(key: String) -> Result<String, JSONRPCErrorError> {
    if key.trim().is_empty() {
        return Err(invalid_params("idempotencyKey must not be empty"));
    }
    if key.len() > 512 {
        return Err(invalid_params("idempotencyKey must be at most 512 bytes"));
    }
    Ok(key)
}

fn validate_roots(roots: Vec<ProjectRoot>) -> Result<Vec<StoredProjectRoot>, JSONRPCErrorError> {
    let mut logical = HashSet::new();
    let mut canonical = HashSet::new();
    roots
        .into_iter()
        .map(|root| {
            let path = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path_checked(
                root.path.into_path_buf(),
            )
            .map_err(|error| invalid_params(format!("invalid project root: {error}")))?
            .into_path_buf();
            if !logical.insert(path.clone()) {
                return Err(invalid_params(format!(
                    "duplicate project root: {}",
                    path.display()
                )));
            }
            if let Ok(resolved) = std::fs::canonicalize(&path)
                && !canonical.insert(resolved)
            {
                return Err(invalid_params(format!(
                    "duplicate resolved project root: {}",
                    path.display()
                )));
            }
            Ok(StoredProjectRoot {
                path: path.display().to_string(),
            })
        })
        .collect()
}

fn validate_thread_ids(thread_ids: Vec<String>) -> Result<Vec<String>, JSONRPCErrorError> {
    let mut seen = HashSet::new();
    for thread_id in &thread_ids {
        if !seen.insert(thread_id.clone()) {
            return Err(invalid_params(format!("duplicate thread id: {thread_id}")));
        }
    }
    Ok(thread_ids)
}

fn api_project(project: StoredProject) -> Result<Project, JSONRPCErrorError> {
    Ok(Project {
        id: project.id,
        name: project.name,
        roots: project
            .roots
            .into_iter()
            .map(|root| {
                Ok(ProjectRoot {
                    path: codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(root.path)
                        .map_err(|error| {
                            internal_error(format!("stored project root is not absolute: {error}"))
                        })?,
                })
            })
            .collect::<Result<_, JSONRPCErrorError>>()?,
        metadata: project.metadata,
        position: project.position,
        created_at: project.created_at_ms / 1000,
        updated_at: project.updated_at_ms / 1000,
    })
}

fn project_store_error(operation: &'static str, error: ThreadStoreError) -> JSONRPCErrorError {
    match error {
        ThreadStoreError::Unsupported { .. } => {
            method_not_found(format!("{operation} is unavailable without sqlite state"))
        }
        ThreadStoreError::InvalidRequest { message } => invalid_params(message),
        ThreadStoreError::Internal { message }
            if message.contains("thread not found") || message.contains("project not found") =>
        {
            invalid_params(message)
        }
        error => internal_error(format!("failed to run {operation}: {error}")),
    }
}
