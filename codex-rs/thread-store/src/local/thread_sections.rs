use super::LocalThreadStore;
use crate::CreateThreadSectionParams;
use crate::DeleteThreadSectionParams;
use crate::ListThreadSectionsParams;
use crate::RenameThreadSectionParams;
use crate::StoredThreadSection;
use crate::StoredThreadSectionsPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use codex_rollout::StateDbHandle;

pub(super) async fn list_thread_sections(
    store: &LocalThreadStore,
    params: ListThreadSectionsParams,
) -> ThreadStoreResult<StoredThreadSectionsPage> {
    let state_db = state_db(store, "threadSection/list")?;
    let page = state_db
        .list_thread_sections(params.cursor.as_deref(), params.limit)
        .await
        .map_err(|err| section_error("list", err))?;

    Ok(StoredThreadSectionsPage {
        sections: page
            .sections
            .into_iter()
            .map(|section| StoredThreadSection {
                id: section.id,
                name: section.name,
            })
            .collect(),
        next_cursor: page.next_cursor,
    })
}

pub(super) async fn create_thread_section(
    store: &LocalThreadStore,
    params: CreateThreadSectionParams,
) -> ThreadStoreResult<StoredThreadSection> {
    let state_db = state_db(store, "threadSection/create")?;
    let section = state_db
        .create_thread_section(&params.name)
        .await
        .map_err(|err| section_error("create", err))?;

    Ok(StoredThreadSection {
        id: section.id,
        name: section.name,
    })
}

pub(super) async fn rename_thread_section(
    store: &LocalThreadStore,
    params: RenameThreadSectionParams,
) -> ThreadStoreResult<Option<StoredThreadSection>> {
    let state_db = state_db(store, "threadSection/update")?;
    let section = state_db
        .rename_thread_section(&params.section_id, &params.name)
        .await
        .map_err(|err| section_error("update", err))?;

    Ok(section.map(|section| StoredThreadSection {
        id: section.id,
        name: section.name,
    }))
}

pub(super) async fn delete_thread_section(
    store: &LocalThreadStore,
    params: DeleteThreadSectionParams,
) -> ThreadStoreResult<bool> {
    let state_db = state_db(store, "threadSection/delete")?;
    state_db
        .delete_thread_section(&params.section_id)
        .await
        .map_err(|err| section_error("delete", err))
}

fn state_db<'store>(
    store: &'store LocalThreadStore,
    operation: &'static str,
) -> ThreadStoreResult<&'store StateDbHandle> {
    store
        .state_db
        .as_ref()
        .ok_or(ThreadStoreError::Unsupported { operation })
}

fn section_error(operation: &str, error: impl std::fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("failed to {operation} thread section: {error}"),
    }
}

#[cfg(test)]
#[path = "thread_sections_tests.rs"]
mod tests;
