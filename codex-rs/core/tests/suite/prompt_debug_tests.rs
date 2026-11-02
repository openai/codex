use std::sync::Arc;

use anyhow::Result;
use codex_core::build_prompt_input;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_extension_api::input_text;
use codex_home::CodexHomeUserInstructionsProvider;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::strip_metadata;
use core_test_support::responses::strip_response_item_id;
use core_test_support::responses::user_message_item;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const TEST_INSTRUCTIONS: &str = "Global test instructions";
const STEP_REMINDER: &str = "Fresh step reminder";

struct StepReminder;

impl ContextContributor for StepReminder {
    fn contribute_world_state<'a>(
        &'a self,
        _input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        Box::pin(async {
            vec![
                WorldStateSectionContribution::new("step_reminder", |_| {
                    let reminder = RenderedWorldStateFragment::new(
                        "developer",
                        ("", ""),
                        input_text(STEP_REMINDER),
                    );
                    (None, Some(reminder))
                })
                .without_full_context(),
            ]
        })
    }
}

#[tokio::test]
async fn build_prompt_input_includes_context_and_user_message() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(codex_home.path().join("AGENTS.md"), TEST_INSTRUCTIONS)?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            codex_self_exe: Some(std::env::current_exe()?),
            ..ConfigOverrides::default()
        })
        .build()
        .await?;
    let user_instructions_provider = Arc::new(CodexHomeUserInstructionsProvider::new(
        config.codex_home.clone(),
    ));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(Arc::new(StepReminder));
    let input = build_prompt_input(
        config,
        vec![UserInput::Text {
            text: "hello from debug prompt".to_string(),
            text_elements: Vec::new(),
        }],
        /*state_db*/ None,
        Arc::new(extensions.build()),
        user_instructions_provider,
    )
    .await?;

    let [initial_context @ .., user_message, reminder] = input.as_slice() else {
        panic!("expected user input followed by step context");
    };
    assert_eq!(
        strip_response_item_id(strip_metadata(user_message.clone())),
        user_message_item("hello from debug prompt")
    );
    assert!(
        matches!(reminder, ResponseItem::Message { role, content, .. }
        if role == "developer" && content == &[input_text(STEP_REMINDER)])
    );
    let contains_text = |item: &ResponseItem, needle: &str| {
        let ResponseItem::Message { content, .. } = item else {
            return false;
        };

        content.iter().any(|content_item| {
            let (ContentItem::InputText { text } | ContentItem::OutputText { text, .. }) =
                content_item
            else {
                return false;
            };
            text.contains(needle)
        })
    };
    assert!(
        initial_context
            .iter()
            .any(|item| contains_text(item, TEST_INSTRUCTIONS))
    );
    assert!(
        !initial_context
            .iter()
            .any(|item| contains_text(item, STEP_REMINDER))
    );
    Ok(())
}
