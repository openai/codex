use super::*;
use crate::memories::extensions::RemovedExtensionResource;
use codex_models_manager::model_info::model_info_from_slug;
use codex_state::Phase2InputSelection;
use core_test_support::PathExt;
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
    model_info.max_context_window = None;
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
fn build_consolidation_prompt_includes_removed_extension_resources() {
    let temp = tempdir().unwrap();
    let memory_root = temp.path().join("memories");
    std::fs::create_dir_all(temp.path().join("memories_extensions")).unwrap();
    let removed_extension_resources = vec![
        RemovedExtensionResource {
            extension: "telepathy".to_string(),
            resource_path: "resources/2026-04-06T11-59-59-abcd-10min-old.md".to_string(),
        },
        RemovedExtensionResource {
            extension: "telepathy".to_string(),
            resource_path: "resources/2026-04-07T12-00-00-abcd-10min-cutoff.md".to_string(),
        },
    ];

    let prompt = build_consolidation_prompt(
        &memory_root,
        &Phase2InputSelection::default(),
        &removed_extension_resources,
    );

    assert!(prompt.contains("Memory extension resources removed by retention pruning:"));
    assert!(prompt.contains("- retention window: 7 days"));
    assert!(prompt.contains("- extension: telepathy"));
    assert!(prompt.contains("  - resources/2026-04-06T11-59-59-abcd-10min-old.md"));
    assert!(prompt.contains("  - resources/2026-04-07T12-00-00-abcd-10min-cutoff.md"));
    assert!(prompt.contains("extension-specific deletion diff"));
}

#[tokio::test]
async fn build_memory_tool_developer_instructions_renders_embedded_template() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path().abs();
    let memories_dir = codex_home.join("memories");
    tokio_fs::create_dir_all(&memories_dir).await.unwrap();
    tokio_fs::write(
        memories_dir.join("memory_summary.md"),
        "Short memory summary for tests.",
    )
    .await
    .unwrap();

    let instructions = build_memory_tool_developer_instructions(&codex_home)
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
