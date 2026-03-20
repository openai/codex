use super::get_thread_id_from_citations;
use super::parse_memory_citation;
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

#[test]
fn parse_memory_citation_extracts_entries_and_rollout_ids() {
    let first = ThreadId::new();
    let second = ThreadId::new();
    let citations = vec![format!(
        "<citation_entries>\nMEMORY.md:1-2|note=[summary]\nrollout_summaries/foo.md:10-12|note=[details]\n</citation_entries>\n<rollout_ids>\n{first}\n{second}\n{first}\n</rollout_ids>"
    )];

    let parsed = parse_memory_citation(citations).expect("memory citation should parse");

    assert_eq!(
        parsed
            .entries
            .iter()
            .map(|entry| (
                entry.path.clone(),
                entry.line_start,
                entry.line_end,
                entry.note.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("MEMORY.md".to_string(), 1, 2, "summary".to_string()),
            (
                "rollout_summaries/foo.md".to_string(),
                10,
                12,
                "details".to_string()
            ),
        ]
    );
    assert_eq!(
        parsed.rollout_ids,
        vec![first.to_string(), second.to_string()]
    );
}
