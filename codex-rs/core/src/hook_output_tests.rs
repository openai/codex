use std::path::Path;

use super::*;
use anyhow::Context;
use tempfile::tempdir;

async fn write_rollout_stub(codex_home: &Path, thread_id: ThreadId) -> Result<std::path::PathBuf> {
    let dir = codex_home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("01");
    fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("rollout-2025-01-01T00-00-00-{thread_id}.jsonl"));
    fs::write(&path, "").await?;
    Ok(path)
}

#[tokio::test]
async fn small_hook_output_remains_inline() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = AbsolutePathBuf::from_absolute_path(dir.path())?;
    let thread_id = ThreadId::new();

    let output = cap_model_visible_hook_text(
        &codex_home,
        thread_id,
        "short".to_string(),
        TruncationPolicy::Tokens(10),
        /*state_db*/ None,
    )
    .await;

    assert_eq!(output, "short");
    assert!(!codex_home.join(HOOK_OUTPUTS_DIR).exists());
    Ok(())
}

#[tokio::test]
async fn large_hook_output_spills_to_file() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = AbsolutePathBuf::from_absolute_path(dir.path())?;
    let thread_id = ThreadId::new();
    let text = "hook output ".repeat(200);

    let output = cap_model_visible_hook_text(
        &codex_home,
        thread_id,
        text.clone(),
        TruncationPolicy::Tokens(20),
        /*state_db*/ None,
    )
    .await;

    assert!(output.contains("tokens truncated"));
    let path = output
        .lines()
        .find_map(|line| line.strip_prefix("Full hook output saved to: "))
        .context("spill path")?;
    assert_eq!(fs::read_to_string(path).await?, text);
    Ok(())
}

#[tokio::test]
async fn cleanup_removes_orphans_and_keeps_live_threads() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = AbsolutePathBuf::from_absolute_path(dir.path())?;
    let live_thread_id = ThreadId::new();
    let orphan_thread_id = ThreadId::new();
    let live_dir = codex_home
        .join(HOOK_OUTPUTS_DIR)
        .join(live_thread_id.to_string());
    let orphan_dir = codex_home
        .join(HOOK_OUTPUTS_DIR)
        .join(orphan_thread_id.to_string());

    write_rollout_stub(codex_home.as_ref(), live_thread_id).await?;
    fs::create_dir_all(live_dir.as_ref()).await?;
    fs::create_dir_all(orphan_dir.as_ref()).await?;
    fs::write(live_dir.join("live.txt").as_ref(), "live").await?;
    fs::write(orphan_dir.join("orphan.txt").as_ref(), "orphan").await?;

    cleanup_stale_hook_outputs(&codex_home, ThreadId::new(), /*state_db*/ None).await?;

    assert!(live_dir.exists());
    assert!(!orphan_dir.exists());
    Ok(())
}
