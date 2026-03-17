use codex_protocol::ThreadId;

pub fn get_thread_id_from_citations(citations: Vec<String>) -> Vec<ThreadId> {
    let mut result = Vec::new();
    for citation in citations {
        let mut ids_block = None;
        for (open, close) in [
            ("<thread_ids>", "</thread_ids>"),
            ("<rollout_ids>", "</rollout_ids>"),
        ] {
            if let Some((_, rest)) = citation.split_once(open)
                && let Some((ids, _)) = rest.split_once(close)
            {
                ids_block = Some(ids);
                break;
            }
        }

        if let Some(ids_block) = ids_block {
            for id in ids_block
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                if let Ok(thread_id) = ThreadId::try_from(id) {
                    result.push(thread_id);
                }
            }
        }
    }
    result
}

#[cfg(test)]
#[path = "citations_tests.rs"]
mod tests;
