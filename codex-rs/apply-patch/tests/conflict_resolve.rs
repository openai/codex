use codex_apply_patch::{parse_patch, resolve_conflict, UpdateFileChunk, Hunk};
use std::fs;
use tempfile::tempdir;

/// Wraps patch body in markers
fn wrap_patch(body: &str) -> String {
    format!("*** Begin Patch\n{}\n*** End Patch", body)
}

#[test]
fn resolve_conflict_chooses_first_hunk() {
    // Setup file
    let dir = tempdir().unwrap();
    let path = dir.path().join("f.txt");
    let base = "A\nB\nC\n";
    fs::write(&path, base).unwrap();

    // Conflicting hunks on line B: change to X and to Y
    let body = "*** Update File: f.txt\n@@\n-B\n+X\n*** Update File: f.txt\n@@\n-B\n+Y";
    let patch = wrap_patch(body);
    let hunks = parse_patch(&patch).unwrap();
    let mut chunks: Vec<UpdateFileChunk> = Vec::new();
    for h in hunks {
        if let Hunk::UpdateFile { chunks: cs, .. } = h {
            chunks.extend(cs);
        }
    }
    // Resolve conflict: should apply only first hunk => B->X
    let update = resolve_conflict(&path, &chunks).unwrap();
    let expected = "A\nX\nC\n";
    assert_eq!(update.content(), expected);
}