use super::LocalThreadStore;
use crate::CreateProjectParams;
use crate::CreatedProject;
use crate::DeletedProject;
use crate::ListProjectsParams;
use crate::MoveProjectParams;
use crate::ProjectMoveOutcome;
use crate::StoredProject;
use crate::StoredProjectRoot;
use crate::StoredProjectsPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateProjectParams;
use crate::UpdatedProject;

pub(super) async fn list_projects(
    store: &LocalThreadStore,
    params: ListProjectsParams,
) -> ThreadStoreResult<StoredProjectsPage> {
    let state = state(store)?;
    let has_cursor = params.cursor.is_some();
    let page = state
        .list_projects(params.cursor.as_deref(), params.limit)
        .await
        .map_err(|error| {
            if has_cursor && error.to_string().starts_with("invalid project cursor:") {
                ThreadStoreError::InvalidRequest {
                    message: error.to_string(),
                }
            } else {
                internal(error)
            }
        })?;
    Ok(StoredProjectsPage {
        projects: page.projects.into_iter().map(stored_project).collect(),
        next_cursor: page.next_cursor,
    })
}

pub(super) async fn read_project(
    store: &LocalThreadStore,
    project_id: String,
) -> ThreadStoreResult<Option<StoredProject>> {
    state(store)?
        .get_project(&project_id)
        .await
        .map(|project| project.map(stored_project))
        .map_err(internal)
}

pub(super) async fn create_project(
    store: &LocalThreadStore,
    params: CreateProjectParams,
) -> ThreadStoreResult<CreatedProject> {
    state(store)?
        .create_project(
            params.name,
            params.roots.into_iter().map(state_root).collect(),
            params.metadata,
            &params.thread_ids,
            &params.idempotency_key,
        )
        .await
        .map(|created| CreatedProject {
            project: stored_project(created.project),
            created: created.created,
        })
        .map_err(project_idempotency_error)
}

pub(super) async fn update_project(
    store: &LocalThreadStore,
    params: UpdateProjectParams,
) -> ThreadStoreResult<Option<UpdatedProject>> {
    state(store)?
        .update_project(
            &params.project_id,
            params.name,
            params
                .roots
                .map(|roots| roots.into_iter().map(state_root).collect()),
            params.metadata,
        )
        .await
        .map(|result| {
            result.map(|(project, changed)| UpdatedProject {
                project: stored_project(project),
                changed,
            })
        })
        .map_err(internal)
}

pub(super) async fn move_project(
    store: &LocalThreadStore,
    params: MoveProjectParams,
) -> ThreadStoreResult<Option<ProjectMoveOutcome>> {
    state(store)?
        .move_project(&params.project_id, params.before_project_id.as_deref())
        .await
        .map(|result| {
            result.map(|changed| {
                if changed {
                    ProjectMoveOutcome::Moved
                } else {
                    ProjectMoveOutcome::Unchanged
                }
            })
        })
        .map_err(|error| {
            let message = error.to_string();
            if message.starts_with("before project ")
                || message.starts_with("project ") && message.contains("cannot be moved")
            {
                ThreadStoreError::InvalidRequest { message }
            } else {
                internal(message)
            }
        })
}

pub(super) async fn delete_project(
    store: &LocalThreadStore,
    project_id: String,
) -> ThreadStoreResult<Option<DeletedProject>> {
    state(store)?
        .delete_project(&project_id)
        .await
        .map(|result| {
            result.map(
                |(affected_active_thread_ids, affected_archived_thread_ids)| DeletedProject {
                    affected_active_thread_ids,
                    affected_archived_thread_ids,
                },
            )
        })
        .map_err(internal)
}

fn state(store: &LocalThreadStore) -> ThreadStoreResult<&codex_rollout::StateDbHandle> {
    store
        .state_db
        .as_ref()
        .ok_or(ThreadStoreError::Unsupported {
            operation: "projects",
        })
}

fn internal(error: impl std::fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: error.to_string(),
    }
}

fn project_idempotency_error(error: impl std::fmt::Display) -> ThreadStoreError {
    let message = error.to_string();
    if message.starts_with("idempotency key refers to deleted project:") {
        ThreadStoreError::InvalidRequest { message }
    } else {
        internal(message)
    }
}

fn state_root(root: StoredProjectRoot) -> codex_state::ProjectRoot {
    codex_state::ProjectRoot { path: root.path }
}

fn stored_project(project: codex_state::Project) -> StoredProject {
    StoredProject {
        id: project.id,
        name: project.name,
        roots: project
            .roots
            .into_iter()
            .map(|root| StoredProjectRoot { path: root.path })
            .collect(),
        metadata: project.metadata,
        position: project.position,
        created_at_ms: project.created_at_ms,
        updated_at_ms: project.updated_at_ms,
    }
}
