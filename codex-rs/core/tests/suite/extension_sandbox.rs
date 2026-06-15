use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_image_generation_extension::install as install_image_generation_extension;
use codex_login::CodexAuth;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::InputModality;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::UserSubmission;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn image_generation_extensions(auth: &CodexAuth) -> Arc<ExtensionRegistry<Config>> {
    let auth_manager = codex_core::test_support::auth_manager_from_auth(auth.clone());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    install_image_generation_extension(&mut extension_builder, auth_manager);
    Arc::new(extension_builder.build())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_tool_receives_turn_environment_sandbox() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let extensions = image_generation_extensions(&auth);
    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
            model_info.input_modalities = vec![InputModality::Text, InputModality::Image];
        })
        .with_config(|config| {
            assert!(config.web_search_mode.set(WebSearchMode::Live).is_ok());
            assert!(config.features.enable(Feature::ImageGeneration).is_ok());
            assert!(config.features.disable(Feature::ImageGenExt).is_ok());
        });
    let test = builder.build(&server).await?;
    let denied_path = test.config.cwd.join("denied.png");
    std::fs::write(&denied_path, b"not readable")?;

    let call_id = "image-edit-denied";
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "image_gen",
                    "imagegen",
                    &json!({
                        "prompt": "edit the image",
                        "referenced_image_paths": [denied_path.display().to_string()],
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut file_system_sandbox_policy = FileSystemSandboxPolicy::default();
    file_system_sandbox_policy
        .entries
        .push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: denied_path.clone(),
            },
            access: FileSystemAccessMode::Deny,
        });
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    test.submit_turn_with_permission_profile("edit the denied image", permission_profile)
        .await?;

    let request = response_mock
        .last_request()
        .context("missing request containing extension output")?;
    let output = request
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .context("extension error text should be present")?;
    assert!(
        output.starts_with(&format!(
            "unable to read referenced image at `{}`:",
            denied_path.display()
        )),
        "unexpected extension error: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_tool_uses_granted_turn_permissions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/images/edits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "created": 1,
            "data": [{"b64_json": "cG5n"}],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let extensions = image_generation_extensions(&auth);
    let base_permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let permission_profile_for_config = base_permission_profile.clone();
    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
            model_info.input_modalities = vec![InputModality::Text, InputModality::Image];
        })
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(permission_profile_for_config)
                .expect("set permission profile");
            assert!(config.web_search_mode.set(WebSearchMode::Live).is_ok());
            assert!(config.features.enable(Feature::ImageGeneration).is_ok());
            assert!(config.features.disable(Feature::ImageGenExt).is_ok());
            assert!(
                config
                    .features
                    .enable(Feature::RequestPermissionsTool)
                    .is_ok()
            );
        });
    let test = builder.build(&server).await?;

    let image_dir = tempfile::tempdir()?;
    let image_path = image_dir.path().canonicalize()?.join("granted.png");
    std::fs::write(&image_path, TINY_PNG_BYTES)?;
    let requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![image_dir.path().canonicalize()?.try_into()?]),
            Some(Vec::new()),
        )),
        ..RequestPermissionProfile::default()
    };
    let permissions_call_id = "permissions-call";
    let image_call_id = "image-edit-granted";
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    permissions_call_id,
                    "request_permissions",
                    &serde_json::to_string(&json!({
                        "reason": "Read an image outside the workspace",
                        "permissions": requested_permissions,
                    }))?,
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_function_call_with_namespace(
                    image_call_id,
                    "image_gen",
                    "imagegen",
                    &json!({
                        "prompt": "edit the image",
                        "referenced_image_paths": [image_path.display().to_string()],
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-3"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let (sandbox_policy, permission_profile) =
        turn_permission_fields(base_permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            submission: UserSubmission {
                items: vec![UserInput::Text {
                    text: "request access and edit the image".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
            },
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::RequestPermissions(request) = event else {
        panic!("expected request_permissions before turn completion");
    };
    assert_eq!(request.call_id, permissions_call_id);
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: permissions_call_id.to_string(),
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock
        .last_request()
        .context("missing request containing extension output")?;
    let output = request.function_call_output(image_call_id);
    let image = &output["output"][0];
    assert_eq!(image["type"], "input_image");
    assert_eq!(image["image_url"], "data:image/png;base64,cG5n");

    Ok(())
}
