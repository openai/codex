//! Independent wire fixtures cover page requests, presence, provenance and unknown data.
use super::*;
use pretty_assertions::assert_eq;

fn page(rows: &[&[u8]]) -> Vec<u8> {
    message(
        /*tag*/ 1,
        &rows
            .iter()
            .flat_map(|row| message(/*tag*/ 1, row))
            .collect::<Vec<_>>(),
    )
}
#[tokio::test]
async fn item_history_paginates_with_explicit_filters_and_preserves_rows() {
    let entry =
        b"\x0a\x01u\x12\x0f\x1a\x0d\x12\x01i\x32\x01d\x2a\x05\x0a\x03\x12\x01Q\x18\x65\x20\x67";
    // A future command source alongside a known agent message.
    let command = b"\x12\x01c\x1a\x00\x22\x01w\x3a\x01i\x58\x63\x60\x03";
    let item = message(/*tag*/ 7, command);
    let opaque = [message(/*tag*/ 1, b"u"), message(/*tag*/ 2, &item)].concat();
    let mut first = page(&[entry, &opaque]);
    first.extend(b"\x12\x01n\x1a\x01b\xa0\x06\x01");
    let unknown = b"\x0a\x01u\x12\x03\xa2\x01\x00";
    let second = page(&[unknown]);
    let mut fixture = fixture_responses(vec![
        response(vec![frame(&first)], tonic::Status::ok("")),
        response(vec![frame(&second)], tonic::Status::ok("")),
    ])
    .await;
    let mut request = ListItemsRequest {
        thread_id: "t".into(),
        turn_id: Some("u".into()),
        page: HistoryQuery {
            cursor: None,
            limit: Some(2),
            order: HistoryOrder::Ascending,
            access: HistoryAccess::Shared,
        },
    };
    let first_page = fixture.client.list_items(&request).await.unwrap();
    assert_request(
        &mut fixture,
        "ListItems",
        b"\x0a\x01t\x12\x01u\x20\x02\x28\x01\x30\x01",
    )
    .await;
    assert_eq!(
        (
            &first_page.thread_id,
            &first_page.next_cursor,
            &first_page.backwards_cursor,
            first_page.payload()
        ),
        (
            &"t".to_owned(),
            &Some("n".into()),
            &Some("b".into()),
            &Bytes::from(first)
        )
    );
    let [item, unknown_source] = first_page.data.as_slice() else {
        panic!("expected two items")
    };
    assert_eq!(unknown_source.summary, None);
    assert_eq!(unknown_source.payload().as_ref(), opaque.as_slice());
    assert_eq!(
        (
            &item.turn_id,
            &item.summary,
            item.started_at_ms,
            item.completed_at_ms,
            item.payload()
        ),
        (
            &"u".to_owned(),
            &Some(ItemSummary::AgentMessage {
                id: "i".into(),
                text: "d".into(),
                questions: Some(vec![AgentQuestion {
                    title: "Q".into(),
                    options: None
                }]),
            }),
            Some(101),
            Some(103),
            &Bytes::from_static(entry)
        )
    );
    request.page.cursor = first_page.next_cursor;
    let second_page = fixture.client.list_items(&request).await.unwrap();
    assert_request(
        &mut fixture,
        "ListItems",
        b"\x0a\x01t\x12\x01u\x1a\x01n\x20\x02\x28\x01\x30\x01",
    )
    .await;
    assert_eq!(
        (second_page.next_cursor, second_page.backwards_cursor),
        (None, None)
    );
    assert_eq!(second_page.data.len(), 1);
    assert_eq!(
        (
            &second_page.data[0].summary,
            second_page.data[0].started_at_ms,
            second_page.data[0].payload()
        ),
        (&None, None, &Bytes::from_static(unknown))
    );
}
#[tokio::test]
async fn turn_history_reuses_summary_and_retains_omitted_metadata() {
    let turn = b"\x08\x65\x10\x02\x22\x01u\x2a\x00\x38\x63\x40\x03";
    let payload = page(&[turn]);
    let mut fixture = fixture(response(vec![frame(&payload)], tonic::Status::ok(""))).await;
    let result = fixture
        .client
        .list_turns(&ListTurnsRequest {
            thread_id: "t".into(),
            page: HistoryQuery {
                cursor: Some("c".into()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    assert_request(&mut fixture, "ListTurns", b"\x0a\x01t\x12\x01c\x20\x02").await;
    assert_eq!(result.data.len(), 1);
    assert_eq!(
        (
            &result.thread_id,
            &result.data[0].summary,
            result.data[0].payload(),
            result.payload()
        ),
        (
            &"t".to_owned(),
            &TurnSummary {
                id: "u".into(),
                status: TurnStatus::Failed,
                started_at: Some(99),
                completed_at: Some(101),
                duration_ms: Some(2),
                error: None
            },
            &Bytes::from_static(turn),
            &Bytes::from(payload)
        )
    );
}
#[tokio::test]
async fn history_distinguishes_empty_pages_from_missing_wrappers_and_bad_rows() {
    let valid_empty = page(&[]);
    let fixture = fixture(response(vec![frame(&valid_empty)], tonic::Status::ok(""))).await;
    let request = ListItemsRequest {
        thread_id: "t".into(),
        turn_id: Some("u".into()),
        page: HistoryQuery::default(),
    };
    assert!(
        fixture
            .client
            .list_items(&request)
            .await
            .unwrap()
            .data
            .is_empty()
    );
    for payload in [
        vec![],
        page(&[b"\x0a\x01u"]),
        page(&[b"\x12\x00"]),
        page(&[b"\x0a\x01u\x12\x05\x1a\x03\x12\x01i"]), // Agent message without text.
        page(&[b"\x0a\x01x\x12\x00"]),
        // Unknown command source and missing required status.
        page(&[b"\x0a\x01u\x12\x0f\x3a\x0d\x12\x01c\x1a\x00\x22\x01w\x3a\x01i\x58\x63"]),
        vec![10, 127],
    ] {
        let server = super::fixture(response(vec![frame(&payload)], tonic::Status::ok(""))).await;
        assert!(matches!(
            server.client.list_items(&request).await,
            Err(Error::InvalidResponse)
        ));
    }
    for turn in [b"\x22\x01u".as_slice(), b"\x40\x01"] {
        let server =
            super::fixture(response(vec![frame(&page(&[turn]))], tonic::Status::ok(""))).await;
        assert!(matches!(
            server
                .client
                .list_turns(&ListTurnsRequest {
                    thread_id: "t".into(),
                    page: HistoryQuery::default()
                })
                .await,
            Err(Error::InvalidResponse)
        ));
    }
}
#[tokio::test]
async fn history_errors_preserve_status_without_retrying() {
    let mut fixture = fixture(response(
        vec![],
        tonic::Status::permission_denied("private error"),
    ))
    .await;
    let error = fixture
        .client
        .list_items(&ListItemsRequest {
            thread_id: "t".into(),
            turn_id: None,
            page: HistoryQuery::default(),
        })
        .await
        .unwrap_err();
    assert!(!format!("{error:?}").contains("private error"));
    assert!(matches!(error, Error::Rpc(status) if status.code == 7));
    assert_request(&mut fixture, "ListItems", b"\x0a\x01t\x28\x02").await;
    assert!(fixture.requests.try_recv().is_err());
}
#[test]
fn history_diagnostics_redact_each_public_payload_type() {
    let query = HistoryQuery {
        cursor: Some("secret".into()),
        ..Default::default()
    };
    let items = ListItemsRequest {
        thread_id: "secret".into(),
        turn_id: Some("secret".into()),
        page: query.clone(),
    };
    let turns = ListTurnsRequest {
        thread_id: "secret".into(),
        page: query.clone(),
    };
    for value in [
        format!("{query:?}"),
        format!("{items:?}"),
        format!("{turns:?}"),
    ] {
        assert!(value.ends_with("([redacted])"));
        assert!(!value.contains("secret"));
    }
}
