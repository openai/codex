use super::*;
use crate::backend::DEFAULT_LIST_MAX_RESULTS;
use crate::backend::DEFAULT_SEARCH_MAX_RESULTS;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn backend(tempdir: &TempDir) -> LocalMemoriesBackend {
    LocalMemoriesBackend::from_memory_root(tempdir.path())
}

fn search_request(queries: &[&str]) -> SearchMemoriesRequest {
    SearchMemoriesRequest {
        queries: queries.iter().map(|query| (*query).to_string()).collect(),
        match_mode: SearchMatchMode::Any,
        path: None,
        cursor: None,
        context_lines: 0,
        case_sensitive: true,
        max_results: DEFAULT_SEARCH_MAX_RESULTS,
    }
}

#[tokio::test]
async fn list_returns_shallow_memory_paths() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("skills/example"))
        .await
        .expect("create skills dir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "summary")
        .await
        .expect("write memory file");
    tokio::fs::write(tempdir.path().join("skills/example/SKILL.md"), "skill")
        .await
        .expect("write skill file");

    let response = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: None,
            cursor: None,
            max_results: DEFAULT_LIST_MAX_RESULTS,
        })
        .await
        .expect("list memories");

    assert_eq!(
        response.entries,
        vec![
            MemoryEntry {
                path: "MEMORY.md".to_string(),
                entry_type: MemoryEntryType::File,
            },
            MemoryEntry {
                path: "skills".to_string(),
                entry_type: MemoryEntryType::Directory,
            },
        ]
    );
    assert_eq!(response.next_cursor, None);
    assert_eq!(response.truncated, false);
}

#[tokio::test]
async fn list_supports_pagination() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("skills"))
        .await
        .expect("create skills dir");
    tokio::fs::create_dir_all(tempdir.path().join("rollout_summaries"))
        .await
        .expect("create rollout dir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "summary")
        .await
        .expect("write memory file");
    tokio::fs::write(tempdir.path().join("memory_summary.md"), "summary")
        .await
        .expect("write memory summary");

    let page1 = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: None,
            cursor: None,
            max_results: 2,
        })
        .await
        .expect("list first page");
    assert_eq!(
        page1.entries,
        vec![
            MemoryEntry {
                path: "MEMORY.md".to_string(),
                entry_type: MemoryEntryType::File,
            },
            MemoryEntry {
                path: "memory_summary.md".to_string(),
                entry_type: MemoryEntryType::File,
            },
        ]
    );
    assert_eq!(page1.next_cursor.as_deref(), Some("2"));
    assert_eq!(page1.truncated, true);

    let page2 = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: None,
            cursor: page1.next_cursor,
            max_results: 2,
        })
        .await
        .expect("list second page");
    assert_eq!(
        page2.entries,
        vec![
            MemoryEntry {
                path: "rollout_summaries".to_string(),
                entry_type: MemoryEntryType::Directory,
            },
            MemoryEntry {
                path: "skills".to_string(),
                entry_type: MemoryEntryType::Directory,
            },
        ]
    );
    assert_eq!(page2.next_cursor, None);
    assert_eq!(page2.truncated, false);
}

#[tokio::test]
async fn list_preserves_lexicographic_order_for_siblings() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("a"))
        .await
        .expect("create a dir");
    tokio::fs::write(tempdir.path().join("a.txt"), "a")
        .await
        .expect("write a.txt file");
    tokio::fs::write(tempdir.path().join("b.txt"), "b")
        .await
        .expect("write b file");

    let response = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: None,
            cursor: None,
            max_results: DEFAULT_LIST_MAX_RESULTS,
        })
        .await
        .expect("list memories");

    assert_eq!(
        response
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "a.txt", "b.txt"]
    );
}

#[tokio::test]
async fn list_scoped_directory_is_shallow() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("skills/example"))
        .await
        .expect("create nested skills dir");
    tokio::fs::write(tempdir.path().join("skills/README.md"), "readme")
        .await
        .expect("write skills readme");
    tokio::fs::write(tempdir.path().join("skills/example/SKILL.md"), "skill")
        .await
        .expect("write nested skill file");

    let response = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: Some("skills".to_string()),
            cursor: None,
            max_results: DEFAULT_LIST_MAX_RESULTS,
        })
        .await
        .expect("list scoped directory");

    assert_eq!(
        response.entries,
        vec![
            MemoryEntry {
                path: "skills/README.md".to_string(),
                entry_type: MemoryEntryType::File,
            },
            MemoryEntry {
                path: "skills/example".to_string(),
                entry_type: MemoryEntryType::Directory,
            },
        ]
    );
}

#[tokio::test]
async fn list_rejects_invalid_cursor() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "summary")
        .await
        .expect("write memory file");

    let err = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: None,
            cursor: Some("bogus".to_string()),
            max_results: DEFAULT_LIST_MAX_RESULTS,
        })
        .await
        .expect_err("cursor should be rejected");

    assert!(matches!(err, MemoriesBackendError::InvalidCursor { .. }));
}

#[tokio::test]
async fn list_rejects_cursor_past_end() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "summary")
        .await
        .expect("write memory file");

    let err = backend(&tempdir)
        .list(ListMemoriesRequest {
            path: None,
            cursor: Some("2".to_string()),
            max_results: DEFAULT_LIST_MAX_RESULTS,
        })
        .await
        .expect_err("cursor past end should be rejected");

    assert!(matches!(err, MemoriesBackendError::InvalidCursor { .. }));
}

#[tokio::test]
async fn read_rejects_directory_and_returns_file_content() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "remember this")
        .await
        .expect("write memory file");

    let response = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 1,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect("read memory");

    assert_eq!(
        response,
        ReadMemoryResponse {
            path: "MEMORY.md".to_string(),
            start_line_number: 1,
            content: "remember this".to_string(),
            truncated: false,
        }
    );

    let err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: ".".to_string(),
            line_offset: 1,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("directory should not be readable as file");
    assert!(matches!(err, MemoriesBackendError::NotFile { .. }));
}

#[tokio::test]
async fn read_supports_line_offset() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "alpha\nbeta\ngamma\n")
        .await
        .expect("write memory file");

    let response = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 2,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect("read memory from line offset");

    assert_eq!(
        response,
        ReadMemoryResponse {
            path: "MEMORY.md".to_string(),
            start_line_number: 2,
            content: "beta\ngamma\n".to_string(),
            truncated: false,
        }
    );
}

#[tokio::test]
async fn read_supports_max_lines() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "alpha\nbeta\ngamma\n")
        .await
        .expect("write memory file");

    let response = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 2,
            max_lines: Some(1),
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect("read memory with line limit");

    assert_eq!(
        response,
        ReadMemoryResponse {
            path: "MEMORY.md".to_string(),
            start_line_number: 2,
            content: "beta\n".to_string(),
            truncated: true,
        }
    );
}

#[tokio::test]
async fn read_rejects_invalid_line_requests() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "only\n")
        .await
        .expect("write memory file");

    let zero_offset_err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 0,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("zero line offset should fail");
    assert!(matches!(
        zero_offset_err,
        MemoriesBackendError::InvalidLineOffset
    ));

    let zero_max_lines_err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 1,
            max_lines: Some(0),
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("zero max lines should fail");
    assert!(matches!(
        zero_max_lines_err,
        MemoriesBackendError::InvalidMaxLines
    ));

    let past_end_err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 3,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("line offset past end should fail");
    assert!(matches!(
        past_end_err,
        MemoriesBackendError::LineOffsetExceedsFileLength
    ));
}

#[tokio::test]
async fn search_supports_directory_and_file_scopes() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("rollout_summaries"))
        .await
        .expect("create rollout summaries dir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "alpha\nneedle\n")
        .await
        .expect("write memory file");
    tokio::fs::write(
        tempdir.path().join("rollout_summaries/a.jsonl"),
        "needle again\n",
    )
    .await
    .expect("write rollout summary");

    let response = backend(&tempdir)
        .search(search_request(&["needle"]))
        .await
        .expect("search all memories");
    assert_eq!(
        response.matches,
        vec![
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 2,
                content_start_line_number: 2,
                content: "needle".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
            MemorySearchMatch {
                path: "rollout_summaries/a.jsonl".to_string(),
                match_line_number: 1,
                content_start_line_number: 1,
                content: "needle again".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
        ]
    );
    assert_eq!(response.next_cursor, None);
    assert_eq!(response.truncated, false);

    let mut request = search_request(&["needle"]);
    request.path = Some("MEMORY.md".to_string());
    let file_response = backend(&tempdir)
        .search(request)
        .await
        .expect("search one memory file");
    assert_eq!(
        file_response.matches,
        vec![MemorySearchMatch {
            path: "MEMORY.md".to_string(),
            match_line_number: 2,
            content_start_line_number: 2,
            content: "needle".to_string(),
            matched_queries: vec!["needle".to_string()],
        }]
    );
    assert_eq!(file_response.next_cursor, None);
    assert_eq!(file_response.truncated, false);
}

#[tokio::test]
async fn search_supports_pagination() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("rollout_summaries"))
        .await
        .expect("create rollout summaries dir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "needle one\nneedle two\n")
        .await
        .expect("write memory file");
    tokio::fs::write(
        tempdir.path().join("rollout_summaries/a.jsonl"),
        "needle three\n",
    )
    .await
    .expect("write rollout summary");

    let mut page1_request = search_request(&["needle"]);
    page1_request.max_results = 2;
    let page1 = backend(&tempdir)
        .search(page1_request)
        .await
        .expect("search first page");
    assert_eq!(
        page1.matches,
        vec![
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 1,
                content_start_line_number: 1,
                content: "needle one".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 2,
                content_start_line_number: 2,
                content: "needle two".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
        ]
    );
    assert_eq!(page1.next_cursor.as_deref(), Some("2"));
    assert_eq!(page1.truncated, true);

    let mut page2_request = search_request(&["needle"]);
    page2_request.cursor = page1.next_cursor;
    page2_request.max_results = 2;
    let page2 = backend(&tempdir)
        .search(page2_request)
        .await
        .expect("search second page");
    assert_eq!(
        page2.matches,
        vec![MemorySearchMatch {
            path: "rollout_summaries/a.jsonl".to_string(),
            match_line_number: 1,
            content_start_line_number: 1,
            content: "needle three".to_string(),
            matched_queries: vec!["needle".to_string()],
        }]
    );
    assert_eq!(page2.next_cursor, None);
    assert_eq!(page2.truncated, false);
}

#[tokio::test]
async fn search_preserves_global_lexicographic_path_order() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::create_dir_all(tempdir.path().join("a"))
        .await
        .expect("create nested dir");
    tokio::fs::write(tempdir.path().join("a/child.md"), "needle in child\n")
        .await
        .expect("write nested file");
    tokio::fs::write(tempdir.path().join("a.txt"), "needle in sibling\n")
        .await
        .expect("write sibling file");

    let response = backend(&tempdir)
        .search(search_request(&["needle"]))
        .await
        .expect("search memories");

    assert_eq!(
        response.matches,
        vec![
            MemorySearchMatch {
                path: "a.txt".to_string(),
                match_line_number: 1,
                content_start_line_number: 1,
                content: "needle in sibling".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
            MemorySearchMatch {
                path: "a/child.md".to_string(),
                match_line_number: 1,
                content_start_line_number: 1,
                content: "needle in child".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
        ]
    );
}

#[tokio::test]
async fn search_supports_context_lines() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(
        tempdir.path().join("MEMORY.md"),
        "alpha\nneedle\nomega\nneedle again\n",
    )
    .await
    .expect("write memory file");

    let mut request = search_request(&["needle"]);
    request.context_lines = 1;
    let response = backend(&tempdir)
        .search(request)
        .await
        .expect("search with context");

    assert_eq!(
        response.matches,
        vec![
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 2,
                content_start_line_number: 1,
                content: "alpha\nneedle\nomega".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 4,
                content_start_line_number: 3,
                content: "omega\nneedle again".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
        ]
    );
}

#[tokio::test]
async fn search_supports_case_insensitive_matching() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "Needle\nneedle\nNEEDLE\n")
        .await
        .expect("write memory file");

    let sensitive_response = backend(&tempdir)
        .search(search_request(&["needle"]))
        .await
        .expect("search with case-sensitive matching");
    assert_eq!(
        sensitive_response.matches,
        vec![MemorySearchMatch {
            path: "MEMORY.md".to_string(),
            match_line_number: 2,
            content_start_line_number: 2,
            content: "needle".to_string(),
            matched_queries: vec!["needle".to_string()],
        }]
    );

    let mut request = search_request(&["needle"]);
    request.case_sensitive = false;
    let insensitive_response = backend(&tempdir)
        .search(request)
        .await
        .expect("search with case-insensitive matching");
    assert_eq!(
        insensitive_response.matches,
        vec![
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 1,
                content_start_line_number: 1,
                content: "Needle".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 2,
                content_start_line_number: 2,
                content: "needle".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 3,
                content_start_line_number: 3,
                content: "NEEDLE".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
        ]
    );
}

#[tokio::test]
async fn search_supports_any_and_all_match_modes() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(
        tempdir.path().join("MEMORY.md"),
        "alpha needle beta\nalpha only\nneedle only\n",
    )
    .await
    .expect("write memory file");

    let any_response = backend(&tempdir)
        .search(search_request(&["alpha", "needle"]))
        .await
        .expect("search with any match mode");
    assert_eq!(
        any_response.matches,
        vec![
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 1,
                content_start_line_number: 1,
                content: "alpha needle beta".to_string(),
                matched_queries: vec!["alpha".to_string(), "needle".to_string()],
            },
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 2,
                content_start_line_number: 2,
                content: "alpha only".to_string(),
                matched_queries: vec!["alpha".to_string()],
            },
            MemorySearchMatch {
                path: "MEMORY.md".to_string(),
                match_line_number: 3,
                content_start_line_number: 3,
                content: "needle only".to_string(),
                matched_queries: vec!["needle".to_string()],
            },
        ]
    );

    let mut request = search_request(&["alpha", "needle"]);
    request.match_mode = SearchMatchMode::All;
    let all_response = backend(&tempdir)
        .search(request)
        .await
        .expect("search with all match mode");
    assert_eq!(
        all_response.matches,
        vec![MemorySearchMatch {
            path: "MEMORY.md".to_string(),
            match_line_number: 1,
            content_start_line_number: 1,
            content: "alpha needle beta".to_string(),
            matched_queries: vec!["alpha".to_string(), "needle".to_string()],
        }]
    );
}

#[tokio::test]
async fn search_rejects_invalid_cursor() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "needle\n")
        .await
        .expect("write memory file");

    let mut request = search_request(&["needle"]);
    request.cursor = Some("bogus".to_string());
    let err = backend(&tempdir)
        .search(request)
        .await
        .expect_err("cursor should be rejected");
    assert!(matches!(err, MemoriesBackendError::InvalidCursor { .. }));

    let mut request = search_request(&["needle"]);
    request.cursor = Some("2".to_string());
    let past_end_err = backend(&tempdir)
        .search(request)
        .await
        .expect_err("cursor past end should be rejected");
    assert!(matches!(
        past_end_err,
        MemoriesBackendError::InvalidCursor { .. }
    ));
}

#[tokio::test]
async fn scoped_paths_reject_parent_segments() {
    let tempdir = TempDir::new().expect("tempdir");
    let err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "../secret".to_string(),
            line_offset: 1,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("parent traversal should fail");

    assert!(matches!(err, MemoriesBackendError::InvalidPath { .. }));
}

#[cfg(unix)]
#[tokio::test]
async fn read_rejects_symlinked_files() {
    let tempdir = TempDir::new().expect("tempdir");
    let outside = tempdir.path().join("outside.txt");
    tokio::fs::write(&outside, "outside")
        .await
        .expect("write outside file");
    std::os::unix::fs::symlink(&outside, tempdir.path().join("inside-link"))
        .expect("create symlink");

    let err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "inside-link".to_string(),
            line_offset: 1,
            max_lines: None,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("symlink should be rejected");

    assert!(matches!(err, MemoriesBackendError::InvalidPath { .. }));
}
