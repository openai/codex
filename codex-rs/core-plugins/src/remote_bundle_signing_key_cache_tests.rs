#![cfg(test)]
//! Exercises cache reuse and isolation through the signed bundle download path.

use super::*;
use crate::test_support::isolated_loopback_bundle_test;
use crate::test_support::load_plugins_config;
use crate::test_support::test_auth_manager;
use crate::test_support::test_plugins_manager_with_auth_manager;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_protocol::auth::AuthMode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn signed_downloads_reuse_keys_only_within_the_same_plugin_manager() -> anyhow::Result<()> {
    if !isolated_loopback_bundle_test(
        "remote_bundle::signing_key_cache_tests::signed_downloads_reuse_keys_only_within_the_same_plugin_manager",
    )? {
        return Ok(());
    }

    let fixture: JsonValue = serde_json::from_str(include_str!(
        "../../plugin-archive-verification/testdata/plugin_archive_signing_v1.json"
    ))?;
    let archive = STANDARD.decode(fixture["archive_base64"].as_str().unwrap())?;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive.clone()))
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/signing-keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&fixture["signing_keys_response"]))
        .expect(2)
        .mount(&server)
        .await;

    let home = TempDir::new()?;
    let mut config = load_plugins_config(home.path(), home.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api", server.uri());
    let auth_manager = test_auth_manager(Some(AuthMode::Chatgpt));
    let auth = auth_manager.auth().await.unwrap();
    let manager = test_plugins_manager_with_auth_manager(
        home.path().to_path_buf(),
        /*restriction_product*/ None,
        Arc::clone(&auth_manager),
    );
    let other_manager = manager.for_auth_context(auth_manager, manager.runtime_auth_slot());
    let download = serde_json::from_value(json!({
        "bundle_download_url": format!("{}/bundle", server.uri()),
        "signing": fixture["signing"],
    }))?;
    let bundle = validate_remote_plugin_bundle(
        "plugins~Plugin_00000000000000000000000000000000",
        REMOTE_GLOBAL_MARKETPLACE_NAME,
        "sample",
        Some("1.0.0"),
        &download,
        /*app_manifest*/ None,
    )?;
    for (manager, expected_key_requests) in [(&manager, 1), (&manager, 1), (&other_manager, 2)] {
        let service = manager
            .remote_plugin_service_config(&config)
            .for_current_account();
        assert_eq!(
            download_remote_plugin_bundle(&service, Some(&auth), &bundle).await?,
            archive
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.path().ends_with("/signing-keys"))
                .count(),
            expected_key_requests,
        );
    }
    Ok(())
}
