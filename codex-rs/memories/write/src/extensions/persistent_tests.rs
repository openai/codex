use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn resource(
    source_id: &str,
    target_file_name: &str,
    content_sha256: &str,
    content: &str,
) -> PersistentMemoryExtensionResource {
    PersistentMemoryExtensionResource {
        source_id: source_id.to_string(),
        target_file_name: target_file_name.to_string(),
        content_sha256: content_sha256.to_string(),
        content: content.to_string(),
    }
}

#[tokio::test]
async fn sync_updates_changed_resources_removes_stale_managed_files_and_preserves_unrelated_files()
{
    let root = TempDir::new().expect("create tempdir");
    let memory_root = root.path();
    let extension_root = crate::memory_extensions_root(memory_root).join("external-import");
    let resources_root = extension_root.join("resources");
    let initial = vec![
        resource("project/index", "index-a.md", "hash-a", "first index"),
        resource("project/topic", "topic-b.md", "hash-b", "first topic"),
    ];

    let first_outcome = sync_persistent_extension_resources(
        memory_root,
        "external-import",
        "instructions v1",
        &initial,
    )
    .await
    .expect("initial sync");
    assert_eq!(first_outcome.written.len(), 2);
    assert_eq!(first_outcome.removed, Vec::<PathBuf>::new());
    assert_eq!(first_outcome.unchanged, 0);
    tokio::fs::write(resources_root.join("unrelated.md"), "keep me")
        .await
        .expect("write unrelated resource");

    assert!(
        !persistent_extension_needs_sync(
            memory_root,
            "external-import",
            "instructions v1",
            &initial,
        )
        .await
        .expect("check current sync state")
    );

    let updated = vec![resource(
        "project/index",
        "index-a.md",
        "hash-a2",
        "updated index",
    )];
    assert!(
        persistent_extension_needs_sync(
            memory_root,
            "external-import",
            "instructions v2",
            &updated,
        )
        .await
        .expect("check changed sync state")
    );

    let second_outcome = sync_persistent_extension_resources(
        memory_root,
        "external-import",
        "instructions v2",
        &updated,
    )
    .await
    .expect("updated sync");
    assert_eq!(
        second_outcome.written,
        vec![resources_root.join("index-a.md")]
    );
    assert_eq!(
        second_outcome.removed,
        vec![resources_root.join("topic-b.md")]
    );
    assert_eq!(second_outcome.unchanged, 0);
    assert_eq!(
        tokio::fs::read_to_string(resources_root.join("unrelated.md"))
            .await
            .expect("read unrelated resource"),
        "keep me"
    );
    assert_eq!(
        tokio::fs::read_to_string(extension_root.join("instructions.md"))
            .await
            .expect("read updated instructions"),
        "instructions v2"
    );
}
