use super::*;
use anyhow::Context;
use anyhow::Result;
use tempfile::tempdir;

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
    Ok(())
}

#[tokio::test]
async fn additional_contexts_apply_limits_individually() -> Result<()> {
    let dir = tempdir()?;
    let limited_text = "limited hook output ".repeat(1_000);
    let unlimited_text = "unlimited hook output ".repeat(5_000);
    assert!(approx_token_count(&unlimited_text) > 10_000);
    let output_dir = AbsolutePathBuf::from_absolute_path(dir.path())?.join(HOOK_OUTPUTS_DIR);
    let spiller = HookOutputSpiller { output_dir };
    let output = spiller
        .maybe_spill_additional_contexts(
            ThreadId::new(),
            vec![
                AdditionalContext {
                    text: limited_text.clone(),
                    limit: AdditionalContextLimit::from_config(Some(1)),
                },
                AdditionalContext {
                    text: unlimited_text.clone(),
                    limit: AdditionalContextLimit::from_config(Some(0)),
                },
                AdditionalContext {
                    text: unlimited_text.clone(),
                    limit: AdditionalContextLimit::from_config(Some(usize::MAX)),
                },
            ],
        )
        .await;
    let [limited_output, zero_limit_output, high_limit_output] = output.as_slice() else {
        panic!("expected one output for each additional context");
    };
    assert!(limited_output.contains("Full hook output saved to:"));
    assert_eq!(zero_limit_output, &unlimited_text);
    assert_eq!(high_limit_output, &unlimited_text);
    Ok(())
}
