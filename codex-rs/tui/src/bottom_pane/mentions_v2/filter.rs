use codex_file_search::FileMatch;
use codex_file_search::MatchType;
use codex_utils_fuzzy_match::fuzzy_match;

use super::candidate::Candidate;
use super::candidate::MentionType;
use super::candidate::SearchResult;
use super::candidate::Selection;
use super::search_mode::SearchMode;

pub(super) fn filtered_candidates(
    candidates: &[Candidate],
    file_matches: &[FileMatch],
    query: &str,
    search_mode: SearchMode,
    show_file_matches: bool,
) -> Vec<SearchResult> {
    let filter = query.trim();
    let mut out = Vec::new();

    for candidate in candidates {
        if !search_mode.accepts(candidate.mention_type) {
            continue;
        }
        if filter.is_empty() {
            out.push(candidate.to_result(/*match_indices*/ None, /*score*/ 0));
            continue;
        }

        if let Some((indices, score)) = best_tool_match(candidate, filter) {
            out.push(candidate.to_result(indices, score));
        }
    }

    if show_file_matches {
        out.extend(
            file_matches
                .iter()
                .map(file_match_to_row)
                .filter(|candidate| search_mode.accepts(candidate.mention_type)),
        );
    }

    sort_rows(&mut out, filter);
    out
}

fn best_tool_match(candidate: &Candidate, filter: &str) -> Option<(Option<Vec<usize>>, i32)> {
    if let Some((indices, score)) = fuzzy_match(&candidate.display_name, filter) {
        return Some((Some(indices), score));
    }

    candidate
        .search_terms
        .iter()
        .filter(|term| *term != &candidate.display_name)
        .filter_map(|term| fuzzy_match(term, filter).map(|(_indices, score)| score))
        .min()
        .map(|score| (None, score))
}

fn sort_rows(rows: &mut [SearchResult], filter: &str) {
    let type_order = |mention_type: MentionType| match mention_type {
        MentionType::Plugin => 0,
        MentionType::App => 1,
        MentionType::Skill => 2,
        MentionType::File | MentionType::Directory => 3,
    };

    rows.sort_by(|a, b| {
        type_order(a.mention_type)
            .cmp(&type_order(b.mention_type))
            .then_with(|| compare_within_rank(a, b, filter))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
}

fn compare_within_rank(a: &SearchResult, b: &SearchResult, filter: &str) -> std::cmp::Ordering {
    if a.mention_type.is_filesystem() && b.mention_type.is_filesystem() {
        return b.score.cmp(&a.score);
    }
    if filter.is_empty() {
        return a.display_name.cmp(&b.display_name);
    }

    a.match_indices
        .is_none()
        .cmp(&b.match_indices.is_none())
        .then_with(|| a.score.cmp(&b.score))
}

fn file_match_to_row(file_match: &FileMatch) -> SearchResult {
    let mention_type = match file_match.match_type {
        MatchType::File => MentionType::File,
        MatchType::Directory => MentionType::Directory,
    };
    SearchResult {
        display_name: file_match.path.to_string_lossy().to_string(),
        description: None,
        mention_type,
        selection: Selection::File(file_match.path.clone()),
        match_indices: file_match
            .indices
            .as_ref()
            .map(|indices| indices.iter().map(|idx| *idx as usize).collect()),
        score: file_match.score as i32,
    }
}
