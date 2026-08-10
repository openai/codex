use super::StateRuntime;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

const CUSTOM_THREAD_SECTION_ID: &str = "01984de2-8f74-7c91-a3b2-5c5e937cf317";
const OTHER_THREAD_SECTION_ID: &str = "01984de2-8f74-7c91-a3b2-5c5e937cf319";

#[tokio::test]
async fn thread_section_ordering_batches_persisted_positions_and_entry_times() -> Result<()> {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let first = ThreadId::new();
    let second = ThreadId::new();
    let unsectioned = ThreadId::new();
    let missing = ThreadId::new();

    for thread_id in [first, second, unsectioned] {
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await?;
    }

    runtime
        .move_thread_to_section(
            first,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        )
        .await?;
    runtime
        .move_thread_to_section(
            second,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        )
        .await?;

    let first_entered_at = runtime
        .get_thread(first)
        .await?
        .expect("first pinned thread should exist")
        .section_entered_at;
    let second_entered_at = runtime
        .get_thread(second)
        .await?
        .expect("second pinned thread should exist")
        .section_entered_at;

    assert_eq!(
        runtime
            .get_thread_section_ordering(&[first, second, unsectioned, missing, first])
            .await?,
        HashMap::from([
            (first, (Some(1_000_000), first_entered_at)),
            (second, (Some(2_000_000), second_entered_at)),
            (unsectioned, (None, None)),
        ])
    );
    assert_eq!(
        runtime.get_thread_section_ordering(&[]).await?,
        HashMap::new()
    );

    Ok(())
}

#[tokio::test]
async fn thread_sections_paginate_and_require_registered_identities() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await
    .expect("state db should initialize");
    let before_pinned = crate::ThreadSection {
        id: "01984de2-8f74-7c91-a3b2-5c5e937cf317".to_string(),
        name: "Before pinned".to_string(),
        appearance: None,
    };
    let pinned = crate::ThreadSection {
        id: crate::PINNED_THREAD_SECTION_ID.to_string(),
        name: crate::PINNED_THREAD_SECTION_NAME.to_string(),
        appearance: None,
    };
    let after_pinned = crate::ThreadSection {
        id: "01984de2-8f74-7c91-a3b2-5c5e937cf319".to_string(),
        name: "After pinned".to_string(),
        appearance: None,
    };

    for section in [&before_pinned, &after_pinned] {
        sqlx::query("INSERT INTO thread_sections (id, name) VALUES (?, ?)")
            .bind(&section.id)
            .bind(&section.name)
            .execute(runtime.pool.as_ref())
            .await
            .expect("custom test sections should be explicitly registered");
    }

    assert_eq!(
        runtime
            .get_thread_section(&pinned.id)
            .await
            .expect("built-in section should load"),
        Some(pinned.clone())
    );
    assert_eq!(
        runtime
            .get_thread_section("01984de2-8f74-7c91-a3b2-5c5e937cf320")
            .await
            .expect("missing section lookup should succeed"),
        None
    );

    assert_eq!(
        runtime
            .list_thread_sections(/*cursor*/ None, /*limit*/ 1)
            .await
            .expect("first section page should load"),
        crate::ThreadSectionsPage {
            sections: vec![before_pinned.clone()],
            next_cursor: Some(before_pinned.id.clone()),
        }
    );
    assert_eq!(
        runtime
            .list_thread_sections(Some(&before_pinned.id), /*limit*/ 1)
            .await
            .expect("pinned section page should load"),
        crate::ThreadSectionsPage {
            sections: vec![pinned.clone()],
            next_cursor: Some(pinned.id.clone()),
        }
    );
    assert_eq!(
        runtime
            .list_thread_sections(Some(&pinned.id), /*limit*/ 1)
            .await
            .expect("final section page should load"),
        crate::ThreadSectionsPage {
            sections: vec![after_pinned],
            next_cursor: None,
        }
    );

    let thread_id = ThreadId::new();
    let mut metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
    metadata.section = Some(before_pinned.clone());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("registered section should be accepted");
    assert_eq!(
        runtime
            .get_thread(thread_id)
            .await
            .expect("sectioned thread should load")
            .expect("sectioned thread should exist")
            .section,
        Some(before_pinned.clone())
    );
    assert!(
        runtime
            .move_thread_to_section(
                thread_id,
                /*section*/ Some("01984de2-8f74-7c91-a3b2-5c5e937cf320"),
                /*before_thread_id*/ None,
            )
            .await
            .is_err(),
        "thread sections must be explicitly registered before assignment"
    );
    assert_eq!(
        runtime
            .get_thread(thread_id)
            .await
            .expect("thread should survive rejected section assignment")
            .expect("thread should still exist")
            .section,
        Some(before_pinned)
    );
}

#[tokio::test]
async fn thread_section_moves_round_trip_and_survive_rollout_reconciliation() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await
    .expect("state db should initialize");
    let thread_id = ThreadId::new();
    let metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("thread insert should succeed");
    assert_eq!(
        runtime
            .get_thread(thread_id)
            .await
            .unwrap()
            .unwrap()
            .section,
        None
    );

    assert!(
        runtime
            .move_thread_to_section(
                thread_id,
                /*section*/ Some(crate::PINNED_THREAD_SECTION_ID),
                /*before_thread_id*/ None,
            )
            .await
            .unwrap()
    );
    let pinned = runtime.get_thread(thread_id).await.unwrap().unwrap();
    assert_eq!(
        (pinned.section.as_ref(), pinned.section_position),
        (
            Some(&crate::ThreadSection {
                id: crate::PINNED_THREAD_SECTION_ID.to_string(),
                name: crate::PINNED_THREAD_SECTION_NAME.to_string(),
                appearance: None,
            }),
            Some(1_000_000),
        )
    );
    assert!(pinned.section_entered_at.is_some());

    assert!(
        runtime
            .move_thread_to_section(
                thread_id,
                /*section*/ Some(crate::PINNED_THREAD_SECTION_ID),
                /*before_thread_id*/ None,
            )
            .await
            .unwrap()
    );
    assert_eq!(
        runtime.get_thread(thread_id).await.unwrap().unwrap(),
        pinned
    );

    runtime
        .upsert_thread(&metadata)
        .await
        .expect("stale rollout metadata should reconcile");
    let reconciled = runtime.get_thread(thread_id).await.unwrap().unwrap();
    assert_eq!(
        (
            reconciled.section,
            reconciled.section_position,
            reconciled.section_entered_at,
        ),
        (
            pinned.section,
            pinned.section_position,
            pinned.section_entered_at
        )
    );

    assert!(
        runtime
            .move_thread_to_section(
                thread_id, /*section*/ None, /*before_thread_id*/ None,
            )
            .await
            .unwrap()
    );
    let unpinned = runtime.get_thread(thread_id).await.unwrap().unwrap();
    assert_eq!(
        (
            unpinned.section,
            unpinned.section_position,
            unpinned.section_entered_at,
        ),
        (None, None, None)
    );
    assert!(
        !runtime
            .move_thread_to_section(
                ThreadId::new(),
                /*section*/ Some(crate::PINNED_THREAD_SECTION_ID),
                /*before_thread_id*/ None,
            )
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn concurrent_section_moves_preserve_unique_positions() -> Result<()> {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let [first, second, third, fourth, fifth] = [
        "00000000-0000-0000-0000-000000000071",
        "00000000-0000-0000-0000-000000000072",
        "00000000-0000-0000-0000-000000000073",
        "00000000-0000-0000-0000-000000000074",
        "00000000-0000-0000-0000-000000000075",
    ]
    .map(|thread_id| ThreadId::from_string(thread_id).expect("valid thread id"));

    for thread_id in [first, second, third, fourth, fifth] {
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await?;
    }

    let updated = tokio::try_join!(
        runtime.move_thread_to_section(
            first,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        ),
        runtime.move_thread_to_section(
            second,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        ),
        runtime.move_thread_to_section(
            third,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        ),
        runtime.move_thread_to_section(
            fourth,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        ),
    )?;
    assert_eq!(updated, (true, true, true, true));

    let mut entered_at = HashMap::new();
    for thread_id in [first, second, third, fourth] {
        let thread = runtime
            .get_thread(thread_id)
            .await?
            .expect("concurrently sectioned thread should exist");
        entered_at.insert(
            thread_id,
            thread
                .section_entered_at
                .expect("section entry time should be recorded"),
        );
    }
    let initial_positions = sqlx::query_scalar::<_, i64>(
        "SELECT section_position FROM threads WHERE thread_section_id = ? ORDER BY section_position, id",
    )
    .bind(crate::PINNED_THREAD_SECTION_ID)
    .fetch_all(runtime.pool.as_ref())
    .await?;
    assert_eq!(
        initial_positions,
        vec![1_000_000, 2_000_000, 3_000_000, 4_000_000]
    );

    let moved = tokio::try_join!(
        runtime.move_thread_to_section(fourth, Some(crate::PINNED_THREAD_SECTION_ID), Some(first)),
        runtime.move_thread_to_section(third, Some(crate::PINNED_THREAD_SECTION_ID), Some(first)),
    )?;
    assert_eq!(moved, (true, true));

    let updated_and_moved = tokio::try_join!(
        runtime.move_thread_to_section(
            fifth,
            Some(crate::PINNED_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        ),
        runtime.move_thread_to_section(first, Some(crate::PINNED_THREAD_SECTION_ID), Some(second)),
    )?;
    assert_eq!(updated_and_moved, (true, true));

    let ordered_threads = sqlx::query_as::<_, (String, i64)>(
        "SELECT id, section_position FROM threads WHERE thread_section_id = ? ORDER BY section_position, id",
    )
    .bind(crate::PINNED_THREAD_SECTION_ID)
    .fetch_all(runtime.pool.as_ref())
    .await?;
    assert_eq!(ordered_threads.len(), 5);
    assert!(
        ordered_threads
            .windows(2)
            .all(|threads| threads[0].1 < threads[1].1),
        "concurrent section mutations must preserve unique ordered positions: {ordered_threads:?}"
    );
    assert_eq!(
        ordered_threads.last().map(|(thread_id, _)| thread_id),
        Some(&fifth.to_string())
    );

    for (thread_id, original_entered_at) in entered_at {
        assert_eq!(
            runtime
                .get_thread(thread_id)
                .await?
                .expect("moved thread should remain sectioned")
                .section_entered_at,
            Some(original_entered_at)
        );
    }

    Ok(())
}

#[tokio::test]
async fn section_moves_preserve_entry_order_and_renumber_exhausted_ranks() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await
    .expect("state db should initialize");
    for (section_id, section_name) in [
        (CUSTOM_THREAD_SECTION_ID, "Custom section"),
        (OTHER_THREAD_SECTION_ID, "Other section"),
    ] {
        sqlx::query("INSERT INTO thread_sections (id, name) VALUES (?, ?)")
            .bind(section_id)
            .bind(section_name)
            .execute(runtime.pool.as_ref())
            .await
            .expect("custom test sections should be explicitly registered");
    }
    let first = ThreadId::from_string("00000000-0000-0000-0000-000000000051").unwrap();
    let second = ThreadId::from_string("00000000-0000-0000-0000-000000000052").unwrap();
    let third = ThreadId::from_string("00000000-0000-0000-0000-000000000053").unwrap();

    for thread_id in [first, second, third] {
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await
            .unwrap();
        assert!(
            runtime
                .move_thread_to_section(
                    thread_id,
                    /*section*/ Some(CUSTOM_THREAD_SECTION_ID),
                    /*before_thread_id*/ None,
                )
                .await
                .unwrap()
        );
    }

    let mut initial = Vec::new();
    for thread_id in [first, second, third] {
        initial.push(runtime.get_thread(thread_id).await.unwrap().unwrap());
    }
    assert_eq!(
        initial
            .iter()
            .map(|thread| thread.section_position)
            .collect::<Vec<_>>(),
        vec![Some(1_000_000), Some(2_000_000), Some(3_000_000)]
    );
    assert!(
        initial
            .iter()
            .all(|thread| thread.section_entered_at.is_some())
    );
    let original_entered_at = initial[2].section_entered_at;

    runtime
        .touch_thread_recency_at(
            first,
            DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap(),
        )
        .await
        .unwrap();
    runtime
        .move_thread_to_section(third, Some(CUSTOM_THREAD_SECTION_ID), Some(second))
        .await
        .unwrap();
    let moved = runtime.get_thread(third).await.unwrap().unwrap();
    assert_eq!(moved.section_position, Some(1_500_000));
    assert_eq!(moved.section_entered_at, original_entered_at);

    runtime
        .move_thread_to_section(
            third,
            Some(CUSTOM_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .get_thread(third)
            .await
            .unwrap()
            .unwrap()
            .section_position,
        Some(3_000_000)
    );

    for (thread_id, position) in [(first, 1_i64), (second, 2), (third, 3)] {
        sqlx::query("UPDATE threads SET section_position = ? WHERE id = ?")
            .bind(position)
            .bind(thread_id.to_string())
            .execute(runtime.pool.as_ref())
            .await
            .unwrap();
    }
    runtime
        .move_thread_to_section(third, Some(CUSTOM_THREAD_SECTION_ID), Some(second))
        .await
        .unwrap();
    let reordered = sqlx::query_scalar::<_, String>(
        "SELECT id FROM threads WHERE thread_section_id = ? ORDER BY section_position, id",
    )
    .bind(CUSTOM_THREAD_SECTION_ID)
    .fetch_all(runtime.pool.as_ref())
    .await
    .unwrap();
    assert_eq!(
        reordered,
        vec![first.to_string(), third.to_string(), second.to_string()]
    );
    assert_eq!(
        runtime
            .get_thread(third)
            .await
            .unwrap()
            .unwrap()
            .section_position,
        Some(1_500_000)
    );

    let unknown_section = "01984de2-8f74-7c91-a3b2-5c5e937cf320";
    let error = runtime
        .move_thread_to_section(third, Some(unknown_section), /*before_thread_id*/ None)
        .await
        .expect_err("unregistered destination sections should be rejected");
    assert_eq!(
        error.to_string(),
        format!("section {unknown_section} does not exist")
    );

    runtime
        .move_thread_to_section(
            second,
            /*section*/ Some(OTHER_THREAD_SECTION_ID),
            /*before_thread_id*/ None,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE threads SET section_entered_at_ms = ? WHERE id = ?")
        .bind(1_i64)
        .bind(third.to_string())
        .execute(runtime.pool.as_ref())
        .await
        .unwrap();
    runtime
        .move_thread_to_section(third, Some(OTHER_THREAD_SECTION_ID), Some(second))
        .await
        .unwrap();
    let moved_across_sections = runtime.get_thread(third).await.unwrap().unwrap();
    assert_eq!(
        moved_across_sections.section,
        Some(crate::ThreadSection {
            id: OTHER_THREAD_SECTION_ID.to_string(),
            name: "Other section".to_string(),
            appearance: None,
        })
    );
    assert_eq!(moved_across_sections.section_position, Some(500_000));
    assert!(
        moved_across_sections
            .section_entered_at
            .is_some_and(|entered_at| entered_at.timestamp_millis() > 1)
    );
    let error = runtime
        .move_thread_to_section(third, /*section*/ None, Some(second))
        .await
        .expect_err("clearing a section cannot accept a before-thread anchor");
    assert_eq!(
        error.to_string(),
        "before thread cannot be specified without a section"
    );

    runtime
        .move_thread_to_section(third, /*section*/ None, /*before_thread_id*/ None)
        .await
        .unwrap();
    let cleared = runtime.get_thread(third).await.unwrap().unwrap();
    assert_eq!(
        (
            cleared.section,
            cleared.section_position,
            cleared.section_entered_at
        ),
        (None, None, None)
    );
}
