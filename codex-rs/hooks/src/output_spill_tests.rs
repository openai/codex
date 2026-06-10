use super::*;
use anyhow::Context;
use anyhow::Result;
use tempfile::tempdir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::Path;

#[tokio::test]
async fn small_hook_output_remains_inline() -> Result<()> {
    let dir = tempdir()?;
    let output_dir = AbsolutePathBuf::from_absolute_path(dir.path())?.join(HOOK_OUTPUTS_DIR);
    let thread_id = ThreadId::new();
    let spiller = HookOutputSpiller {
        output_dir: output_dir.clone(),
    };

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
    let text = "hook output ".repeat(1_000);
    let output_dir = AbsolutePathBuf::from_absolute_path(dir.path())?.join(HOOK_OUTPUTS_DIR);
    let spiller = HookOutputSpiller { output_dir };

    let output = spiller
        .maybe_spill_text(ThreadId::new(), text.clone())
        .await;

    assert!(output.contains("tokens truncated"));
    let path = output
        .lines()
        .find_map(|line| line.strip_prefix("Full hook output saved to: "))
        .context("spill path")?;
    assert_eq!(fs::read_to_string(path).await?, text);
    #[cfg(unix)]
    {
        let path = Path::new(path);
        let parent_mode = fs::metadata(path.parent().context("spill parent")?)
            .await?
            .permissions()
            .mode()
            & 0o777;
        let file_mode = fs::metadata(path).await?.permissions().mode() & 0o777;
        assert_eq!((parent_mode, file_mode), (0o700, 0o600));
    }
    Ok(())
}
