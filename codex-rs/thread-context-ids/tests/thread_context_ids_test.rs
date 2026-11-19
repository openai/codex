#![cfg(test)]

use codex_thread_context_ids_test_support::thread_context_ids;
use pretty_assertions::assert_eq;
use uuid::Uuid;

#[test]
fn creates_prefixed_uuid_v7_ids() {
    let id = thread_context_ids().next_item("msg");
    let uuid = id.strip_prefix("msg_").expect("message prefix");
    assert_eq!(
        Uuid::parse_str(uuid).expect("UUID suffix").get_version(),
        Some(uuid::Version::SortRand)
    );
}
