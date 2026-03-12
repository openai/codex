use super::*;
use crate::rollout::list::parse_cursor;
use pretty_assertions::assert_eq;

#[test]
fn cursor_to_anchor_normalizes_timestamp_format() {
    let uuid = Uuid::new_v4();
    let ts_str = "2026-01-27T12-34-56";
    let token = format!("{ts_str}|{uuid}");
    let cursor = parse_cursor(token.as_str()).expect("cursor should parse");
    let anchor = cursor_to_anchor(Some(&cursor)).expect("anchor should parse");

    let naive =
        NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H-%M-%S").expect("ts should parse");
    let expected_ts = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
        .with_nanosecond(0)
        .expect("nanosecond");

    assert_eq!(anchor.id, uuid);
    assert_eq!(anchor.ts, expected_ts);
}
