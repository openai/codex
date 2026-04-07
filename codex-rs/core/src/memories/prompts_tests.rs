use super::*;
use codex_models_manager::model_info::model_info_from_slug;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::fs as tokio_fs;

#[test]
fn build_stage_one_input_message_truncates_rollout_using_model_context_window() {
    let input = format!("{}{}{}", "a".repeat(700_000), "middle", "z".repeat(700_000));
    let mut model_info = model_info_from_slug("gpt-5.2-codex");
    model_info.context_window = Some(123_000);
    let expected_rollout_token_limit = usize::try_from(
        ((123_000_i64 * model_info.effective_context_window_percent) / 100)
            * phase_one::CONTEXT_WINDOW_PERCENT
            / 100,
    )
    .unwrap();
    let expected_truncated = truncate_text(
        &input,
        TruncationPolicy::Tokens(expected_rollout_token_limit),
    );
    let message = build_stage_one_input_message(
        &model_info,
        Path::new("/tmp/rollout.jsonl"),
        Path::new("/tmp"),
        &input,
    )
    .unwrap();

    assert!(expected_truncated.contains("tokens truncated"));
    assert!(expected_truncated.starts_with('a'));
    assert!(expected_truncated.ends_with('z'));
    assert!(message.contains(&expected_truncated));
}

#[test]
fn build_stage_one_input_message_uses_default_limit_when_model_context_window_missing() {
    let input = format!("{}{}{}", "a".repeat(700_000), "middle", "z".repeat(700_000));
    let mut model_info = model_info_from_slug("gpt-5.2-codex");
    model_info.context_window = None;
    let expected_truncated = truncate_text(
        &input,
        TruncationPolicy::Tokens(phase_one::DEFAULT_STAGE_ONE_ROLLOUT_TOKEN_LIMIT),
    );
    let message = build_stage_one_input_message(
        &model_info,
        Path::new("/tmp/rollout.jsonl"),
        Path::new("/tmp"),
        &input,
    )
    .unwrap();

    assert!(message.contains(&expected_truncated));
}

#[test]
fn build_consolidation_prompt_renders_embedded_template() {
    let prompt =
        build_consolidation_prompt(Path::new("/tmp/codex"), &Phase2InputSelection::default());

    assert!(prompt.contains("Folder structure (under /tmp/codex/memories/):"));
    assert!(prompt.contains("Optional memory extensions (under /tmp/codex/memories_extensions/)"));
    assert!(prompt.contains("**Diff since last consolidation:**"));
    assert!(prompt.contains("- selected inputs this run: 0"));
}

#[tokio::test]
async fn build_consolidation_prompt_points_to_extensions_without_inlining_them() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let extension_dir = codex_home.join("memories_extensions/tape_recorder");
    tokio_fs::create_dir_all(extension_dir.join("resources"))
        .await
        .unwrap();
    tokio_fs::write(
        extension_dir.join("instruction.md"),
        "source-specific instructions\n",
    )
    .await
    .unwrap();
    tokio_fs::write(
        extension_dir.join("resources/notes.md"),
        "source-specific resource\n",
    )
    .await
    .unwrap();

    let prompt = build_consolidation_prompt(codex_home, &Phase2InputSelection::default());

    assert!(prompt.contains(&format!(
        "Optional memory extensions (under {}/memories_extensions/)",
        codex_home.display()
    )));
    assert!(prompt.contains("<extension_name>/instruction.md"));
    assert!(prompt.contains("<extension_name>/resources/"));
    assert!(!prompt.contains("source-specific instructions"));
    assert!(!prompt.contains("source-specific resource"));
}

#[tokio::test]
async fn build_memory_tool_developer_instructions_renders_embedded_template() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let memories_dir = codex_home.join("memories");
    tokio_fs::create_dir_all(&memories_dir).await.unwrap();
    tokio_fs::write(
        memories_dir.join("memory_summary.md"),
        "Short memory summary for tests.",
    )
    .await
    .unwrap();

    let instructions = build_memory_tool_developer_instructions(codex_home)
        .await
        .unwrap();

    assert!(instructions.contains(&format!(
        "- {}/memory_summary.md (already provided below; do NOT open again)",
        memories_dir.display()
    )));
    assert!(instructions.contains("Short memory summary for tests."));
    assert_eq!(
        instructions
            .matches("========= MEMORY_SUMMARY BEGINS =========")
            .count(),
        1
    );
}
