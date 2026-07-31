use super::LocalThreadStore;
use crate::CreateThreadSectionParams;
use crate::DeleteThreadSectionParams;
use crate::ListThreadSectionsParams;
use crate::RenameThreadSectionParams;
use crate::StoredThreadSection;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::local::test_support::test_config;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn local_thread_sections_require_sqlite_and_preserve_section_identity() {
    let home = tempfile::TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let unsupported_store = LocalThreadStore::new(config.clone(), /*state_db*/ None);
    assert!(!unsupported_store.supports_thread_sections());
    assert!(matches!(
        unsupported_store
            .create_thread_section(CreateThreadSectionParams {
                name: "Research".to_string(),
            })
            .await,
        Err(ThreadStoreError::Unsupported {
            operation: "threadSection/create"
        })
    ));

    let runtime = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    let store = LocalThreadStore::new(config, Some(runtime));
    assert!(store.supports_thread_sections());

    let section = store
        .create_thread_section(CreateThreadSectionParams {
            name: "Research".to_string(),
        })
        .await
        .expect("create section");
    let sections = store
        .list_thread_sections(ListThreadSectionsParams {
            cursor: None,
            limit: 10,
        })
        .await
        .expect("list sections");
    assert!(sections.sections.contains(&section));

    let renamed = store
        .rename_thread_section(RenameThreadSectionParams {
            section_id: section.id.clone(),
            name: "Planning".to_string(),
        })
        .await
        .expect("rename section");
    assert_eq!(
        renamed,
        Some(StoredThreadSection {
            id: section.id.clone(),
            name: "Planning".to_string(),
        })
    );

    assert!(
        store
            .delete_thread_section(DeleteThreadSectionParams {
                section_id: section.id,
            })
            .await
            .expect("delete section")
    );
}
