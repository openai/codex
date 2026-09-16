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
        unified_diff: "a".repeat(crate::workspace_diff::MAX_BYTES + 128),
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

    assert!(!root.join(crate::workspace_diff::FILENAME).exists());
    assert_eq!(memory_storage_bytes(&root).await.expect("storage size"), 6);
    let diff = memory_workspace_diff(&root)
        .await
        .expect("load workspace diff");
    assert_eq!(diff.changes, Vec::new());
}

#[tokio::test]
async fn memory_storage_size_counts_nested_files_but_not_git_or_symlinks() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    for version in [MemoryVersion::V1, MemoryVersion::V2] {
        let root = home.path().join(version.directory_name());
        fs::create_dir_all(root.join("rollout_summaries"))?;
        fs::create_dir_all(root.join("extensions/notes"))?;
        fs::create_dir_all(root.join(".git/objects"))?;
        fs::write(
            root.join(".git/objects/baseline"),
            "git metadata is not memory",
        )?;
        let files = [
            ("memory_summary.md", "summary"),
            ("rollout_summaries/thread.md", "rollout"),
            ("extensions/notes/note.md", "café"),
        ];
        for (path, content) in files {
            fs::write(root.join(path), content)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(root.join("memory_summary.md"), root.join("file-link"))?;
            symlink(&root, root.join("directory-loop"))?;
            symlink(root.join("missing"), root.join("broken-link"))?;
        }
        assert_eq!(
            memory_storage_bytes(&root).await?,
            files
                .iter()
                .map(|(_, content)| content.len() as u64)
                .sum::<u64>(),
        );
    }
    Ok(())
}

#[tokio::test]
async fn prepare_memory_workspace_recovers_unusable_git_dir() {
    let home = TempDir::new().expect("tempdir");
    let root = home.path().join("memories");
    fs::create_dir_all(root.join(".git")).expect("create unusable git dir");
    fs::write(root.join("MEMORY.md"), "memory").expect("write memory");

    prepare_memory_workspace(&root)
        .await
        .expect("prepare memory workspace");

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

#[tokio::test]
async fn validate_consolidation_artifacts_rejects_invalid_summary() {
    let home = TempDir::new().expect("tempdir");
    let root = home.path().join("memories");
    fs::create_dir_all(&root).expect("create memory root");
    fs::write(root.join("MEMORY.md"), "memory").expect("write memory");
    fs::write(root.join("memory_summary.md"), "outdated\n").expect("write summary");

    let err = validate_consolidation_artifacts(&root)
        .await
        .expect_err("invalid summary should fail validation");

    assert!(err.to_string().contains("does not start with v1"));
}

#[tokio::test]
async fn v2_accepts_summary_only_and_checks_its_format_and_size() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let root = home.path();
    fs::write(
        root.join("memory_summary.md"),
        "v1\n\n## User Profile\nProfile\n## User preferences\nPreferences\n## General Tips\nTips\n## What's in Memory\nIndex\n",
    )?;
    validate_consolidation_artifacts_for_version(root, MemoryVersion::V2).await?;
    assert!(validate_consolidation_artifacts(root).await.is_err());
    fs::write(root.join("memory_summary.md"), "v2\nsummary")?;
    assert!(
        validate_consolidation_artifacts_for_version(root, MemoryVersion::V2)
            .await
            .is_err()
    );
    fs::write(
        root.join("memory_summary.md"),
        format!("v1\n{}", "x".repeat(10_000)),
    )?;
    assert!(
        validate_consolidation_artifacts_for_version(root, MemoryVersion::V2)
            .await
            .is_err()
    );
    Ok(())
}
