//! Catalog approval headings follow model changes without rewriting or repeating context.

#![cfg(test)]

use super::*;
use codex_core::config::Constrained;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSettingsOverrides;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_approval_headings_follow_selected_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        [
            (
                "response-1",
                "I can inspect the workspace with the current permissions.",
            ),
            (
                "response-2",
                "The selected model's approval instructions are now active.",
            ),
            ("response-3", "The permissions are unchanged."),
        ]
        .into_iter()
        .map(|(id, text)| sse(vec![ev_assistant_message(id, text), ev_completed(id)]))
        .collect(),
    )
    .await;
    let first_slug = "catalog-approval-headings-a";
    let second_slug = "catalog-approval-headings-b";
    let models = [(first_slug, "Model A"), (second_slug, "Model B")]
        .into_iter()
        .map(|(slug, label)| {
            let mut model = model_info_from_slug(slug);
            model.use_responses_lite = true;
            model.model_messages = Some(ModelMessages {
                approvals: Some(ApprovalMessages {
                    request_permissions: Some(format!("# {label} permission requests")),
                    auto_review_suffix: Some(format!("# {label} automatic review")),
                    ..Default::default()
                }),
                ..Default::default()
            });
            model
        })
        .collect();
    let test = test_codex()
        .with_model(first_slug)
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.workspace_roots = vec![config.cwd.clone()];
            config.base_instructions = Some("Help with the workspace.".to_string());
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("read-only permissions should be allowed");
            for feature in [
                Feature::ExecPermissionApprovals,
                Feature::RequestPermissionsTool,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable permission requests");
            }
            config.model_catalog = Some(ModelsResponse { models });
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_text_turn("Inspect the workspace permissions.")
        .await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(second_slug.to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Continue with the selected model.")
        .await?;
    test.submit_text_turn("Continue without changing permissions.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    let headings = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| text.contains("<permissions instructions>"))
                .map(|text| {
                    text.lines()
                        .filter(|line| line.starts_with("# Model "))
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let first_headings = vec![
        "# Model A permission requests",
        "# Model A automatic review",
    ];
    let second_headings = vec![
        "# Model B permission requests",
        "# Model B automatic review",
    ];
    assert_eq!(
        headings,
        vec![
            vec![first_headings.clone()],
            vec![first_headings.clone(), second_headings.clone()],
            vec![first_headings, second_headings],
        ]
    );
    for pair in requests.windows(2) {
        let previous = pair[0].input();
        let next = pair[1].input();
        assert_eq!(&next[..previous.len()], previous);
    }
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "Catalog permission-request and automatic-review headings follow the selected model. The switch appends new developer instructions after the prior reply and before the next user message; an unchanged follow-up preserves the full history without repeating instructions.",
        &requests,
        &ContextSnapshotOptions::default().include_request_settings(),
    ));
    Ok(())
}
