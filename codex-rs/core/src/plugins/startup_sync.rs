use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tracing::info;
use tracing::warn;

use crate::AuthManager;
use crate::config::Config;

use super::PluginsManager;

const STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE: &str = ".tmp/app-server-remote-plugin-sync-v1";
const STARTUP_REMOTE_PLUGIN_SYNC_PREREQUISITE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn start_startup_remote_plugin_sync_once(
    manager: Arc<PluginsManager>,
    codex_home: PathBuf,
    config: Config,
    auth_manager: Arc<AuthManager>,
) {
    let marker_path = startup_remote_plugin_sync_marker_path(codex_home.as_path());
    if marker_path.is_file() {
        return;
    }

    tokio::spawn(async move {
        if marker_path.is_file() {
            return;
        }

        if !wait_for_startup_remote_plugin_sync_prerequisites(codex_home.as_path()).await {
            warn!(
                codex_home = %codex_home.display(),
                "skipping startup remote plugin sync because curated marketplace is not ready"
            );
            return;
        }

        let auth = auth_manager.auth().await;
        match manager
            .sync_plugins_from_remote(&config, auth.as_ref(), /*additive_only*/ true)
            .await
        {
            Ok(sync_result) => {
                info!(
                    installed_plugin_ids = ?sync_result.installed_plugin_ids,
                    enabled_plugin_ids = ?sync_result.enabled_plugin_ids,
                    disabled_plugin_ids = ?sync_result.disabled_plugin_ids,
                    uninstalled_plugin_ids = ?sync_result.uninstalled_plugin_ids,
                    "completed startup remote plugin sync"
                );
                if let Err(err) =
                    write_startup_remote_plugin_sync_marker(codex_home.as_path()).await
                {
                    warn!(
                        error = %err,
                        path = %marker_path.display(),
                        "failed to persist startup remote plugin sync marker"
                    );
                }
            }
            Err(err) => {
                warn!(
                    error = %err,
                    "startup remote plugin sync failed; will retry on next app-server start"
                );
            }
        }
    });
}

fn startup_remote_plugin_sync_marker_path(codex_home: &Path) -> PathBuf {
    codex_home.join(STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE)
}

fn startup_remote_plugin_sync_prerequisites_ready(codex_home: &Path) -> bool {
    codex_home
        .join(".tmp/plugins/.agents/plugins/marketplace.json")
        .is_file()
        && codex_home.join(".tmp/plugins.sha").is_file()
}

async fn wait_for_startup_remote_plugin_sync_prerequisites(codex_home: &Path) -> bool {
    let deadline = tokio::time::Instant::now() + STARTUP_REMOTE_PLUGIN_SYNC_PREREQUISITE_TIMEOUT;
    loop {
        if startup_remote_plugin_sync_prerequisites_ready(codex_home) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn write_startup_remote_plugin_sync_marker(codex_home: &Path) -> std::io::Result<()> {
    let marker_path = startup_remote_plugin_sync_marker_path(codex_home);
    if let Some(parent) = marker_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(marker_path, b"ok\n").await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::CodexAuth;
    use crate::config::CONFIG_TOML_FILE;
    use crate::plugins::curated_plugins_repo_path;
    use crate::plugins::test_support::TEST_CURATED_PLUGIN_SHA;
    use crate::plugins::test_support::write_curated_plugin_sha;
    use crate::plugins::test_support::write_file;
    use crate::plugins::test_support::write_openai_curated_marketplace;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test]
    async fn startup_remote_plugin_sync_writes_marker_and_reconciles_state() {
        let tmp = tempdir().expect("tempdir");
        let curated_root = curated_plugins_repo_path(tmp.path());
        write_openai_curated_marketplace(&curated_root, &["linear"]);
        write_curated_plugin_sha(tmp.path());
        write_file(
            &tmp.path().join(CONFIG_TOML_FILE),
            r#"[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false
"#,
        );

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/plugins/list"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
  {"id":"1","name":"linear","marketplace_name":"openai-curated","version":"1.0.0","enabled":true}
]"#,
            ))
            .mount(&server)
            .await;

        let mut config = crate::plugins::test_support::load_plugins_config(tmp.path()).await;
        config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
        let manager = Arc::new(PluginsManager::new(tmp.path().to_path_buf()));
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());

        start_startup_remote_plugin_sync_once(
            Arc::clone(&manager),
            tmp.path().to_path_buf(),
            config,
            auth_manager,
        );

        let marker_path = tmp.path().join(STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if marker_path.is_file() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("marker should be written");

        assert!(
            tmp.path()
                .join(format!(
                    "plugins/cache/openai-curated/linear/{TEST_CURATED_PLUGIN_SHA}"
                ))
                .is_dir()
        );
        let config = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE))
            .expect("config should exist");
        assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
        assert!(config.contains("enabled = true"));

        let marker_contents =
            std::fs::read_to_string(marker_path).expect("marker should be readable");
        assert_eq!(marker_contents, "ok\n");
    }
}
