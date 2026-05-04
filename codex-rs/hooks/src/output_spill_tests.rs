use super::*;
use anyhow::Context;
use anyhow::Result;
use tempfile::tempdir;

#[tokio::test]
async fn small_hook_output_remains_inline() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = AbsolutePathBuf::from_absolute_path(dir.path())?;
    let output_dir = codex_home.join(HOOK_OUTPUTS_DIR);
    let thread_id = ThreadId::new();
    let spiller = HookOutputSpiller::new(codex_home);

    let output = spiller
        .maybe_spill_text(thread_id, "short".to_string())
        .await;

    assert_eq!(output, "short");
    assert!(!output_dir.exists());
    Ok(())
}

#[tokio::test]
async fn large_hook_output_spills_to_file() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = AbsolutePathBuf::from_absolute_path(dir.path())?;
    let text = "hook output ".repeat(1_000);
    let spiller = HookOutputSpiller::new(codex_home);

    let output = spiller
        .maybe_spill_text(ThreadId::new(), text.clone())
        .await;

    assert!(output.contains("tokens truncated"));
    let path = output
        .lines()
        .find_map(|line| line.strip_prefix("Full hook output saved to: "))
        .context("spill path")?;
    assert_eq!(fs::read_to_string(path).await?, text);
    Ok(())
}
