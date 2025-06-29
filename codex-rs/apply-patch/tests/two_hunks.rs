use codex_apply_patch::{parse_patch, unified_diff_from_chunks, Hunk, ApplyPatchError};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

/// Wraps a patch body in Begin/End markers
fn wrap_patch(body: &str) -> String {
    format!("*** Begin Patch\n{}\n*** End Patch", body)
}

#[test]
fn multiple_non_overlapping_hunks() {
    // Set up a temporary directory and file
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("f.txt");
    let base = "line1\nline2\nline3\nline4\nline5\n";
    fs::write(&file_path, base).unwrap();

    // Create a patch with two hunks, modifying line2 and line4 separately
    let body = "*** Update File: f.txt\n@@\n-line2\n+foo\n*** Update File: f.txt\n@@\n-line4\n+bar";
    let patch = wrap_patch(body);
    // Parse into hunks
    let hunks = parse_patch(&patch).unwrap();
    // Extract all update chunks
    let mut all_chunks = Vec::new();
    for h in hunks {
        match h {
            Hunk::UpdateFile { path, chunks, .. } => {
                // ensure path matches our file
                assert_eq!(path, Path::new("f.txt"));
                all_chunks.extend(chunks);
            }
            _ => panic!("Expected only UpdateFile hunks"),
        }
    }
    // Apply unified diff
    let update = unified_diff_from_chunks(&file_path, &all_chunks).unwrap();
    // Expected final content
    let expected = "line1\nfoo\nline3\nbar\nline5\n";
    assert_eq!(update.content(), expected);
}

#[test]
fn overlapping_hunks_should_fail() {
    // Base file
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("f.txt");
    let base = "A\nB\nC\nD\n";
    fs::write(&file_path, base).unwrap();

    // Two hunks both replacing line B
    let body = "*** Update File: f.txt\n@@\n-B\n+X\n*** Update File: f.txt\n@@\n-B\n+Y";
    let patch = wrap_patch(body);
    let hunks = parse_patch(&patch).unwrap();
    // Collect chunks
    let mut all_chunks = Vec::new();
    for h in hunks {
        if let Hunk::UpdateFile { chunks, .. } = h {
            all_chunks.extend(chunks);
        }
    }
    // Applying overlapping hunks should error
    let result = unified_diff_from_chunks(&file_path, &all_chunks);
    assert!(matches!(result, Err(ApplyPatchError::ComputeReplacements(_))));
}