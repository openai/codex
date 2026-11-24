//! Exercises remote plugin archive verification through the app-server RPCs.

mod common;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::TestAppServerBuilder;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginInstalledResponse;
use codex_app_server_protocol::PluginReconcileChangedPlugin;
use codex_app_server_protocol::PluginReconcileResponse;
use codex_app_server_protocol::PluginShareCheckoutResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use common::ALLOW_HTTP_BUNDLE_DOWNLOADS;
use common::PluginArchiveSigningFixture;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Match;
use wiremock::Mock;
use wiremock::MockBuilder;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_00000000000000000000000000000000";
const CACHE_DIR: &str = "plugins/cache/openai-curated-remote/linear";
const MANIFEST: &str = ".codex-plugin/plugin.json";

fn app_server(server: &MockServer, codex_home: &Path) -> Result<TestAppServerBuilder> {
    let base_url = format!("{}/backend-api/", server.uri());
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"

[features]
plugins = true
"#
        ),
    )?;
    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    Ok(TestAppServer::builder()
        .with_codex_home(codex_home)
        .without_managed_config()
        // The mock backend uses the auth fixture even on a workload-identity devbox.
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            ("CODEX_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", None),
            ("OPENAI_FEDERATION_RULE_ID", None),
            ("OPENAI_IDENTITY_TOKEN_FILE", None),
            ("OPENAI_IDENTITY_TOKEN_SOURCE", None),
            ("OPENAI_SPIFFE_AUDIENCE", None),
            ("OPENAI_SPIFFE_ID", None),
            ("OPENAI_WORKLOAD_IDENTITY_CONTEXT", None),
            (ALLOW_HTTP_BUNDLE_DOWNLOADS, Some("1")),
        ]))
}

fn catalog_plugin(server: &MockServer, version: &str) -> Value {
    json!({
        "id": REMOTE_PLUGIN_ID, "name": "linear", "scope": "GLOBAL", "enabled": true,
        "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE",
        "release": {
            "version": version, "display_name": "Linear", "description": "Track issues",
            "interface": {}, "bundle_download_url": format!("{}/bundle", server.uri()),
        }
    })
}

fn catalog_detail(requested_id: &str, plugin: Value) -> Mock {
    Mock::given(method("GET"))
        .and(path(format!("/backend-api/ps/plugins/{requested_id}")))
        .and(query_param("includeDownloadUrls", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(plugin))
        .expect(1)
}

fn installed_snapshot(query: impl Match + 'static, plugins: Vec<Value>) -> Mock {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query)
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": plugins, "pagination": {"limit": 200, "next_page_token": null}
        })))
}

fn bundle(bytes: Vec<u8>) -> Mock {
    Mock::given(method("GET"))
        .and(path("/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
}

fn remote_plugin_install(remote_plugin_id: &str) -> MockBuilder {
    Mock::given(method("POST"))
        .and(path(format!(
            "/backend-api/ps/plugins/{remote_plugin_id}/install"
        )))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
}

fn remote_plugin_install_response(remote_plugin_id: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "id": remote_plugin_id,
        "enabled": true,
    }))
}

#[derive(Debug)]
struct CacheManifestExists(PathBuf);

impl Match for CacheManifestExists {
    fn matches(&self, _request: &Request) -> bool {
        self.0.is_file()
    }
}

fn signing_keys(response: ResponseTemplate) -> Mock {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/signing-keys"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(response)
}

fn tampered_archive(mut archive: Vec<u8>) -> Vec<u8> {
    // Changing gzip mtime preserves extraction but invalidates the signed digest.
    archive[4] ^= 1;
    archive
}

async fn reconcile(app: &mut TestAppServer) -> Result<PluginReconcileResponse> {
    let id = app
        .send_raw_request(
            "plugin/reconcile",
            Some(json!({"reason": "tooling_changed"})),
        )
        .await?;
    timeout(DEFAULT_TIMEOUT, app.read_response(id)).await?
}

#[tokio::test]
async fn unsigned_install_skips_signing_keys() -> Result<()> {
    let fixture = PluginArchiveSigningFixture::load()?;
    let server = MockServer::start().await;
    let codex_home = TempDir::new()?;
    let mut plugin = catalog_plugin(&server, "1.2.3");
    plugin["status"] = json!("ENABLED");
    catalog_detail(REMOTE_PLUGIN_ID, plugin)
        .mount(&server)
        .await;
    installed_snapshot(query_param("scope", "GLOBAL"), Vec::new())
        .mount(&server)
        .await;
    bundle(fixture.archive).expect(1).mount(&server).await;
    remote_plugin_install(REMOTE_PLUGIN_ID)
        .respond_with(remote_plugin_install_response(REMOTE_PLUGIN_ID))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/signing-keys"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let mut app = app_server(&server, codex_home.path())?
        .build_initialized()
        .await?;
    let id = app
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: None,
            remote_marketplace_name: Some("openai-curated-remote".to_owned()),
            install_attempt_id: None,
            plugin_name: REMOTE_PLUGIN_ID.to_owned(),
        })
        .await?;
    let response: PluginInstallResponse = timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
    assert_eq!(
        response,
        PluginInstallResponse {
            auth_policy: PluginAuthPolicy::OnUse,
            apps_needing_auth: Vec::new(),
        }
    );
    assert!(
        codex_home
            .path()
            .join(CACHE_DIR)
            .join("1.2.3")
            .join(MANIFEST)
            .is_file()
    );
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn reconcile_materializes_remote_releases_and_preserves_the_cache() -> Result<()> {
    let fixture = PluginArchiveSigningFixture::load()?;
    let server = MockServer::start().await;
    let codex_home = TempDir::new()?;
    let mut app = app_server(&server, codex_home.path())?
        .with_args(&[
            "-c",
            "features.remote_plugin=false",
            "-c",
            "features.plugin_sharing=false",
        ])
        .build_initialized()
        .await?;
    app.start_thread(Default::default()).await?;
    let cache = codex_home.path().join(CACHE_DIR);
    let snapshot = |plugin: &Value| {
        installed_snapshot(
            query_param("includeDownloadUrls", "true"),
            vec![plugin.clone()],
        )
    };
    let changed = PluginReconcileResponse {
        changed_plugins: vec![PluginReconcileChangedPlugin {
            id: "linear@openai-curated-remote".to_string(),
            has_mcps: false,
            has_apps: false,
            has_hooks: false,
            has_skills: true,
        }],
        ..Default::default()
    };
    snapshot(&catalog_plugin(&server, "1.0.0"))
        .expect(1)
        .mount(&server)
        .await;
    bundle(fixture.archive.clone())
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(reconcile(&mut app).await?, changed);
    assert_eq!(
        std::fs::read_to_string(cache.join("1.0.0/skills/linear/SKILL.md"))?,
        "---\nname: linear\ndescription: Signed plugin test skill\n---\nUse the signed test fixture.\n"
    );
    server.verify().await;
    server.reset().await;

    let mut plugin = catalog_plugin(&server, "1.2.3");
    plugin["release"]["signing"] = fixture.signing.clone();
    snapshot(&plugin).expect(1).mount(&server).await;
    bundle(fixture.archive.clone())
        .expect(1)
        .mount(&server)
        .await;
    let mut keys = fixture.signing_keys_response;
    keys["cache_ttl_seconds"] = json!(0);
    signing_keys(ResponseTemplate::new(200).set_body_json(keys))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(reconcile(&mut app).await?, changed);
    assert!(cache.join("1.2.3").join(MANIFEST).is_file());
    server.verify().await;
    server.reset().await;

    let mut same_version = plugin.clone();
    same_version["release"]["signing"]["signature"] = Value::Null;
    snapshot(&same_version).expect(1).mount(&server).await;
    assert_eq!(
        reconcile(&mut app).await?,
        PluginReconcileResponse::default()
    );
    let requests = server
        .received_requests()
        .await
        .context("recorded requests")?;
    assert!(requests.iter().all(|req| !matches!(
        req.url.path(),
        "/bundle" | "/backend-api/ps/plugins/signing-keys"
    )));
    server.verify().await;
    server.reset().await;

    plugin["release"]["version"] = json!("2.0.0");
    let rejected = PluginReconcileResponse {
        failed_remote_plugin_ids: vec![REMOTE_PLUGIN_ID.to_string()],
        failed_materialization_remote_plugin_ids: vec![REMOTE_PLUGIN_ID.to_string()],
        ..Default::default()
    };
    snapshot(&plugin).expect(1).mount(&server).await;
    bundle(tampered_archive(fixture.archive.clone()))
        .expect(1)
        .mount(&server)
        .await;
    signing_keys(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    assert_eq!(reconcile(&mut app).await?, rejected);
    assert!(cache.join("1.2.3").join(MANIFEST).is_file());
    assert!(!cache.join("2.0.0").exists());
    server.verify().await;
    server.reset().await;

    // A blocking reconcile waits for the background retry. Both must reject the new version.
    snapshot(&plugin).expect(2).mount(&server).await;
    bundle(fixture.archive).expect(2).mount(&server).await;
    signing_keys(ResponseTemplate::new(503))
        .expect(2)
        .mount(&server)
        .await;
    let id = app
        .send_raw_request("plugin/installed", Some(json!({})))
        .await?;
    let _: PluginInstalledResponse = timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
    assert_eq!(reconcile(&mut app).await?, rejected);
    assert!(cache.join("1.2.3").join(MANIFEST).is_file());
    assert!(!cache.join("2.0.0").exists());
    Ok(())
}

async fn assert_digest_error(app: &mut TestAppServer, id: i64) -> Result<()> {
    let error = timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert!(
        error
            .error
            .message
            .contains("archive digest does not match"),
        "{}",
        error.error.message
    );
    Ok(())
}

#[tokio::test]
async fn share_checkout_verifies_before_writing_local_files() -> Result<()> {
    let fixture = PluginArchiveSigningFixture::load()?;
    for tampered in [true, false] {
        let server = MockServer::start().await;
        let codex_home = TempDir::new()?;
        let home = TempDir::new()?;
        let mut plugin = catalog_plugin(&server, "1.2.3");
        plugin["release"]["signing"] = fixture.signing.clone();
        plugin["scope"] = json!("WORKSPACE");
        plugin["discoverability"] = json!("PRIVATE");
        plugin["share_url"] = json!("https://chatgpt.example/plugins/share/share-key-1");
        plugin["share_principals"] = json!([{
            "principal_type": "user", "principal_id": "user-owner__account-123", "role": "owner", "name": "Owner"
        }]);
        catalog_detail(REMOTE_PLUGIN_ID, plugin)
            .mount(&server)
            .await;
        installed_snapshot(query_param("scope", "WORKSPACE"), Vec::new())
            .mount(&server)
            .await;
        let archive = fixture.archive.clone();
        bundle(if tampered {
            tampered_archive(archive)
        } else {
            archive
        })
        .expect(1)
        .mount(&server)
        .await;
        signing_keys(ResponseTemplate::new(200).set_body_json(&fixture.signing_keys_response))
            .expect(u64::from(!tampered))
            .mount(&server)
            .await;
        let home_env = home.path().to_string_lossy();
        let mut app = app_server(&server, codex_home.path())?
            .without_auto_env()
            .with_env_overrides(&[("HOME", Some(&home_env)), ("USERPROFILE", Some(&home_env))])
            .build_initialized()
            .await?;
        let id = app
            .send_raw_request(
                "plugin/share/checkout",
                Some(json!({"remotePluginId": REMOTE_PLUGIN_ID})),
            )
            .await?;
        let checkout = home.path().join("plugins").join("linear");
        let marketplace = home.path().join(".agents/plugins/marketplace.json");
        let mapping = codex_home
            .path()
            .join(".tmp/plugin-share-local-paths-v1.json");
        if tampered {
            assert_digest_error(&mut app, id).await?;
            assert!(!checkout.exists());
            assert!(!marketplace.exists());
            assert!(!mapping.exists());
        } else {
            let response: PluginShareCheckoutResponse =
                timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
            assert_eq!(response.plugin_path.as_path(), checkout);
            assert_eq!(response.plugin_id, "linear@codex-curated");
            assert!(checkout.join("skills/linear/SKILL.md").is_file());
            let marketplace: Value = serde_json::from_slice(&std::fs::read(marketplace)?)?;
            assert_eq!(marketplace["plugins"][0]["name"], "linear");
            let mapping: Value = serde_json::from_slice(&std::fs::read(mapping)?)?;
            assert_eq!(
                mapping["localPluginPathsByRemotePluginId"][REMOTE_PLUGIN_ID],
                json!(checkout)
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn install_verifies_eval_archives_before_cloud_mutation() -> Result<()> {
    let mut fixture = PluginArchiveSigningFixture::load()?;
    let mut eval_key = fixture.signing_keys_response["keys"][0].clone();
    eval_key["key_id"] = json!("scoped-eval-key");
    eval_key["purpose"] = json!("EVAL_PLUGIN_RELEASE_ARCHIVE_SIGNING");
    fixture.signing_keys_response["eval_keys"] = json!([eval_key]);
    fixture.signing["key_id"] = json!("scoped-eval-key");
    for tampered in [false, true] {
        let server = MockServer::start().await;
        let codex_home = TempDir::new()?;
        let installed = codex_home.path().join(CACHE_DIR).join("1.2.3");
        let requested_id = "linear-alias";
        let mut plugin = catalog_plugin(&server, "1.2.3");
        plugin["release"]["signing"] = fixture.signing.clone();
        catalog_detail(requested_id, plugin).mount(&server).await;
        installed_snapshot(query_param("scope", "GLOBAL"), Vec::new())
            .mount(&server)
            .await;
        let archive = fixture.archive.clone();
        bundle(if tampered {
            tampered_archive(archive)
        } else {
            archive
        })
        .expect(1)
        .mount(&server)
        .await;
        signing_keys(ResponseTemplate::new(200).set_body_json(&fixture.signing_keys_response))
            .expect(u64::from(!tampered))
            .mount(&server)
            .await;
        remote_plugin_install(requested_id)
            .and(CacheManifestExists(installed.join(MANIFEST)))
            .respond_with(remote_plugin_install_response(requested_id))
            .mount(&server)
            .await;
        let mut app = app_server(&server, codex_home.path())?
            .build_initialized()
            .await?;
        app.start_thread(Default::default()).await?;
        let id = app
            .send_plugin_install_request(PluginInstallParams {
                marketplace_path: None,
                remote_marketplace_name: Some("caller-marketplace-is-ignored".to_owned()),
                install_attempt_id: None,
                plugin_name: requested_id.to_owned(),
            })
            .await?;
        if tampered {
            assert_digest_error(&mut app, id).await?;
        } else {
            let response: PluginInstallResponse =
                timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
            assert_eq!(
                response,
                PluginInstallResponse {
                    auth_policy: PluginAuthPolicy::OnUse,
                    apps_needing_auth: Vec::new(),
                }
            );
        }
        assert_eq!(installed.is_dir(), !tampered);
        let requests = server
            .received_requests()
            .await
            .context("recorded requests")?;
        assert_eq!(
            requests
                .iter()
                .filter(|req| req.method == "POST" && req.url.path().ends_with("/install"))
                .count(),
            usize::from(!tampered)
        );
    }
    Ok(())
}
