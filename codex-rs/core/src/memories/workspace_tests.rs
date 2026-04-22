use super::*;
use codex_git_utils::GitBaselineChange;
use codex_git_utils::GitBaselineChangeStatus;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

#[test]
fn render_workspace_diff_file_bounds_large_diff() {
    let diff = GitBaselineDiff {
        changes: vec![GitBaselineChange {
            status: GitBaselineChangeStatus::Modified,
            path: "MEMORY.md".to_string(),
        }],
        unified_diff: "a".repeat(WORKSPACE_DIFF_MAX_BYTES + 128),
    };

    let rendered = render_workspace_diff_file(&diff);

    assert!(rendered.contains("- M MEMORY.md"));
    assert!(rendered.contains("[workspace diff truncated at 4194304 bytes]"));
    assert!(rendered.ends_with("```\n"));
}

#[tokio::test]
async fn reset_memory_workspace_baseline_removes_generated_diff() {
    let home = TempDir::new().expect("tempdir");
    let root = home.path().join("memories");
    prepare_memory_workspace(&root)
        .await
        .expect("prepare memory workspace");
    fs::write(root.join("MEMORY.md"), "memory").expect("write memory");
    write_workspace_diff(
        &root,
        &GitBaselineDiff {
            changes: vec![GitBaselineChange {
                status: GitBaselineChangeStatus::Added,
                path: "MEMORY.md".to_string(),
            }],
            unified_diff: "+memory\n".to_string(),
        },
    )
    .await
    .expect("write workspace diff");

    reset_memory_workspace_baseline(&root)
        .await
        .expect("reset baseline");

    assert!(!root.join(WORKSPACE_DIFF_FILENAME).exists());
    let diff = memory_workspace_diff(&root)
        .await
        .expect("load workspace diff");
    assert_eq!(diff.changes, Vec::new());
}

#[test]
fn previous_char_boundary_handles_multibyte_text() {
    let text = "aé";
    assert_eq!(previous_char_boundary(text, /*max_bytes*/ 2), 1);
}
