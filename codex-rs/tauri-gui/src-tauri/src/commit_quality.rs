use codex_core::git::commit_quality::{CommitQualityAnalyzer, CommitQualityScore};
use tauri::command;

#[command]
pub async fn analyze_commit_quality(
    repo_path: String,
    commit_sha: String,
) -> Result<CommitQualityScore, String> {
    let analyzer = CommitQualityAnalyzer::new();

    analyzer
        .analyze_commit(&repo_path, &commit_sha)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn analyze_commits_batch(
    repo_path: String,
    commit_shas: Vec<String>,
) -> Result<Vec<CommitQualityScore>, String> {
    let analyzer = CommitQualityAnalyzer::new();

    analyzer
        .analyze_commits_batch(&repo_path, &commit_shas)
        .await
        .map_err(|e| e.to_string())
}
