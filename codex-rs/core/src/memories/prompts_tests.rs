use super::*;
use crate::models_manager::model_info::model_info_from_slug;
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

#[tokio::test]
async fn build_consolidation_prompt_renders_embedded_template_without_modules() {
    let prompt =
        build_consolidation_prompt(Path::new("/tmp/memories"), &Phase2InputSelection::default())
            .await;

    assert!(prompt.contains("Folder structure (under /tmp/memories/):"));
    assert!(prompt.contains("**Diff since last consolidation:**"));
    assert!(prompt.contains("- selected inputs this run: 0"));
}

#[tokio::test]
async fn build_consolidation_prompt_appends_modules_in_sorted_order() {
    let temp = tempdir().unwrap();
    let memories_dir = temp.path();
    let modules_dir = memories_dir.join("consolidation");
    tokio_fs::create_dir_all(&modules_dir).await.unwrap();
    tokio_fs::write(modules_dir.join("02-second.md"), "second module\n")
        .await
        .unwrap();
    tokio_fs::write(modules_dir.join("01-first.md"), "first module\n")
        .await
        .unwrap();
    tokio_fs::write(modules_dir.join("ignored.txt"), "ignored module\n")
        .await
        .unwrap();

    let prompt = build_consolidation_prompt(memories_dir, &Phase2InputSelection::default()).await;

    let first_index = prompt.find("first module").unwrap();
    let second_index = prompt.find("second module").unwrap();
    assert!(first_index < second_index);
    assert!(!prompt.contains("ignored module"));
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
