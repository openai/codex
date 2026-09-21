//! Exercises live catalog routing through the public model/list RPC.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use core_test_support::responses::mount_models_once;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::MockServer;

fn catalog(slug: &str) -> Result<ModelsResponse> {
    Ok(ModelsResponse {
        models: vec![serde_json::from_value::<ModelInfo>(json!({
            "slug": slug,
            "display_name": slug,
            "supported_reasoning_levels": [],
            "shell_type": "shell_command",
            "visibility": "list",
            "supported_in_api": true,
            "priority": 0,
            "support_verbosity": false,
            "truncation_policy": {"mode": "bytes", "limit": 10000},
            "experimental_supported_tools": []
        }))?],
    })
}

async fn list_models(server: &mut TestAppServer) -> Result<ModelListResponse> {
    server
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams::default(),
        })
        .await
}

#[test_case("other"; "selection_changes")]
#[test_case("gateway"; "definition_changes")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_list_blocks_noncompliant_cached_provider_until_requirements_allow_it(
    required_selection: &str,
) -> Result<()> {
    let gateway = MockServer::start().await;
    let other = MockServer::start().await;
    mount_models_once(&gateway, catalog("gateway-model")?).await;
    mount_models_once(&other, catalog("other-model")?).await;
    let home = TempDir::new()?;
    let providers = format!(
        r#"
[model_providers.gateway]
name = "Gateway"
base_url = "{}/v1"
requires_openai_auth = true
[model_providers.other]
name = "Other"
base_url = "{}/v1"
requires_openai_auth = true
"#,
        gateway.uri(),
        other.uri(),
    );
    std::fs::write(
        home.path().join("config.toml"),
        format!("model_provider = 'gateway'\n{providers}"),
    )?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("chatgpt-access-token").plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    let requirements_path = home.path().join("requirements.toml");
    std::fs::write(
        &requirements_path,
        format!("model_provider = 'gateway'\n{providers}"),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized()
        .await?;

    // Let the startup refresh populate the cache before exercising cached RPCs.
    tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            if let Ok(bytes) = tokio::fs::read(home.path().join("models_cache.json")).await
                && let Ok(cache) = serde_json::from_slice::<serde_json::Value>(&bytes)
                && cache["models"][0]["slug"] == "gateway-model"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;

    let initial = list_models(&mut server).await?;
    assert_eq!(
        initial
            .data
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["gateway-model"]
    );
    assert_eq!(list_models(&mut server).await?, initial);
    assert_eq!(
        gateway
            .received_requests()
            .await
            .expect("request recording enabled")
            .len(),
        1
    );
    assert!(
        other
            .received_requests()
            .await
            .expect("request recording enabled")
            .is_empty()
    );

    let updated_providers = if required_selection == "gateway" {
        providers.replace(&gateway.uri(), &other.uri())
    } else {
        providers.clone()
    };
    let updated = format!("model_provider = '{required_selection}'\n{updated_providers}");
    std::fs::write(&requirements_path, &updated)?;
    let id = server
        .send_raw_request("model/list", Some(json!({})))
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(id))
        .await?;
    assert_eq!(error.error, codex_app_server_protocol::JSONRPCErrorError {
        code: -32600,
        message: "failed to load configuration: Your organization's required model provider settings changed. Restart Codex to apply them; this request was not sent".to_string(),
        data: None,
    });
    assert_eq!(
        gateway
            .received_requests()
            .await
            .expect("request recording enabled")
            .len(),
        1
    );
    assert!(
        other
            .received_requests()
            .await
            .expect("request recording enabled")
            .is_empty()
    );

    // A warm catalog must not hide a policy load failure or make any stale request.
    std::fs::write(&requirements_path, "model_provider = []")?;
    let id = server
        .send_raw_request("model/list", Some(json!({})))
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(id))
        .await?;
    assert_eq!(error.error.code, -32600);
    assert!(error.error.message.contains("failed to load configuration"));
    assert_eq!(
        gateway
            .received_requests()
            .await
            .expect("request recording enabled")
            .len(),
        1
    );
    assert!(
        other
            .received_requests()
            .await
            .expect("request recording enabled")
            .is_empty()
    );

    // Removing the requirement makes the retained provider permissible again.
    std::fs::remove_file(requirements_path)?;
    assert_eq!(list_models(&mut server).await?, initial);
    assert_eq!(
        gateway
            .received_requests()
            .await
            .expect("request recording enabled")
            .len(),
        1
    );
    Ok(())
}
