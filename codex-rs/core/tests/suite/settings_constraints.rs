use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::CodexThread;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::PermissionMessages;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::ThreadSettingsSnapshot;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::time::Duration;
use test_case::test_case;
use tokio::time::timeout;

const INITIAL_MODEL: &str = "gpt-5.4";
const PROTECTED_MODEL: &str = "gpt-5.2";
const REVIEW_INSTRUCTIONS: &str = "Protected-model automatic review instructions.";
const PERMISSION_INSTRUCTIONS: &str = "Protected-model read-only instructions.";

#[derive(Clone, Copy)]
enum SettingsOperation {
    Standalone,
    StandaloneWithReply,
    TurnStart,
}

impl SettingsOperation {
    async fn submit(
        self,
        codex: &CodexThread,
        thread_settings: ThreadSettingsOverrides,
    ) -> Result<ThreadSettingsSnapshot> {
        let id = match self {
            Self::Standalone => {
                codex
                    .submit(Op::ThreadSettings {
                        thread_settings,
                        reply: None,
                    })
                    .await?
            }
            Self::StandaloneWithReply => {
                if let Err(error) = codex.update_thread_settings(thread_settings).await {
                    assert!(matches!(
                        error.details(),
                        CodexErrorDetails::InvalidRequest(_)
                    ));
                    return Err(error.into());
                }
                // A successful update still notifies subscribers. A prior direct rejection must not.
                return match wait_for_event(codex, |event| {
                    matches!(
                        event,
                        EventMsg::ThreadSettingsApplied(_) | EventMsg::Error(_)
                    )
                })
                .await
                {
                    EventMsg::ThreadSettingsApplied(applied) => Ok(applied.thread_settings),
                    event => anyhow::bail!("unexpected settings event: {event:?}"),
                };
            }
            Self::TurnStart => {
                let result = codex
                    .start_or_steer_turn(
                        TurnInputRequest::user_input(vec![UserInput::Text {
                            text: "use the protected model".to_string(),
                            text_elements: Vec::new(),
                        }])
                        .with_thread_settings(thread_settings),
                    )
                    .await?;
                let TurnInputSubmission::Started { turn_id } = result else {
                    anyhow::bail!("expected a new turn, got {result:?}");
                };
                turn_id
            }
        };
        timeout(Duration::from_secs(10), async {
            loop {
                let event = codex.next_event().await?;
                if event.id != id {
                    continue;
                }
                match event.msg {
                    EventMsg::ThreadSettingsApplied(applied) => return Ok(applied.thread_settings),
                    EventMsg::Error(error) => {
                        assert_eq!(error.codex_error_info, Some(CodexErrorInfo::BadRequest));
                        anyhow::bail!(error.message);
                    }
                    _ => {}
                }
            }
        })
        .await?
    }
}

#[test_case(SettingsOperation::Standalone; "standalone settings")]
#[test_case(SettingsOperation::StandaloneWithReply; "standalone settings with reply")]
#[test_case(SettingsOperation::TurnStart; "turn-start settings")]
#[tokio::test]
async fn protected_model_settings_use_the_proposed_permissions(
    operation: SettingsOperation,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_completed("response"),
        ]),
    )
    .await;
    let mut protected = model_info_from_slug(PROTECTED_MODEL);
    let messages = protected.model_messages.as_mut().expect("model messages");
    messages.approvals = Some(ApprovalMessages {
        on_request: None,
        on_request_auto_review: Some(REVIEW_INSTRUCTIONS.to_string()),
        never: None,
        unless_trusted: None,
    });
    messages.permissions = Some(PermissionMessages {
        danger_full_access: None,
        workspace_write: None,
        read_only: Some(PERMISSION_INSTRUCTIONS.to_string()),
    });
    let test = test_codex()
        .with_model(INITIAL_MODEL)
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[auto_review]\nrequired_on_models = [\"gpt-5.2\"]\n",
            ),
        )
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model_info_from_slug(INITIAL_MODEL), protected],
            });
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::User;
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("full-access profile should be allowed");
            config
                .features
                .enable(Feature::GuardianApproval)
                .expect("test config should allow Guardian");
        })
        .build_with_auto_env(&server)
        .await?;
    let initial = test.codex.thread_settings_snapshot().await;
    assert_eq!(initial.permission_profile, PermissionProfile::Disabled);
    let model_update = ThreadSettingsOverrides {
        model: Some(PROTECTED_MODEL.to_string()),
        ..Default::default()
    };

    let error = operation
        .submit(&test.codex, model_update.clone())
        .await
        .expect_err("the protected model requires restricted permissions");
    assert!(
        error.to_string().contains("you need to use auto review"),
        "{error}"
    );
    assert_eq!(test.codex.thread_settings_snapshot().await, initial);
    assert!(response.requests().is_empty());

    let expected = ThreadSettingsSnapshot {
        model: PROTECTED_MODEL.to_string(),
        collaboration_mode: initial.collaboration_mode.with_updates(
            Some(PROTECTED_MODEL.to_string()),
            /*effort*/ None,
            /*developer_instructions*/ None,
        ),
        approvals_reviewer: ApprovalsReviewer::AutoReview,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        ..initial
    };
    let applied = operation
        .submit(
            &test.codex,
            ThreadSettingsOverrides {
                permission_profile: Some(PermissionProfile::read_only()),
                ..model_update
            },
        )
        .await?;
    assert_eq!(applied, expected);
    assert_eq!(test.codex.thread_settings_snapshot().await, expected);
    if let SettingsOperation::TurnStart = operation {
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    // Even an update with no step-settings edits must revalidate the existing
    // protected model against the proposed permissions.
    let error = operation
        .submit(
            &test.codex,
            ThreadSettingsOverrides {
                permission_profile: Some(PermissionProfile::Disabled),
                ..Default::default()
            },
        )
        .await
        .expect_err("full access must invalidate the protected model settings");
    assert!(
        error.to_string().contains("you need to use auto review"),
        "{error}"
    );
    assert_eq!(test.codex.thread_settings_snapshot().await, expected);

    if matches!(
        operation,
        SettingsOperation::Standalone | SettingsOperation::StandaloneWithReply
    ) {
        test.submit_text_turn("use the committed settings").await?;
    }
    let request = response.single_request();
    assert_eq!(request.body_json()["model"], PROTECTED_MODEL);
    let instructions = request.message_input_texts("developer").join("\n");
    assert!(instructions.contains(REVIEW_INSTRUCTIONS), "{instructions}");
    assert!(
        instructions.contains(PERMISSION_INSTRUCTIONS),
        "{instructions}"
    );
    Ok(())
}
