use super::StateRuntime;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;
use anyhow::Result;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn deleting_custom_section_preserves_threads_and_clears_section_ordering() -> Result<()> {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let deleted_section = runtime
        .create_thread_section(
            "Research",
            Some(crate::ThreadSectionAppearance {
                icon: Some("folder".to_string()),
                color: Some("purple".to_string()),
            }),
        )
        .await?;
    let retained_section = runtime
        .create_thread_section("Planning", /*appearance*/ None)
        .await?;
    let first = ThreadId::new();
    let second = ThreadId::new();
    let retained = ThreadId::new();

    for (thread_id, section_id) in [
        (first, &deleted_section.id),
        (second, &deleted_section.id),
        (retained, &retained_section.id),
    ] {
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await?;
        runtime
            .move_thread_to_section(thread_id, Some(section_id), /*before_thread_id*/ None)
            .await?;
    }

    assert!(runtime.delete_thread_section(&deleted_section.id).await?);
    assert_eq!(runtime.get_thread_section(&deleted_section.id).await?, None);

    for thread_id in [first, second] {
        let thread = runtime
            .get_thread(thread_id)
            .await?
            .expect("section deletion must preserve its member threads");
        assert_eq!(
            (
                thread.section,
                thread.section_position,
                thread.section_entered_at
            ),
            (None, None, None)
        );
    }

    let retained_thread = runtime
        .get_thread(retained)
        .await?
        .expect("unrelated section member should remain");
    assert_eq!(retained_thread.section, Some(retained_section));
    assert!(retained_thread.section_position.is_some());
    assert!(retained_thread.section_entered_at.is_some());

    Ok(())
}

#[tokio::test]
async fn concurrent_section_deletion_and_membership_moves_preserve_thread_invariants() -> Result<()>
{
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let section = runtime
        .create_thread_section("Research", /*appearance*/ None)
        .await?;
    let thread_id = ThreadId::new();
    runtime
        .upsert_thread(&test_thread_metadata(
            &codex_home,
            thread_id,
            codex_home.clone(),
        ))
        .await?;

    let (deleted, moved) = tokio::join!(
        runtime.delete_thread_section(&section.id),
        runtime.move_thread_to_section(
            thread_id,
            Some(&section.id),
            /*before_thread_id*/ None
        ),
    );

    assert!(deleted?);
    if let Err(error) = moved {
        assert!(error.to_string().contains("does not exist"));
    }
    let thread = runtime
        .get_thread(thread_id)
        .await?
        .expect("concurrent section deletion must preserve the thread");
    assert_eq!(
        (
            thread.section,
            thread.section_position,
            thread.section_entered_at
        ),
        (None, None, None)
    );

    Ok(())
}
