//! Exercises archive verification through the measurement reference cache.

mod common;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_core_plugins::PluginMeasurementTarget;
use codex_core_plugins::PluginsConfigInput;
use codex_core_plugins::RemotePluginMeasurementCache;
use codex_core_plugins::TrustedPluginRoots;
use codex_exec_server::LOCAL_FS;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::CodexAuth;
use codex_utils_path_uri::PathUri;
use common::ALLOW_HTTP_BUNDLE_DOWNLOADS;
use common::PluginArchiveSigningFixture;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::process::Command;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[tokio::test]
async fn signed_reference_rejects_tampering_and_can_retry_with_valid_bytes() -> Result<()> {
    // Keep the HTTP override inside the subprocess.
    const CHILD: &str = "CODEX_SIGNED_MEASUREMENT_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "signed_reference_rejects_tampering_and_can_retry_with_valid_bytes",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(ALLOW_HTTP_BUNDLE_DOWNLOADS, "1")
            .output()?;
        ensure!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    let signing = PluginArchiveSigningFixture::load()?;
    let server = MockServer::start().await;
    let config = PluginsConfigInput::new(
        Default::default(),
        Default::default(),
        "openai".to_owned(),
        /*plugins_enabled*/ true,
        /*remote_plugin_enabled*/ true,
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        /*product_sku*/ None,
    );
    let executor = TempDir::new()?;
    let script = "plugins/cache/openai-curated-remote/linear/1.2.3/scripts/install.mjs";
    let script_path = executor.path().join(script);
    std::fs::create_dir_all(script_path.parent().context("script directory")?)?;
    std::fs::write(script_path, "")?;
    let cwd = PathUri::from_host_native_path(executor.path())?;
    let target = PluginMeasurementTarget::from_command(
        &["node".to_owned(), script.to_owned()],
        &cwd,
        LOCAL_FS.as_ref(),
    )
    .await
    .context("canonical plugin target")?;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let cache = RemotePluginMeasurementCache::new();
    let plugin = json!({
        "id": "plugins~linear", "name": "linear", "scope": "GLOBAL", "enabled": true,
        "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE", "status": "ENABLED",
        "release": {
            "version": "1.2.3", "display_name": "Linear", "description": "",
            "bundle_download_url": format!("{}/bundle/linear", server.uri()),
            "interface": {}, "signing": signing.signing,
        },
    });

    // Retry the same plugin/version through the same cache after failure.
    for tampered in [true, false] {
        Mock::given(method("GET"))
            .and(path("/ps/plugins/installed"))
            .and(query_param("scope", "GLOBAL"))
            .and(query_param("includeDownloadUrls", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [plugin], "pagination": {},
            })))
            .expect(1)
            .mount(&server)
            .await;
        let mut bytes = signing.archive.clone();
        if tampered {
            // Still a valid gzip archive: extraction alone must not accept it.
            bytes[4] ^= 1;
        }
        Mock::given(method("GET"))
            .and(path("/bundle/linear"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/ps/plugins/signing-keys"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&signing.signing_keys_response))
            .expect(u64::from(!tampered))
            .mount(&server)
            .await;
        let result = cache
            .prepare(&config, &auth, &target, /*runtime_auth*/ None)
            .await;
        if tampered {
            let error = match result {
                Err(error) => error,
                Ok(_) => anyhow::bail!("tampered archive must never return a trusted reference"),
            };
            assert_eq!(error.to_string(), "plugin measurement preparation failed");
        } else {
            let reference = result?.context("valid signed reference")?;
            assert_ne!(reference.roots(), &TrustedPluginRoots::default());
        }
        server.verify().await;
        server.reset().await;
    }
    Ok(())
}
