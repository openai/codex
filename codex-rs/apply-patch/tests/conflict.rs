use codex_apply_patch::{parse_patch, unified_diff_from_chunks, ApplyPatchError, Hunk};
use std::fs;
use codex_apply_patch::UpdateFileChunk;
use tempfile::tempdir;

/// Helper to wrap patch body in Begin/End markers.
fn wrap_patch(body: &str) -> String {
    format!("*** Begin Patch\n{}\n*** End Patch", body)
}

#[test]
fn non_conflicting_patch_applies_cleanly() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("f.txt");
    let base = "one\ntwo\nthree\n";
    fs::write(&file_path, base).unwrap();

    // Patch: change 'two' to 'twoX'
    let body = "*** Update File: f.txt\n@@\n-two\n+twoX";
    let patch = wrap_patch(body);
    let hunks = parse_patch(&patch).unwrap();
    // Collect update chunks from hunks
    let mut chunks: Vec<UpdateFileChunk> = Vec::new();
    for h in hunks {
        if let Hunk::UpdateFile { chunks: cs, .. } = h {
            chunks.extend(cs);
        }
    }
    let update = unified_diff_from_chunks(&file_path, &chunks).unwrap();
    let expected = "one\ntwoX\nthree\n";
    assert_eq!(update.content(), expected);
}

#[test]
fn conflicting_patches_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("f.txt");
    let base = "one\ntwo\nthree\n";
    fs::write(&file_path, base).unwrap();

    // Two hunks both modifying 'two' differently (overlapping)
    let body = "*** Update File: f.txt\n@@\n-two\n+twoA\n*** Update File: f.txt\n@@\n-two\n+twoB";
    let patch = wrap_patch(body);
    let hunks = parse_patch(&patch).unwrap();
    let mut chunks: Vec<UpdateFileChunk> = Vec::new();
    for h in hunks {
        if let Hunk::UpdateFile { chunks: cs, .. } = h {
            chunks.extend(cs);
        }
    }
    let result = unified_diff_from_chunks(&file_path, &chunks);
    // Expect an error due to overlapping chunks/different replacements
    assert!(matches!(result, Err(ApplyPatchError::ComputeReplacements(_))));
}