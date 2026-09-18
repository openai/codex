//! Real bundle preparation tests without mutating the parent test environment.

use super::*;
use crate::test_support::load_plugins_config;
use crate::test_support::write_file;
use anyhow::Context;
use anyhow::Result;
use codex_exec_server::LOCAL_FS;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const SCRIPT: &str = "console.log('installed');\n";
const ANALYTICS: &str = "version: 1\noperations:\n  install:\n    path: scripts/install.mjs\n    measurements:\n      duration_ms:\n        dimensions:\n          outcome: [success, error]\n";

fn isolated_bundle_test(name: &str) -> Result<bool> {
    const CHILD: &str = "CODEX_MEASUREMENT_REFERENCE_TEST";
    if std::env::var(CHILD).as_deref() == Ok(name) {
        return Ok(true);
    }
    let output = Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            &format!("plugin_measurement_references::tests::{name}"),
            "--nocapture",
        ])
        .env(CHILD, name)
        .env("CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS", "1")
        .output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(false)
}

fn installed(server: &MockServer, version: &str) -> Value {
    json!({
        "id": "plugins~sites", "name": "sites", "scope": "GLOBAL", "enabled": true,
        "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE", "status": "ENABLED",
        "release": {"version": version, "display_name": "Sites", "description": "",
            "bundle_download_url": format!("{}/bundle/{version}", server.uri()), "interface": {}}
    })
}

async fn catalog(server: &MockServer, plugins: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/installed"))
        .and(query_param("scope", "GLOBAL"))
        .and(query_param("includeDownloadUrls", "true"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"plugins": plugins, "pagination": {}})),
        )
        .mount(server)
        .await;
}

fn archive(plugin_name: &str) -> Result<Vec<u8>> {
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let manifest = json!({"name": plugin_name, "version": "1.0.0"}).to_string();
    for (name, bytes) in [
        (".codex-plugin/plugin.json", manifest.as_str()),
        ("scripts/install.mjs", SCRIPT),
        ("analytics.yaml", ANALYTICS),
        // Invalid capability files must not be parsed by measurement preparation.
        (".mcp.json", "invalid json"),
        ("skills/broken/SKILL.md", "invalid skill"),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, name, bytes.as_bytes())?;
    }
    Ok(archive.into_inner()?.finish()?)
}

async fn bundle(server: &MockServer, version: &str, count: u64) -> Result<()> {
    Mock::given(method("GET"))
        .and(path(format!("/bundle/{version}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive("sites")?))
        .expect(count)
        .mount(server)
        .await;
    Ok(())
}

async fn wait_until_removed(path: &Path) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

fn named_plugin(server: &MockServer, name: &str) -> Result<(Value, PluginMeasurementTarget)> {
    let mut plugin = installed(server, "1.0.0");
    plugin["name"] = json!(name);
    plugin["id"] = json!(format!("plugins~{name}"));
    plugin["release"]["bundle_download_url"] = json!(format!("{}/bundle/{name}", server.uri()));
    let target = PluginMeasurementTarget {
        plugin_id: PluginId::new(name.to_owned(), "openai-curated-remote".to_owned())?,
        version: "1.0.0".to_owned(),
        path_convention: PathConvention::Posix,
    };
    Ok((plugin, target))
}

async fn target(
    executor: &Path,
    version: &str,
) -> Result<(PluginMeasurementTarget, Vec<String>, PathUri)> {
    let script = executor.join(format!(
        "plugins/cache/openai-curated-remote/sites/{version}/scripts/install.mjs"
    ));
    write_file(&script, SCRIPT);
    let command = vec!["node".to_owned(), script.to_string_lossy().into_owned()];
    let cwd = PathUri::from_host_native_path(executor)?;
    let target = PluginMeasurementTarget::from_command(&command, &cwd, LOCAL_FS.as_ref())
        .await
        .context("canonical plugin target")?;
    Ok((target, command, cwd))
}

struct Fixture {
    server: MockServer,
    home: TempDir,
    executor: TempDir,
    config: PluginsConfigInput,
    auth: CodexAuth,
    cache: RemotePluginMeasurementCache,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let server = MockServer::start().await;
        let home = tempfile::tempdir()?;
        let executor = tempfile::tempdir()?;
        let mut config = load_plugins_config(home.path(), executor.path()).await;
        config.chatgpt_base_url = server.uri();
        Ok(Self {
            server,
            home,
            executor,
            config,
            auth: CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            cache: RemotePluginMeasurementCache::new(),
        })
    }

    async fn prepare(
        &self,
        target: &PluginMeasurementTarget,
    ) -> Result<Option<Arc<PluginMeasurementReference>>> {
        self.cache.prepare(&self.config, &self.auth, target).await
    }

    async fn assert_request_count(&self, expected: usize) {
        assert_eq!(
            self.server
                .received_requests()
                .await
                .expect("requests")
                .len(),
            expected
        );
    }

    async fn wait_for_cached_reference(
        &self,
        ready: impl Fn(&ReferenceCell) -> bool,
    ) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let is_ready = {
                    let references = self.cache.references.lock().await;
                    ready(&references[0].1)
                };
                if is_ready {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        Ok(())
    }
}

#[tokio::test]
async fn narrow_preparation_reuses_versions_and_keeps_old_leases_alive() -> Result<()> {
    if !isolated_bundle_test("narrow_preparation_reuses_versions_and_keeps_old_leases_alive")? {
        return Ok(());
    }
    let mut f = Fixture::new().await?;
    let (first, command, cwd) = target(f.executor.path(), "1.0.0").await?;
    let mut unrelated = installed(&f.server, "1.0.0");
    unrelated["id"] = json!("plugins~unrelated");
    unrelated["name"] = json!("unrelated");
    unrelated["release"]["bundle_download_url"] = json!("invalid URL must never be examined");
    catalog(&f.server, vec![unrelated, installed(&f.server, "1.0.0")]).await;
    bundle(&f.server, "1.0.0", /*count*/ 1).await?;
    let (left, right) = tokio::join!(f.prepare(&first), f.prepare(&first));
    let first_ref = left?.context("first reference")?;
    assert!(Arc::ptr_eq(
        &first_ref,
        &right?.context("parallel reference")?
    ));
    let warm = f.prepare(&first).await?.context("warm reference")?;
    assert!(Arc::ptr_eq(&first_ref, &warm));
    let old_home = first_ref
        .home
        .as_ref()
        .expect("reference store")
        .path()
        .to_owned();
    let resolved = first_ref
        .roots()
        .resolve_metrics_operation_in_filesystem(&command, &cwd, LOCAL_FS.as_ref())
        .await
        .context("real resolver")?;
    assert_eq!(resolved.operation.operation_name, "install");
    f.server.verify().await;
    f.server.reset().await;

    let (second, _, _) = target(f.executor.path(), "2.0.0").await?;
    catalog(&f.server, vec![installed(&f.server, "2.0.0")]).await;
    assert!(Arc::ptr_eq(
        &first_ref,
        &f.prepare(&first).await?.context("reference after update")?
    ));
    // A newer release cannot supply an older version's declaration on a cold cache.
    assert!(
        RemotePluginMeasurementCache::new()
            .prepare(&f.config, &f.auth, &first)
            .await?
            .is_none()
    );
    bundle(&f.server, "2.0.0", /*count*/ 1).await?;
    let new_ref = f.prepare(&second).await?.context("new reference")?;
    assert_ne!(
        new_ref.home.as_ref().expect("reference store").path(),
        old_home
    );
    assert!(Arc::ptr_eq(
        &first_ref,
        &f.prepare(&first).await?.context("retained older version")?
    ));
    assert!(
        first_ref
            .roots()
            .resolve_metrics_operation_in_filesystem(&command, &cwd, LOCAL_FS.as_ref())
            .await
            .is_some()
    );
    f.server.verify().await;
    f.server.reset().await;

    let mut disabled = installed(&f.server, "2.0.0");
    disabled["enabled"] = json!(false);
    let mut replaced = installed(&f.server, "2.0.0");
    replaced["id"] = json!("plugins~replacement");
    for plugins in [vec![], vec![disabled], vec![replaced]] {
        catalog(&f.server, plugins).await;
        assert!(f.prepare(&first).await?.is_none());
        f.assert_request_count(/*expected*/ 1).await;
        f.server.reset().await;
    }
    drop(warm);
    drop(first_ref);

    catalog(&f.server, vec![installed(&f.server, "2.0.0")]).await;
    bundle(&f.server, "2.0.0", /*count*/ 1).await?;
    f.auth = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.other",
        "other-account",
        /*chatgpt_plan_type*/ None,
    )?;
    let other_ref = f.prepare(&second).await?.context("other actor reference")?;
    assert!(!Arc::ptr_eq(&new_ref, &other_ref));
    assert!(f.prepare(&first).await?.is_none());
    wait_until_removed(&old_home).await?;
    f.server.verify().await;
    f.server.reset().await;

    let mut disabled = installed(&f.server, "2.0.0");
    disabled["enabled"] = json!(false);
    catalog(&f.server, vec![disabled]).await;
    assert!(f.prepare(&second).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn preparation_preserves_catalog_spelling_and_executor_case_rules() -> Result<()> {
    if !isolated_bundle_test("preparation_preserves_catalog_spelling_and_executor_case_rules")? {
        return Ok(());
    }
    let f = Fixture::new().await?;
    let mut plugin = installed(&f.server, "2.0.0-RC1");
    plugin["name"] = json!("Sites");
    plugin["release"]["version"] = json!("  2.0.0-RC1\n");
    catalog(&f.server, vec![plugin.clone()]).await;
    Mock::given(method("GET"))
        .and(path("/bundle/2.0.0-RC1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive("Sites")?))
        .expect(1)
        .mount(&f.server)
        .await;

    // Drive and UNC preselection normalize these hints identically on every host.
    let windows = PluginMeasurementTarget {
        plugin_id: PluginId::parse("sites@openai-curated-remote")?,
        version: "2.0.0-rc1".to_owned(),
        path_convention: PathConvention::Windows,
    };
    let reference = f.prepare(&windows).await?.context("Windows reference")?;
    let exact = PluginMeasurementTarget {
        plugin_id: PluginId::parse("Sites@openai-curated-remote")?,
        version: "2.0.0-RC1".to_owned(),
        path_convention: PathConvention::Posix,
    };
    assert!(Arc::ptr_eq(
        &reference,
        &f.prepare(&exact)
            .await?
            .context("same authenticated bundle")?
    ));
    let store = crate::store::PluginStore::new(
        reference
            .home
            .as_ref()
            .expect("reference store")
            .path()
            .to_owned(),
    );
    assert_eq!(
        store.active_plugin_version(&exact.plugin_id),
        Some(exact.version.clone())
    );
    let root = store.plugin_root(&exact.plugin_id, &exact.version);
    let command = vec![
        "node".to_owned(),
        root.join("scripts/install.mjs")
            .to_string_lossy()
            .into_owned(),
    ];
    let operation = reference
        .roots()
        .resolve_metrics_operation(&command, &root)
        .context("trusted declaration retains the authenticated identity")?;
    assert_eq!(
        (operation.plugin_id, operation.operation.operation_name),
        (exact.plugin_id.clone(), "install".to_owned())
    );
    for (name, version) in [("sites", "2.0.0-RC1"), ("Sites", "2.0.0-rc1")] {
        let mismatch = PluginMeasurementTarget {
            plugin_id: PluginId::new(name.to_owned(), "openai-curated-remote".to_owned())?,
            version: version.to_owned(),
            path_convention: PathConvention::Posix,
        };
        assert!(f.prepare(&mismatch).await?.is_none());
    }
    f.server.verify().await;
    f.server.reset().await;

    let mut ambiguous = installed(&f.server, "2.0.0-rc1");
    ambiguous["id"] = json!("plugins~other-case");
    catalog(&f.server, vec![plugin.clone(), ambiguous]).await;
    assert!(f.prepare(&windows).await?.is_none());
    f.assert_request_count(/*expected*/ 1).await;
    // POSIX still selects its exact entry, even when another entry folds the same way.
    assert!(Arc::ptr_eq(
        &reference,
        &f.prepare(&exact)
            .await?
            .context("exact POSIX catalog entry")?
    ));
    f.server.reset().await;

    plugin["release"]["version"] = json!("2.0.0-rc1");
    catalog(&f.server, vec![plugin.clone()]).await;
    Mock::given(method("GET"))
        .and(path("/bundle/2.0.0-RC1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive("Sites")?))
        .expect(1)
        .mount(&f.server)
        .await;
    let lower_case = f
        .prepare(&windows)
        .await?
        .context("current catalog version")?;
    assert!(!Arc::ptr_eq(&reference, &lower_case));
    f.server.verify().await;
    f.server.reset().await;

    plugin["release"]["version"] = json!("3.0.0");
    catalog(&f.server, vec![plugin]).await;
    assert!(f.prepare(&windows).await?.is_none());
    assert!(Arc::ptr_eq(
        &reference,
        &f.prepare(&exact)
            .await?
            .context("unambiguous historical version")?
    ));
    Ok(())
}

#[tokio::test]
async fn preparation_requires_enabled_matching_installed_global_plugin() -> Result<()> {
    if !isolated_bundle_test("preparation_requires_enabled_matching_installed_global_plugin")? {
        return Ok(());
    }
    let mut f = Fixture::new().await?;
    let (target, _, _) = target(f.executor.path(), "1.0.0").await?;
    for (field, value) in [
        ("/enabled", json!(false)),
        ("/scope", json!("WORKSPACE")),
        ("/installation_policy", json!("NOT_AVAILABLE")),
        ("/status", json!("DISABLED_BY_ADMIN")),
        ("/name", json!("other")),
        ("/release/version", json!("   ")),
        ("/release/version", json!("../1.0.0")),
    ] {
        let mut plugin = installed(&f.server, "1.0.0");
        *plugin.pointer_mut(field).expect("catalog field") = value;
        catalog(&f.server, vec![plugin]).await;
        assert!(f.prepare(&target).await?.is_none(), "{field}");
        f.assert_request_count(/*expected*/ 1).await;
        f.server.reset().await;
    }
    catalog(&f.server, vec![installed(&f.server, "1.0.0"); 2]).await;
    assert!(f.prepare(&target).await?.is_none());
    f.server.reset().await;
    for (enabled, auth) in [
        (false, f.auth.clone()),
        (true, CodexAuth::from_api_key("test")),
    ] {
        f.config.remote_plugin_enabled = enabled;
        f.auth = auth;
        assert!(f.prepare(&target).await?.is_none());
    }
    f.assert_request_count(/*expected*/ 0).await;

    write_file(
        &f.home.path().join("config.toml"),
        "[plugins.\"sites@openai-curated-remote\"]\nenabled = false\n",
    );
    f.config = load_plugins_config(f.home.path(), f.executor.path()).await;
    f.config.chatgpt_base_url = f.server.uri();
    f.auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut forced = installed(&f.server, "1.0.0");
    forced["installation_policy"] = json!("INSTALLED_BY_DEFAULT");
    catalog(&f.server, vec![forced]).await;
    bundle(&f.server, "1.0.0", /*count*/ 1).await?;
    assert!(f.prepare(&target).await?.is_some());
    Ok(())
}

#[tokio::test]
async fn preparation_failures_do_not_expose_authenticated_urls_or_response_bodies() -> Result<()> {
    if !isolated_bundle_test(
        "preparation_failures_do_not_expose_authenticated_urls_or_response_bodies",
    )? {
        return Ok(());
    }
    let f = Fixture::new().await?;
    let (target, _, _) = target(f.executor.path(), "1.0.0").await?;
    for invalid_url in [true, false] {
        let mut plugin = installed(&f.server, "1.0.0");
        plugin["release"]["bundle_download_url"] = json!(if invalid_url {
            "not a URL?sig=query-secret".to_owned()
        } else {
            format!("{}/bundle/1.0.0?sig=query-secret", f.server.uri())
        });
        catalog(&f.server, vec![plugin]).await;
        Mock::given(method("GET"))
            .and(path("/bundle/1.0.0"))
            .respond_with(ResponseTemplate::new(500).set_body_string("response-body-secret"))
            .expect(u64::from(!invalid_url))
            .mount(&f.server)
            .await;
        let error = match f.prepare(&target).await {
            Err(error) => error,
            Ok(_) => anyhow::bail!("expected a preparation failure"),
        };
        for diagnostic in [
            format!("{error}"),
            format!("{error:#}"),
            format!("{error:?}"),
            format!("{error:#?}"),
        ]
        .into_iter()
        .chain(error.chain().map(ToString::to_string))
        {
            for secret in [
                "query-secret",
                "response-body-secret",
                "sig=",
                f.server.uri().as_str(),
            ] {
                assert!(!diagnostic.contains(secret), "diagnostic exposed {secret}");
            }
        }
        f.server.verify().await;
        f.server.reset().await;
    }
    Ok(())
}

#[tokio::test]
async fn canceled_waiters_do_not_retry_a_failed_preparation() -> Result<()> {
    if !isolated_bundle_test("canceled_waiters_do_not_retry_a_failed_preparation")? {
        return Ok(());
    }
    let f = Arc::new(Fixture::new().await?);
    let (target, _, _) = target(f.executor.path(), "1.0.0").await?;
    let target = Arc::new(target);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut plugin = installed(&f.server, "1.0.0");
    plugin["release"]["bundle_download_url"] =
        json!(format!("http://{}/bundle", listener.local_addr()?));
    catalog(&f.server, vec![plugin]).await;
    let release = Arc::new(Semaphore::new(0));
    let (requested, mut requests) = mpsc::unbounded_channel();
    let bundle_server = tokio::spawn({
        let release = Arc::clone(&release);
        async move {
            for attempt in 0..2 {
                let (socket, _) = listener.accept().await?;
                let mut socket = tokio::io::BufReader::new(socket);
                let mut line = String::new();
                loop {
                    line.clear();
                    socket.read_line(&mut line).await?;
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }
                requested.send(attempt)?;
                let (status, body) = if attempt == 0 {
                    release.acquire().await?.forget();
                    ("500 Internal Server Error", Vec::new())
                } else {
                    ("200 OK", archive("sites")?)
                };
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await?;
                socket.write_all(&body).await?;
            }
            Ok::<_, anyhow::Error>(())
        }
    });
    let mut callers = Vec::new();
    for _ in 0..4 {
        let f = Arc::clone(&f);
        let target = Arc::clone(&target);
        callers.push(tokio::spawn(async move { f.prepare(&target).await }));
    }
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), requests.recv()).await?,
        Some(0)
    );
    // Wait until all four callers hold the cell alongside the cache before
    // canceling the callers during the held download.
    f.wait_for_cached_reference(|cell| Arc::strong_count(cell) >= 5)
        .await?;
    for caller in callers {
        caller.abort();
        assert!(matches!(caller.await, Err(error) if error.is_cancelled()));
    }
    release.add_permits(1);
    // Observe all admitted workers finishing before checking the request count;
    // an abandoned waiter must not become a second initializer after the failure.
    f.wait_for_cached_reference(|cell| cell.has_changed().is_err())
        .await?;
    assert!(requests.try_recv().is_err(), "canceled caller retried");
    let reference = tokio::time::timeout(Duration::from_secs(5), f.prepare(&target))
        .await??
        .context("a new caller can retry the failed preparation")?;
    assert_eq!(requests.recv().await, Some(1));
    assert!(Arc::ptr_eq(
        &reference,
        &f.prepare(&target).await?.context("retry is cached")?
    ));
    bundle_server.await??;
    Ok(())
}

#[tokio::test]
async fn cache_evicts_least_recently_used_bundles_without_invalidating_command_leases() -> Result<()>
{
    if !isolated_bundle_test(
        "cache_evicts_least_recently_used_bundles_without_invalidating_command_leases",
    )? {
        return Ok(());
    }
    let f = Fixture::new().await?;
    let mut plugins = Vec::new();
    let mut targets = Vec::new();
    for index in 0..=MAX_CACHED_REFERENCES {
        let name = format!("plugin-{index}");
        let (plugin, target) = named_plugin(&f.server, &name)?;
        plugins.push(plugin);
        targets.push(target);
        Mock::given(method("GET"))
            .and(path(format!("/bundle/{name}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(archive(&name)?))
            .expect(if index == 1 { 2 } else { 1 })
            .mount(&f.server)
            .await;
    }
    catalog(&f.server, plugins).await;
    let mut leases = Vec::new();
    for target in &targets[..MAX_CACHED_REFERENCES] {
        leases.push(f.prepare(target).await?.context("initial reference")?);
    }
    // A concurrent warm reader must not make a completed entry unevictable.
    let completed_reader = Arc::clone(&f.cache.references.lock().await[1].1);
    assert!(Arc::ptr_eq(
        &leases[0],
        &f.prepare(&targets[0]).await?.context("recent reference")?
    ));
    let extra = f
        .prepare(&targets[MAX_CACHED_REFERENCES])
        .await?
        .context("new plugin")?;
    let evicted_home = leases[1]
        .home
        .as_ref()
        .expect("reference store")
        .path()
        .to_owned();
    assert!(
        evicted_home.exists(),
        "command lease survives cache eviction"
    );
    assert!(Arc::ptr_eq(
        &leases[0],
        &f.prepare(&targets[0])
            .await?
            .context("retained recent reference")?
    ));
    let replacement = f
        .prepare(&targets[1])
        .await?
        .context("evicted plugin reload")?;
    assert_ne!(
        replacement.home.as_ref().expect("reference store").path(),
        evicted_home
    );
    drop(completed_reader);
    drop(leases.remove(1));
    wait_until_removed(&evicted_home).await?;
    assert!(
        extra
            .home
            .as_ref()
            .expect("reference store")
            .path()
            .exists()
    );
    f.server.verify().await;
    Ok(())
}

#[tokio::test]
async fn dropping_the_cache_cancels_an_unused_download() -> Result<()> {
    if !isolated_bundle_test("dropping_the_cache_cancels_an_unused_download")? {
        return Ok(());
    }
    let f = Arc::new(Fixture::new().await?);
    let (target, _, _) = target(f.executor.path(), "1.0.0").await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut plugin = installed(&f.server, "1.0.0");
    plugin["release"]["bundle_download_url"] =
        json!(format!("http://{}/bundle", listener.local_addr()?));
    catalog(&f.server, vec![plugin]).await;
    let (requested, request) = tokio::sync::oneshot::channel();
    let download = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut socket = tokio::io::BufReader::new(socket);
        let mut line = String::new();
        loop {
            line.clear();
            socket.read_line(&mut line).await?;
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        requested.send(()).expect("test awaits bundle request");
        // Hold the response until cancellation closes the download connection.
        let mut remainder = Vec::new();
        socket.read_to_end(&mut remainder).await?;
        Ok::<_, anyhow::Error>(())
    });
    let caller = tokio::spawn({
        let f = Arc::clone(&f);
        async move { f.prepare(&target).await }
    });
    tokio::time::timeout(Duration::from_secs(5), request).await??;
    caller.abort();
    assert!(matches!(caller.await, Err(error) if error.is_cancelled()));
    drop(f);
    tokio::time::timeout(Duration::from_secs(5), download).await???;
    Ok(())
}

#[test]
fn final_store_cleanup_runs_outside_the_async_worker() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()?;
    runtime.block_on(async {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = wait.recv();
        });
        ready.await?;
        let home = tempfile::tempdir()?;
        let path = home.path().to_owned();
        drop(PluginMeasurementReference {
            roots: TrustedPluginRoots::default(),
            home: Some(home),
        });
        assert!(path.exists(), "cleanup must wait for the blocking worker");
        release.send(())?;
        blocker.await?;
        wait_until_removed(&path).await
    })?;

    // References may also outlive their runtime and still need ordinary cleanup.
    let home = tempfile::tempdir()?;
    let path = home.path().to_owned();
    drop(runtime);
    drop(PluginMeasurementReference {
        roots: TrustedPluginRoots::default(),
        home: Some(home),
    });
    assert!(!path.exists());
    Ok(())
}
