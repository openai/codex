#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn discovery_preserves_pagination_and_order_on_each_request() {
    let payload = b"\x0a\x00\x12\x04next\x1a\x04back";
    let mut fixture = fixture_responses(vec![
        response(vec![frame(payload)], tonic::Status::ok("")),
        response(vec![frame(b"\x0a\x00")], tonic::Status::ok("")),
    ])
    .await;
    let mut query = ThreadListQuery {
        cursor: Some("cursor".into()),
        limit: Some(7),
        sort_key: ThreadSortKey::UpdatedAt,
        sort_direction: ThreadSortDirection::Asc,
        originators: vec!["tui".into()],
    };
    let page = fixture.client.list_threads(&query).await.unwrap();
    assert_eq!(
        (
            page.data,
            page.next_cursor.clone(),
            page.backwards_cursor,
            page.read_states
        ),
        (Vec::new(), Some("next".into()), Some("back".into()), None)
    );
    assert_request(
        &mut fixture,
        "ListThreads",
        b"\x0a\x06cursor\x10\x07\x18\x02\x20\x01\x28\x00\x32\x05\x0a\x03tui",
    )
    .await;
    query.cursor = page.next_cursor;
    let last = fixture.client.list_threads(&query).await.unwrap();
    assert_eq!((last.data, last.next_cursor), (Vec::new(), None));
    assert_request(
        &mut fixture,
        "ListThreads",
        b"\x0a\x04next\x10\x07\x18\x02\x20\x01\x28\x00\x32\x05\x0a\x03tui",
    )
    .await;
    assert!(fixture.requests.try_recv().is_err());
}

#[tokio::test]
async fn discovery_preserves_native_snapshots_and_read_state() {
    // Independent native Thread encodings: Git metadata and future field (99), and a legacy row.
    let thread = Bytes::from_static(
        b"\x22\x01v\x28\x01\x32\x01/\x48\x00\x62\x28\x0a\x04main\x12\x18https://example.com/repo\x1a\x06abc123\
          \x72\x01t\x82\x01\x01p\xaa\x01\x00\xda\x01\x01s\xe2\x01\x02\x08\x63\xea\x01\x02\x12\x00\xfa\x01\x00\x80\x02\x02\x98\x06\x01",
    );
    let legacy = Bytes::from_static(b"\x22\x00\x28\x00\x32\x01/\x48\x00\x72\x06legacy\x82\x01\x00\xaa\x01\x00\xda\x01\x00\xe2\x01\x02\x08\x01\xea\x01\x02\x12\x00\xfa\x01\x00\x80\x02\x00");
    let mut data = Vec::new();
    for row in [&thread, &legacy, &thread] {
        prost::encoding::bytes::encode(1, row, &mut data);
    }
    let mut payload = Vec::new();
    prost::encoding::bytes::encode(1, &Bytes::from(data), &mut payload);
    // Read state map: t -> {first_unread: thread_start, revision: r}.
    payload.extend_from_slice(b"\x22\x0e\x0a\x0c\x0a\x01t\x12\x07\x0a\x02\x0a\x00\x12\x01r");
    let mut fixture = fixture(response(vec![frame(&payload)], tonic::Status::ok(""))).await;
    let page = fixture
        .client
        .list_threads(&ThreadListQuery::default())
        .await
        .unwrap();
    assert_eq!(
        page.data
            .iter()
            .map(ThreadSnapshot::protobuf)
            .collect::<Vec<_>>(),
        vec![&thread, &legacy, &thread]
    );
    assert_eq!(
        (
            page.data[1].id.as_str(),
            page.data[1].session_id.as_str(),
            page.data[1].model_provider.as_str(),
            page.data[1].cli_version.as_str()
        ),
        ("legacy", "", "", "")
    );
    let page_json = serde_json::to_value(&page).unwrap();
    assert_eq!(
        page_json["data"][0]["gitInfo"],
        serde_json::json!({"branch": "main", "originUrl": "https://example.com/repo", "sha": "abc123"})
    );
    assert!(page.data[1].git_info.is_none());
    assert_eq!(page.data[0].status, ThreadStatus::Idle);
    assert_eq!(
        page.read_states,
        Some(std::collections::HashMap::from([(
            "t".into(),
            ThreadReadState {
                first_unread: Some(ThreadUnreadPosition::ThreadStart),
                revision: "r".into(),
            }
        )]))
    );
    assert_request(
        &mut fixture,
        "ListThreads",
        b"\x18\x01\x20\x02\x28\x00\x32\x00",
    )
    .await;
}

#[tokio::test]
async fn discovery_rejects_missing_data_invalid_snapshot_and_empty_revision() {
    for payload in [
        &b""[..],
        &b"\x0a\x02\x0a\x00"[..],
        &b"\x0a\x00\x22\x07\x0a\x05\x0a\x01t\x12\x00"[..],
    ] {
        let mut fixture = fixture(response(vec![frame(payload)], tonic::Status::ok(""))).await;
        assert!(matches!(
            fixture
                .client
                .list_threads(&ThreadListQuery::default())
                .await,
            Err(Error::InvalidResponse)
        ));
        assert_request(
            &mut fixture,
            "ListThreads",
            b"\x18\x01\x20\x02\x28\x00\x32\x00",
        )
        .await;
        assert!(fixture.requests.try_recv().is_err());
    }
}
