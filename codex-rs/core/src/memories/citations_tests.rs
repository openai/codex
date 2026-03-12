use super::get_thread_id_from_citations;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

#[test]
fn get_thread_id_from_citations_extracts_thread_ids() {
    let first = ThreadId::new();
    let second = ThreadId::new();

    let citations = vec![format!(
        "<memory_citation>\n<citation_entries>\nMEMORY.md:1-2|note=[x]\n</citation_entries>\n<thread_ids>\n{first}\nnot-a-uuid\n{second}\n</thread_ids>\n</memory_citation>"
    )];

    assert_eq!(get_thread_id_from_citations(citations), vec![first, second]);
}

#[test]
fn get_thread_id_from_citations_supports_legacy_rollout_ids() {
    let thread_id = ThreadId::new();

    let citations = vec![format!(
        "<memory_citation>\n<rollout_ids>\n{thread_id}\n</rollout_ids>\n</memory_citation>"
    )];

    assert_eq!(get_thread_id_from_citations(citations), vec![thread_id]);
}
