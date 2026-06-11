use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_image_generation_extension::install as install_image_generation_extension;
use codex_login::CodexAuth;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::InputModality;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use serde_json::json;

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
