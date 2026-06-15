use super::*;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;

async fn runtime_with_thread() -> (Arc<StateRuntime>, ThreadId) {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime");
    let thread_id = ThreadId::new();
    runtime
        .upsert_thread(&test_thread_metadata(
            codex_home.as_path(),
            thread_id,
            codex_home.clone(),
        ))
        .await
        .expect("insert thread");
    (runtime, thread_id)
}

#[tokio::test]
async fn claim_next_has_one_winner_across_runtime_instances() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let second_runtime = StateRuntime::init(
        runtime.codex_home().to_path_buf(),
        "test-provider".to_string(),
    )
    .await
    .expect("second state runtime");
    runtime
        .thread_queue()
        .enqueue(thread_id, br#"{"input":[]}"#)
        .await
        .expect("enqueue");

    let (first, second) = tokio::join!(
        runtime.thread_queue().claim_next(thread_id),
        second_runtime.thread_queue().claim_next(thread_id),
    );
    assert_eq!(
        1,
        [first.expect("first claim"), second.expect("second claim")]
            .into_iter()
            .flatten()
            .count()
    );
}

#[tokio::test]
async fn stale_claim_owner_cannot_mutate_reclaimed_item() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let queue = runtime.thread_queue();
    let item = queue
        .enqueue(thread_id, br#"{"input":["hello"]}"#)
        .await
        .expect("enqueue");
    let first_claim = queue.claim_next(thread_id).await.unwrap().unwrap();
    assert!(
        queue
            .release_claim(&item.queued_item_id, &first_claim.claim_token)
            .await
            .unwrap()
    );
    let second_claim = queue.claim_next(thread_id).await.unwrap().unwrap();

    assert!(
        !queue
            .complete_claim(&item.queued_item_id, &first_claim.claim_token)
            .await
            .unwrap()
    );
    assert!(
        queue
            .complete_claim(&item.queued_item_id, &second_claim.claim_token)
            .await
            .unwrap()
    );
    assert!(queue.list(thread_id).await.unwrap().is_empty());
}

#[tokio::test]
async fn failed_head_blocks_later_pending_item_until_removed() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let queue = runtime.thread_queue();
    let first = queue.enqueue(thread_id, br#"{"n":1}"#).await.unwrap();
    let second = queue.enqueue(thread_id, br#"{"n":2}"#).await.unwrap();
    let claim = queue.claim_next(thread_id).await.unwrap().unwrap();
    queue
        .fail_claim(
            &first.queued_item_id,
            &claim.claim_token,
            br#"{"message":"nope"}"#,
        )
        .await
        .unwrap();

    assert_eq!(None, queue.claim_next(thread_id).await.unwrap());
    assert!(
        queue
            .delete(thread_id, &first.queued_item_id)
            .await
            .unwrap()
    );
    assert_eq!(
        second.queued_item_id,
        queue
            .claim_next(thread_id)
            .await
            .unwrap()
            .unwrap()
            .item
            .queued_item_id
    );
}

#[tokio::test]
async fn recovery_only_marks_stale_claims_failed() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let queue = runtime.thread_queue();
    let item = queue.enqueue(thread_id, br#"{"n":1}"#).await.unwrap();
    queue.claim_next(thread_id).await.unwrap().unwrap();

    assert_eq!(
        0,
        queue
            .recover_claims_as_failed_before(
                thread_id,
                /*stale_before_ms*/ 0,
                br#"{"message":"claim interrupted"}"#,
            )
            .await
            .unwrap()
    );
    assert_eq!(
        1,
        queue
            .recover_claims_as_failed_before(
                thread_id,
                /*stale_before_ms*/ i64::MAX,
                br#"{"message":"claim interrupted"}"#,
            )
            .await
            .unwrap()
    );
    let visible = queue.list(thread_id).await.unwrap();
    assert_eq!(item.queued_item_id, visible[0].queued_item_id);
    assert_eq!(crate::QueuedItemState::Failed, visible[0].state);
    assert_eq!(None, queue.claim_next(thread_id).await.unwrap());
}

#[tokio::test]
async fn reorder_requires_and_returns_every_visible_item() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let queue = runtime.thread_queue();
    let first = queue.enqueue(thread_id, br#"{"n":1}"#).await.unwrap();
    let second = queue.enqueue(thread_id, br#"{"n":2}"#).await.unwrap();
    assert!(
        queue
            .reorder(thread_id, std::slice::from_ref(&first.queued_item_id))
            .await
            .is_err()
    );
    let reordered = queue
        .reorder(
            thread_id,
            &[second.queued_item_id.clone(), first.queued_item_id.clone()],
        )
        .await
        .unwrap();
    assert_eq!(
        vec![second.queued_item_id, first.queued_item_id],
        reordered
            .into_iter()
            .map(|item| item.queued_item_id)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn list_page_preserves_fifo_order_and_payload() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let queue = runtime.thread_queue();
    queue.enqueue(thread_id, br#"{"n":1}"#).await.unwrap();
    let second = queue.enqueue(thread_id, br#"{"n":2}"#).await.unwrap();
    let third = queue.enqueue(thread_id, br#"{"n":3}"#).await.unwrap();

    let page = queue
        .list_page(thread_id, /*offset*/ 1, /*limit*/ 2)
        .await
        .unwrap();
    assert_eq!(
        vec![second.queued_item_id, third.queued_item_id],
        page.iter()
            .map(|item| item.queued_item_id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(br#"{"n":2}"#, page[0].payload_jsonb.as_slice());
}

#[tokio::test]
async fn json_payloads_are_bound_as_text() {
    let (runtime, thread_id) = runtime_with_thread().await;
    let queue = runtime.thread_queue();
    queue.enqueue(thread_id, b"3456").await.unwrap();

    let visible = queue.list(thread_id).await.unwrap();
    assert_eq!(b"3456", visible[0].payload_jsonb.as_slice());
}

#[tokio::test]
async fn deleting_thread_deletes_its_queue() {
    let (runtime, thread_id) = runtime_with_thread().await;
    runtime
        .thread_queue()
        .enqueue(thread_id, br#"{"n":1}"#)
        .await
        .unwrap();
    assert_eq!(1, runtime.delete_thread(thread_id).await.unwrap());
    assert!(
        runtime
            .thread_queue()
            .list(thread_id)
            .await
            .unwrap()
            .is_empty()
    );
}
