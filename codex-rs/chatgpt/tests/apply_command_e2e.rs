#![expect(clippy::expect_used)]

use codex_chatgpt::apply_command::apply_diff_from_task;
use codex_chatgpt::get_task::GetTaskResponse;
use std::path::Path;
use tempfile::TempDir;
use tokio::process::Command;

/// Creates a temporary git repository with initial commit
async fn create_temp_git_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();

    let output = Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to initialize git repo: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .await?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .await?;

    std::fs::write(repo_path.join("README.md"), "# Test Repo\n")?;

    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(repo_path)
        .output()
        .await?;

    let output = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to create initial commit: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(temp_dir)
}

async fn mock_get_task_with_fixture() -> anyhow::Result<GetTaskResponse> {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/task_turn_fixture.json");
    let fixture_content = std::fs::read_to_string(fixture_path)?;
    let response: GetTaskResponse = serde_json::from_str(&fixture_content)?;
    Ok(response)
}

#[tokio::test]
async fn test_apply_command_creates_fibonacci_file() {
    let temp_repo = create_temp_git_repo()
        .await
        .expect("Failed to create temp git repo");
    let repo_path = temp_repo.path();

    let task_response = mock_get_task_with_fixture()
        .await
        .expect("Failed to load fixture");

    std::env::set_current_dir(repo_path).expect("Failed to change directory");

    apply_diff_from_task(task_response)
        .await
        .expect("Failed to apply diff from task");

    // Assert that fibonacci.js was created in scripts/ directory
    let fibonacci_path = repo_path.join("scripts/fibonacci.js");
    assert!(fibonacci_path.exists(), "fibonacci.js was not created");

    // Verify the file contents match expected
    let contents = std::fs::read_to_string(&fibonacci_path).expect("Failed to read fibonacci.js");
    assert!(
        contents.contains("function fibonacci(n)"),
        "fibonacci.js doesn't contain expected function"
    );
    assert!(
        contents.contains("#!/usr/bin/env node"),
        "fibonacci.js doesn't have shebang"
    );
    assert!(
        contents.contains("module.exports = fibonacci;"),
        "fibonacci.js doesn't export function"
    );

    // Verify file has correct number of lines (31 as specified in fixture)
    let line_count = contents.lines().count();
    assert_eq!(
        line_count, 31,
        "fibonacci.js should have 31 lines, got {}",
        line_count
    );
}
