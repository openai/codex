//! Coverage for attachment identity, capacity, pagination, and durable lifecycle behavior.

use super::StateRuntime;
use crate::AddThreadAttachmentOutcome;
use crate::MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES;
use crate::MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES;
use crate::MAX_THREAD_ATTACHMENT_TYPE_BYTES;
use crate::MAX_THREAD_ATTACHMENTS_PER_THREAD;
use crate::RemoveThreadAttachmentOutcome;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;
use anyhow::Result;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

async fn runtime_with_threads(count: usize) -> Result<(Arc<StateRuntime>, PathBuf, Vec<ThreadId>)> {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let mut thread_ids = Vec::with_capacity(count);
    for _ in 0..count {
        let thread_id = ThreadId::new();
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await?;
        thread_ids.push(thread_id);
    }
    Ok((runtime, codex_home, thread_ids))
}

#[tokio::test]
async fn copying_attachments_is_atomic_and_independent() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 2).await?;
    let source = thread_ids[0];
    let destination = thread_ids[1];
    for key in ["first", "second"] {
        runtime
            .add_thread_attachment(source, "document", key, &json!({"path": key}))
            .await?;
    }
    sqlx::query(
        "CREATE TRIGGER fail_attachment_copy BEFORE INSERT ON thread_attachments WHEN NEW.identity_key = 'second' BEGIN SELECT RAISE(FAIL, 'copy failure'); END",
    )
    .execute(runtime.pool.as_ref())
    .await?;
    let error = runtime
        .copy_thread_attachments(source, destination)
        .await
        .expect_err("a failed insert must roll back the entire copy");
    assert!(error.to_string().contains("copy failure"));
    assert!(
        runtime
            .list_thread_attachments(destination, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments
            .is_empty()
    );
    sqlx::query("DROP TRIGGER fail_attachment_copy")
        .execute(runtime.pool.as_ref())
        .await?;
    runtime.copy_thread_attachments(source, destination).await?;
    let originals = runtime
        .list_thread_attachments(source, /*cursor*/ None, /*limit*/ 10)
        .await?
        .attachments;
    let copied = runtime
        .list_thread_attachments(destination, /*cursor*/ None, /*limit*/ 10)
        .await?
        .attachments;
    assert_eq!(copied.len(), originals.len());
    for (original, copy) in originals.iter().zip(&copied) {
        assert_ne!(copy.id, original.id);
        assert_eq!(
            copy,
            &crate::ThreadAttachment {
                id: copy.id.clone(),
                thread_id: destination,
                created_at: copy.created_at,
                ..original.clone()
            }
        );
    }
    runtime
        .remove_thread_attachment(source, "document", "first")
        .await?;
    runtime.delete_thread(source).await?;
    assert_eq!(
        runtime
            .list_thread_attachments(destination, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments,
        copied
    );
    Ok(())
}

#[tokio::test]
async fn attachment_attachments_are_idempotent_and_scoped_to_their_thread() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 2).await?;
    let first = runtime
        .add_thread_attachment(
            thread_ids[0],
            "pull_request",
            "openai/codex#123",
            &json!({ "url": "https://github.com/openai/codex/pull/123" }),
        )
        .await?;
    let AddThreadAttachmentOutcome::Created(first) = first else {
        anyhow::bail!("first attachment should create an attachment");
    };
    assert_eq!(first.thread_id, thread_ids[0]);

    let repeated = runtime
        .add_thread_attachment(
            thread_ids[0],
            "pull_request",
            "openai/codex#123",
            &json!({ "url": "https://github.com/openai/codex/pull/456" }),
        )
        .await?;
    assert_eq!(
        repeated,
        AddThreadAttachmentOutcome::Existing(first.clone())
    );

    let other = runtime
        .add_thread_attachment(
            thread_ids[1],
            "pull_request",
            "openai/codex#123",
            &json!({ "url": "https://github.com/openai/codex/pull/123" }),
        )
        .await?;
    let AddThreadAttachmentOutcome::Created(other) = other else {
        anyhow::bail!("the same identity on another thread should create its own attachment");
    };
    assert_ne!(first.id, other.id);
    assert_eq!(
        runtime
            .list_thread_attachments(thread_ids[0], /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments,
        vec![first]
    );
    Ok(())
}

#[tokio::test]
async fn attachment_removals_return_not_found_or_the_removed_record() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 1).await?;
    let thread_id = thread_ids[0];
    let removed_before_attach = runtime
        .remove_thread_attachment(thread_id, "pull_request", "openai/codex#123")
        .await?;
    assert_eq!(
        removed_before_attach,
        RemoveThreadAttachmentOutcome::NotFound
    );

    let explicit = runtime
        .add_thread_attachment(
            thread_id,
            "pull_request",
            "openai/codex#123",
            &json!({ "url": "https://github.com/openai/codex/pull/123" }),
        )
        .await?;
    let AddThreadAttachmentOutcome::Created(explicit) = explicit else {
        anyhow::bail!("attachment should create an attachment");
    };
    assert_eq!(
        runtime
            .remove_thread_attachment(thread_id, "pull_request", "openai/codex#123")
            .await?,
        RemoveThreadAttachmentOutcome::Removed(explicit)
    );
    assert!(
        runtime
            .list_thread_attachments(thread_id, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn attachment_listing_scopes_pagination_to_one_thread() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 2).await?;
    let mut expected = Vec::new();
    for (thread_id, identity_key) in [
        (thread_ids[0], "first"),
        (thread_ids[0], "second"),
        (thread_ids[0], "third"),
        (thread_ids[1], "excluded"),
    ] {
        let created = runtime
            .add_thread_attachment(
                thread_id,
                "pull_request",
                identity_key,
                &json!({ "identity": identity_key }),
            )
            .await?;
        if thread_id == thread_ids[0] {
            let AddThreadAttachmentOutcome::Created(attachment) = created else {
                anyhow::bail!("new identities should create attachments");
            };
            expected.push(attachment);
        }
    }

    let first_page = runtime
        .list_thread_attachments(thread_ids[0], /*cursor*/ None, /*limit*/ 2)
        .await?;
    assert_eq!(first_page.attachments, expected[..2]);
    let other_thread_error = runtime
        .list_thread_attachments(
            thread_ids[1],
            first_page.next_cursor.as_deref(),
            /*limit*/ 2,
        )
        .await
        .expect_err("a cursor from another thread must be rejected");
    assert!(
        other_thread_error
            .to_string()
            .contains("invalid pagination cursor")
    );
    let second_page = runtime
        .list_thread_attachments(
            thread_ids[0],
            first_page.next_cursor.as_deref(),
            /*limit*/ 2,
        )
        .await?;
    assert_eq!(second_page.attachments, expected[2..]);
    assert_eq!(second_page.next_cursor, None);
    Ok(())
}

#[tokio::test]
async fn active_attachment_limit_is_freed_by_removal() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 1).await?;
    let thread_id = thread_ids[0];

    for index in 0..MAX_THREAD_ATTACHMENTS_PER_THREAD {
        runtime
            .add_thread_attachment(
                thread_id,
                "pull_request",
                &format!("attachment-{index}"),
                &json!({ "index": index }),
            )
            .await?;
    }

    let existing = runtime
        .add_thread_attachment(
            thread_id,
            "pull_request",
            "attachment-0",
            &json!({ "index": "different" }),
        )
        .await?;
    assert!(matches!(existing, AddThreadAttachmentOutcome::Existing(_)));
    let attachment_limit = runtime
        .add_thread_attachment(thread_id, "pull_request", "one-too-many", &json!({}))
        .await
        .expect_err("a new attachment must not exceed the per-thread limit");
    assert!(
        attachment_limit.to_string().contains(
            "invalid thread attachment request: thread attachment identity count exceeds"
        )
    );

    let removed = runtime
        .remove_thread_attachment(thread_id, "pull_request", "attachment-0")
        .await?;
    assert!(matches!(removed, RemoveThreadAttachmentOutcome::Removed(_)));
    assert_eq!(
        runtime
            .remove_thread_attachment(thread_id, "pull_request", "attachment-0")
            .await?,
        RemoveThreadAttachmentOutcome::NotFound
    );
    let replacement = runtime
        .add_thread_attachment(thread_id, "pull_request", "one-too-many", &json!({}))
        .await?;
    assert!(matches!(
        replacement,
        AddThreadAttachmentOutcome::Created(_)
    ));

    Ok(())
}

#[tokio::test]
async fn attachments_survive_restart_and_archive_but_cascade_on_thread_deletion() -> Result<()> {
    let (runtime, codex_home, thread_ids) = runtime_with_threads(/*count*/ 1).await?;
    let thread_id = thread_ids[0];
    let AddThreadAttachmentOutcome::Created(attachment) = runtime
        .add_thread_attachment(
            thread_id,
            "pull_request",
            "openai/codex#123",
            &json!({ "url": "https://github.com/openai/codex/pull/123" }),
        )
        .await?
    else {
        anyhow::bail!("first attachment should create an attachment");
    };

    let reopened = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    assert_eq!(
        reopened
            .list_thread_attachments(thread_id, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments,
        vec![attachment.clone()]
    );

    let rollout_path = codex_home.join("archived.jsonl");
    reopened
        .mark_archived(thread_id, &rollout_path, Utc::now())
        .await?;
    reopened.mark_unarchived(thread_id, &rollout_path).await?;
    assert_eq!(
        reopened
            .list_thread_attachments(thread_id, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments,
        vec![attachment]
    );

    assert_eq!(reopened.delete_thread(thread_id).await?, 1);
    assert!(
        reopened
            .list_thread_attachments(thread_id, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn attachment_requests_reject_invalid_identity_payload_and_cursor() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 1).await?;
    let thread_id = thread_ids[0];
    for (attachment_type, identity_key, expected) in [
        (" ".to_string(), "pr".to_string(), "type must not be empty"),
        (
            "x".repeat(MAX_THREAD_ATTACHMENT_TYPE_BYTES + 1),
            "pr".to_string(),
            "type exceeds",
        ),
        (
            "pull_request".to_string(),
            " ".to_string(),
            "key must not be empty",
        ),
        (
            "pull_request".to_string(),
            "x".repeat(MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES + 1),
            "key exceeds",
        ),
    ] {
        let error = runtime
            .add_thread_attachment(thread_id, &attachment_type, &identity_key, &json!({}))
            .await
            .expect_err("invalid attachment identities must be rejected");
        assert!(error.to_string().contains(expected));
    }

    let too_large = runtime
        .add_thread_attachment(
            thread_id,
            "pull_request",
            "pr",
            &json!("x".repeat(MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES)),
        )
        .await
        .expect_err("oversized attachment payload must be rejected");
    assert!(too_large.to_string().contains("payload exceeds"));
    let invalid_cursor = runtime
        .list_thread_attachments(thread_id, Some("not-a-cursor"), /*limit*/ 10)
        .await
        .expect_err("malformed cursor must be rejected");
    assert!(
        invalid_cursor
            .to_string()
            .contains("invalid pagination cursor")
    );
    let missing = ThreadId::new();
    let missing_error = runtime
        .add_thread_attachment(missing, "pull_request", "pr", &json!({}))
        .await
        .expect_err("missing owners must be rejected");
    assert!(missing_error.to_string().contains("thread not found"));
    Ok(())
}

#[tokio::test]
async fn concurrent_attachment_attachments_preserve_one_deterministic_identity() -> Result<()> {
    let (runtime, _codex_home, thread_ids) = runtime_with_threads(/*count*/ 1).await?;
    let thread_id = thread_ids[0];
    let mut joins = Vec::new();
    for _ in 0..8 {
        let runtime = Arc::clone(&runtime);
        joins.push(tokio::spawn(async move {
            runtime
                .add_thread_attachment(
                    thread_id,
                    "pull_request",
                    "openai/codex#123",
                    &json!({ "url": "https://github.com/openai/codex/pull/123" }),
                )
                .await
        }));
    }

    let mut created = 0;
    for join in joins {
        match join.await?? {
            AddThreadAttachmentOutcome::Created(_) => created += 1,
            AddThreadAttachmentOutcome::Existing(_) => {}
        }
    }
    assert_eq!(created, 1);
    assert_eq!(
        runtime
            .list_thread_attachments(thread_id, /*cursor*/ None, /*limit*/ 10)
            .await?
            .attachments
            .len(),
        1
    );
    Ok(())
}
