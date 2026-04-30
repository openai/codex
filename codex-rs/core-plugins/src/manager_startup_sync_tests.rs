use super::*;
use crate::manager::PluginManagerConfigSnapshot;
use crate::manager::PluginsManager;
use crate::startup_sync::curated_plugins_repo_path;
use codex_config::CONFIG_TOML_FILE;
use codex_config::CloudRequirementsLoader;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_config::loader::load_config_layers_state;
use codex_exec_server::LOCAL_FS;
use codex_features::FeatureOverrides;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const TEST_CURATED_PLUGIN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const TEST_CURATED_PLUGIN_CACHE_VERSION: &str = "01234567";

fn write_file(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn write_curated_plugin(root: &Path, plugin_name: &str) {
    let plugin_root = root.join("plugins").join(plugin_name);
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"{plugin_name}"}}"#),
    );
}

fn write_openai_curated_marketplace(root: &Path, plugin_names: &[&str]) {
    let plugins = plugin_names
        .iter()
        .map(|plugin_name| {
            format!(
                r#"{{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "./plugins/{plugin_name}"
      }}
    }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    write_file(
        &root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "openai-curated",
  "plugins": [
{plugins}
  ]
}}"#
        ),
    );
    for plugin_name in plugin_names {
        write_curated_plugin(root, plugin_name);
    }
}

fn write_curated_plugin_sha(codex_home: &Path) {
    write_file(
        &codex_home.join(".tmp/plugins.sha"),
        &format!("{TEST_CURATED_PLUGIN_SHA}\n"),
    );
}

async fn load_plugins_config_snapshot(
    codex_home: &Path,
    chatgpt_base_url: String,
) -> PluginManagerConfigSnapshot {
    let cwd = AbsolutePathBuf::try_from(codex_home.to_path_buf()).expect("cwd should be absolute");
    let config_layer_stack = load_config_layers_state(
        LOCAL_FS.as_ref(),
        codex_home,
        Some(cwd),
        &[],
        LoaderOverrides::without_managed_config_for_tests(),
        CloudRequirementsLoader::default(),
        &NoopThreadConfigLoader,
    )
    .await
    .expect("config layers should load");
    PluginManagerConfigSnapshot::from_layer_stack(
        codex_home.to_path_buf(),
        chatgpt_base_url,
        config_layer_stack,
        FeatureOverrides::default(),
    )
    .expect("plugin manager config snapshot should build")
}

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

    let config =
        load_plugins_config_snapshot(tmp.path(), format!("{}/backend-api/", server.uri())).await;
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
                "plugins/cache/openai-curated/linear/{TEST_CURATED_PLUGIN_CACHE_VERSION}"
            ))
            .is_dir()
    );
    let config =
        std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("config should exist");
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(config.contains("enabled = true"));

    let marker_contents = std::fs::read_to_string(marker_path).expect("marker should be readable");
    assert_eq!(marker_contents, "ok\n");
}
