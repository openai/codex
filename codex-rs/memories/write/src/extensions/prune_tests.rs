use super::*;
use crate::memory_extensions_root;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test]
async fn prunes_only_old_resources_from_extensions_with_instructions() {
    let codex_home = TempDir::new().expect("create temp codex home");
    let memory_root = codex_home.path().join("memories");
    let extensions_root = memory_extensions_root(&memory_root);
    let chronicle_resources = extensions_root.join("chronicle/resources");
    tokio::fs::create_dir_all(&chronicle_resources)
        .await
        .expect("create chronicle resources");
    tokio::fs::write(
        extensions_root.join("chronicle/instructions.md"),
        "instructions",
    )
    .await
    .expect("write chronicle instructions");

    let now = DateTime::from_naive_utc_and_offset(
        NaiveDateTime::parse_from_str(
            "2026-04-14T12-00-00",
            crate::extension_resources::FILENAME_TS_FORMAT,
        )
        .expect("parse now"),
        Utc,
    );
    let old_file = chronicle_resources.join("2026-04-06T11-59-59-abcd-10min-old.md");
    let exact_cutoff_file = chronicle_resources.join("2026-04-07T12-00-00-abcd-10min-cutoff.md");
    let recent_file = chronicle_resources.join("2026-04-08T12-00-00-abcd-10min-recent.md");
    let invalid_file = chronicle_resources.join("not-a-timestamp.md");
    for file in [&old_file, &exact_cutoff_file, &recent_file, &invalid_file] {
        tokio::fs::write(file, "resource")
            .await
            .expect("write chronicle resource");
    }

    let ignored_resources = extensions_root.join("ignored/resources");
    tokio::fs::create_dir_all(&ignored_resources)
        .await
        .expect("create ignored resources");
    let ignored_old_file = ignored_resources.join("2026-04-06T11-59-59-abcd-10min-old.md");
    tokio::fs::write(&ignored_old_file, "ignored")
        .await
        .expect("write ignored resource");

    prune_old_extension_resources_with_now(&memory_root, now).await;

    assert!(
        !tokio::fs::try_exists(&old_file)
            .await
            .expect("check old file")
    );
    assert!(
        !tokio::fs::try_exists(&exact_cutoff_file)
            .await
            .expect("check cutoff file")
    );
    assert!(
        tokio::fs::try_exists(&recent_file)
            .await
            .expect("check recent file")
    );
    assert!(
        tokio::fs::try_exists(&invalid_file)
            .await
            .expect("check invalid file")
    );
    assert!(
        tokio::fs::try_exists(&ignored_old_file)
            .await
            .expect("check ignored file")
    );
}

#[test]
fn parses_timestamp_prefix_from_resource_file_name() {
    let parsed = resource_timestamp("2026-04-06T11-59-59-abcd-10min-old.md")
        .expect("timestamp should parse");

    assert_eq!(parsed.timestamp(), 1_775_476_799);
    assert!(resource_timestamp("not-a-timestamp.md").is_none());
}
