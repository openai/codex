use super::*;
use crate::backend::DEFAULT_LIST_MAX_RESULTS;
use crate::backend::DEFAULT_SEARCH_MAX_RESULTS;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn backend(tempdir: &TempDir) -> LocalMemoriesBackend {
    LocalMemoriesBackend::from_memory_root(tempdir.path())
}

#[tokio::test]
async fn list_returns_recursive_memory_paths() {
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
            MemoryEntry {
                path: "skills/example".to_string(),
                entry_type: MemoryEntryType::Directory,
            },
            MemoryEntry {
                path: "skills/example/SKILL.md".to_string(),
                entry_type: MemoryEntryType::File,
            },
        ]
    );
    assert_eq!(response.truncated, false);
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
async fn read_rejects_invalid_line_offsets() {
    let tempdir = TempDir::new().expect("tempdir");
    tokio::fs::write(tempdir.path().join("MEMORY.md"), "only\n")
        .await
        .expect("write memory file");

    let zero_offset_err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 0,
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("zero line offset should fail");
    assert!(matches!(
        zero_offset_err,
        MemoriesBackendError::InvalidLineOffset
    ));

    let past_end_err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "MEMORY.md".to_string(),
            line_offset: 3,
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
        .search(SearchMemoriesRequest {
            query: "needle".to_string(),
            path: None,
            max_results: DEFAULT_SEARCH_MAX_RESULTS,
        })
        .await
        .expect("search all memories");
    assert_eq!(
        response
            .matches
            .iter()
            .map(|entry| (entry.path.as_str(), entry.line_number))
            .collect::<Vec<_>>(),
        vec![("MEMORY.md", 2), ("rollout_summaries/a.jsonl", 1)]
    );

    let file_response = backend(&tempdir)
        .search(SearchMemoriesRequest {
            query: "needle".to_string(),
            path: Some("MEMORY.md".to_string()),
            max_results: DEFAULT_SEARCH_MAX_RESULTS,
        })
        .await
        .expect("search one memory file");
    assert_eq!(file_response.matches.len(), 1);
    assert_eq!(file_response.matches[0].path, "MEMORY.md");
}

#[tokio::test]
async fn scoped_paths_reject_parent_segments() {
    let tempdir = TempDir::new().expect("tempdir");
    let err = backend(&tempdir)
        .read(ReadMemoryRequest {
            path: "../secret".to_string(),
            line_offset: 1,
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
            max_tokens: DEFAULT_READ_MAX_TOKENS,
        })
        .await
        .expect_err("symlink should be rejected");

    assert!(matches!(err, MemoriesBackendError::InvalidPath { .. }));
}
