use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use crate::OPENAI_API_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::PluginsConfigInput;
use crate::http_client_selector::HttpClientSelector;
use crate::remote::RemotePluginServiceConfig;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_config::loader::load_config_layers_state;
use codex_exec_server::LOCAL_FS;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestBuilder;
use codex_utils_absolute_path::AbsolutePathBuf;
use http::Method;
use toml::Value;

pub(crate) const TEST_CURATED_PLUGIN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
pub(crate) const TEST_CURATED_PLUGIN_CACHE_VERSION: &str = "01234567";

pub(crate) fn test_http_client_factory() -> HttpClientFactory {
    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
}

#[derive(Debug)]
pub(crate) struct RecordingHttpClientSelector {
    selected_urls: Arc<Mutex<Vec<String>>>,
    delegate: RouteAwareClientPool,
}

impl RecordingHttpClientSelector {
    pub(crate) fn new() -> (Arc<Self>, Arc<Mutex<Vec<String>>>) {
        let selected_urls = Arc::new(Mutex::new(Vec::new()));
        let delegate = RouteAwareClientPool::with_chatgpt_cloudflare_cookies(
            test_http_client_factory(),
            ClientRouteClass::Api,
        );
        (
            Arc::new(Self {
                selected_urls: Arc::clone(&selected_urls),
                delegate,
            }),
            selected_urls,
        )
    }
}

impl HttpClientSelector for RecordingHttpClientSelector {
    fn request(&self, method: Method, url: &str) -> RouteAwareRequestBuilder {
        match self.selected_urls.lock() {
            Ok(mut selected_urls) => selected_urls.push(url.to_string()),
            Err(error) => panic!("selected URL recorder lock should not be poisoned: {error}"),
        }
        self.delegate.request(method, url)
    }
    fn outbound_proxy_policy(&self) -> OutboundProxyPolicy {
        self.delegate.outbound_proxy_policy()
    }
}

pub(crate) fn recording_remote_plugin_service_config(
    chatgpt_base_url: String,
) -> (RemotePluginServiceConfig, Arc<Mutex<Vec<String>>>) {
    let (http_clients, selected_urls) = RecordingHttpClientSelector::new();
    (
        RemotePluginServiceConfig {
            chatgpt_base_url,
            http_clients,
        },
        selected_urls,
    )
}

pub(crate) fn recorded_http_client_urls(selected_urls: &Mutex<Vec<String>>) -> Vec<String> {
    match selected_urls.lock() {
        Ok(selected_urls) => selected_urls.clone(),
        Err(error) => panic!("selected URL recorder lock should not be poisoned: {error}"),
    }
}

pub(crate) fn write_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
    fs::write(path, contents).unwrap();
}

pub(crate) fn write_curated_plugin(root: &Path, plugin_name: &str) {
    let plugin_root = root.join("plugins").join(plugin_name);
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        &format!(
            r#"{{
  "name": "{plugin_name}",
  "description": "Plugin that includes skills, MCP servers, and app connectors"
}}"#
        ),
    );
    write_file(
        &plugin_root.join("skills/SKILL.md"),
        "---\nname: sample\ndescription: sample\n---\n",
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample-docs": {
      "type": "http",
      "url": "https://sample.example/mcp"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "calendar": {
      "id": "connector_calendar"
    }
  }
}"#,
    );
}

pub(crate) fn write_openai_curated_marketplace(root: &Path, plugin_names: &[&str]) {
    write_curated_marketplace(
        root,
        "marketplace.json",
        OPENAI_CURATED_MARKETPLACE_NAME,
        /*display_name*/ None,
        plugin_names,
    );
}

pub(crate) fn write_openai_api_curated_marketplace(root: &Path, plugin_names: &[&str]) {
    write_curated_marketplace(
        root,
        "api_marketplace.json",
        OPENAI_API_CURATED_MARKETPLACE_NAME,
        Some("OpenAI Curated"),
        plugin_names,
    );
}

fn write_curated_marketplace(
    root: &Path,
    manifest_name: &str,
    marketplace_name: &str,
    display_name: Option<&str>,
    plugin_names: &[&str],
) {
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
    let interface = display_name
        .map(|display_name| {
            format!(
                r#"
  "interface": {{
    "displayName": "{display_name}"
  }},"#
            )
        })
        .unwrap_or_default();
    write_file(
        &root.join(".agents/plugins").join(manifest_name),
        &format!(
            r#"{{
  "name": "{marketplace_name}",{interface}
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

pub(crate) fn write_curated_plugin_sha_with(codex_home: &Path, sha: &str) {
    write_file(&codex_home.join(".tmp/plugins.sha"), &format!("{sha}\n"));
}

pub(crate) async fn load_plugins_config(codex_home: &Path, cwd: &Path) -> PluginsConfigInput {
    let codex_home = AbsolutePathBuf::try_from(codex_home).expect("codex home should be absolute");
    let cwd = AbsolutePathBuf::try_from(cwd).expect("cwd should be absolute");
    let config_layer_stack = load_config_layers_state(
        LOCAL_FS.as_ref(),
        codex_home.as_path(),
        Some(cwd),
        &[],
        LoaderOverrides::without_managed_config_for_tests(),
        &NoopThreadConfigLoader,
    )
    .await
    .expect("config should load");
    let effective_config = config_layer_stack.effective_config();
    PluginsConfigInput::new(
        config_layer_stack,
        feature_enabled(&effective_config, "plugins", /*default_enabled*/ true),
        feature_enabled(
            &effective_config,
            "remote_plugin",
            /*default_enabled*/ true,
        ),
        "https://chatgpt.com/backend-api/".to_string(),
        test_http_client_factory(),
    )
}

fn feature_enabled(config: &Value, key: &str, default_enabled: bool) -> bool {
    config
        .get("features")
        .and_then(Value::as_table)
        .and_then(|features| features.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(default_enabled)
}
