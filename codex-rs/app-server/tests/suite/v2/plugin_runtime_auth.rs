//! Cover direct plugin mutations when runtime authentication is enabled after startup.

use super::*;
use anyhow::Context;
use base64::Engine;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::PluginUninstallResponse;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("dual_header", "install"; "install_dual_header")]
#[test_case("assertion_only", "install"; "install_assertion_only")]
#[test_case("dual_header", "uninstall"; "uninstall_dual_header")]
#[test_case("assertion_only", "uninstall"; "uninstall_assertion_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_mutation_initializes_runtime_after_config_reload(
    mode: &str,
    operation: &str,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let token = format!(
        "e30.{}.c2ln",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            json!({
                "exp": 4_102_444_800_u64,
                "session_id": "session-a",
                "iss": "https://auth.openai.com",
                "https://api.openai.com/auth": {
                    "chatgpt_user_id": "user-123", "chatgpt_account_id": "account-123",
                },
            })
            .to_string()
        )
    );
    if std::env::var(PROBE_SERVER).is_ok() {
        let home = TempDir::new()?;
        write_chatgpt_auth(
            home.path(),
            ChatGptAuthFixture::new(&token)
                .account_id("account-123")
                .chatgpt_user_id("user-123")
                .chatgpt_account_id("account-123"),
            AuthCredentialsStoreMode::File,
        )?;
        write_remote_plugin_catalog_config(home.path(), "https://chatgpt.com/backend-api")?;
        let config_path = home.path().join("config.toml");
        let initial = std::fs::read_to_string(&config_path)?;
        std::fs::write(
            &config_path,
            format!(
                "{initial}\n[features.runtime_agent_instances]\nenabled = false\nmode = {mode:?}\n"
            ),
        )?;
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .without_auto_env()
            .with_env_overrides(&[
                ("OPENAI_API_KEY", None),
                ("CODEX_ACCESS_TOKEN", None),
                (TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS, Some("1")),
            ])
            .build_initialized_with_timeout(DEFAULT_TIMEOUT)
            .await?;
        std::fs::write(
            &config_path,
            format!(
                "{initial}\n[features.runtime_agent_instances]\nenabled = true\nmode = {mode:?}\n"
            ),
        )?;
        let cache = home
            .path()
            .join("plugins/cache/openai-curated-remote/linear");
        match operation {
            "install" => {
                let id = send_remote_plugin_install_request(&mut app, REMOTE_PLUGIN_ID).await?;
                let response: PluginInstallResponse =
                    timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
                assert_eq!(
                    response,
                    PluginInstallResponse {
                        auth_policy: PluginAuthPolicy::OnUse,
                        apps_needing_auth: Vec::new(),
                    }
                );
                assert!(cache.join("1.2.3/.codex-plugin/plugin.json").is_file());
            }
            "uninstall" => {
                std::fs::create_dir_all(cache.join("1.2.3/.codex-plugin"))?;
                std::fs::write(
                    cache.join("1.2.3/.codex-plugin/plugin.json"),
                    r#"{"name":"linear","version":"1.2.3"}"#,
                )?;
                let id = app
                    .send_plugin_uninstall_request(PluginUninstallParams {
                        plugin_id: REMOTE_PLUGIN_ID.into(),
                    })
                    .await?;
                let response: PluginUninstallResponse =
                    timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
                assert_eq!(response, PluginUninstallResponse {});
                assert!(!cache.exists());
            }
            _ => unreachable!("test operation"),
        }
        return Ok(());
    }

    let server = MockServer::start().await;
    let bundle = mount_remote_plugin_bundle(
        &server,
        /*status_code*/ 200,
        remote_plugin_bundle_tar_gz_bytes("linear")?,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-a", "agent_host_id": "host-a",
            "actor": {"kind": "chatgpt", "user_id": "user-123", "account_id": "account-123"},
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/backend-api/ps/plugins/{REMOTE_PLUGIN_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": REMOTE_PLUGIN_ID, "name": "linear", "scope": "GLOBAL",
            "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE", "status": "ENABLED",
            "release": {
                "version": "1.2.3", "bundle_download_url": bundle, "display_name": "Linear",
                "description": "Track work in Linear.",
                "app_ids": [], "interface": {}, "skills": [],
            },
        })))
        .expect(1)
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/backend-api/ps/plugins/{REMOTE_PLUGIN_ID}/{operation}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": REMOTE_PLUGIN_ID, "enabled": operation == "install",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [], "pagination": {"limit": 50, "next_page_token": null},
        })))
        .mount(&server)
        .await;
    run_harness_through_proxy(
        &server,
        &format!("suite::v2::plugin_install::runtime_auth_tests::direct_mutation_initializes_runtime_after_config_reload::{operation}_{mode}"),
        /*realtime_target*/ None,
    ).await?;
    let requests = server
        .received_requests()
        .await
        .context("recorded requests")?;
    let plugin_requests = requests
        .iter()
        .filter(|request| {
            request
                .url
                .path()
                .starts_with(&format!("/backend-api/ps/plugins/{REMOTE_PLUGIN_ID}"))
        })
        .collect::<Vec<_>>();
    assert_eq!(plugin_requests.len(), 2);
    for request in plugin_requests {
        let proof = if mode == "dual_header" {
            assert_eq!(request.headers["authorization"], format!("Bearer {token}"));
            request.headers["x-openai-agent-assertion"].to_str()?
        } else {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            request.headers["authorization"]
                .to_str()?
                .strip_prefix("AgentAssertion ")
                .context("assertion auth")?
        };
        let claims: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).context("assertion payload")?)?,
        )?;
        assert_eq!(claims["runtime_agent_instance"], "runtime-a");
    }
    server.verify().await;
    Ok(())
}
