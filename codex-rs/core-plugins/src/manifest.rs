use codex_config::HooksFile;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::find_plugin_manifest_path;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;
const MAX_DEFAULT_PROMPT_COUNT: usize = 3;
const MAX_DEFAULT_PROMPT_LEN: usize = 128;

pub type PluginManifest = codex_plugin::manifest::PluginManifest<AbsolutePathBuf>;
pub type PluginManifestHooks = codex_plugin::manifest::PluginManifestHooks<AbsolutePathBuf>;
pub type PluginManifestInterface = codex_plugin::manifest::PluginManifestInterface<AbsolutePathBuf>;
pub type PluginManifestMcpServers =
    codex_plugin::manifest::PluginManifestMcpServers<AbsolutePathBuf>;
pub type PluginManifestPaths = codex_plugin::manifest::PluginManifestPaths<AbsolutePathBuf>;

pub(crate) type UriPluginManifest = codex_plugin::manifest::PluginManifest<PathUri>;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    // Keep manifest paths as raw strings so we can validate the required `./...` syntax before
    // resolving them under the plugin root.
    #[serde(default)]
    skills: Option<RawPluginManifestPaths>,
    #[serde(default)]
    mcp_servers: Option<RawPluginManifestMcpServers>,
    #[serde(default)]
    apps: Option<String>,
    #[serde(default)]
    hooks: Option<RawPluginManifestHooks>,
    #[serde(default)]
    interface: Option<RawPluginManifestInterface>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestInterface {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    long_description: Option<String>,
    #[serde(default)]
    developer_name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    #[serde(alias = "websiteURL")]
    website_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "privacyPolicyURL")]
    privacy_policy_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "termsOfServiceURL")]
    terms_of_service_url: Option<String>,
    #[serde(default)]
    default_prompt: Option<RawPluginManifestDefaultPrompt>,
    #[serde(default)]
    brand_color: Option<String>,
    #[serde(default)]
    composer_icon: Option<String>,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    logo_dark: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestDefaultPrompt {
    String(String),
    List(Vec<RawPluginManifestDefaultPromptEntry>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestDefaultPromptEntry {
    String(String),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestPaths {
    Path(String),
    Paths(Vec<String>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestMcpServers {
    Path(String),
    Object(std::collections::BTreeMap<String, JsonValue>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestHooks {
    Path(String),
    Paths(Vec<String>),
    Inline(HooksFile),
    InlineList(Vec<HooksFile>),
    Invalid(JsonValue),
}

/// Loads a plugin manifest from the local host filesystem.
pub fn load_plugin_manifest(plugin_root: &Path) -> Option<PluginManifest> {
    let manifest_path = find_plugin_manifest_path(plugin_root)?;
    let contents = fs::read_to_string(&manifest_path).ok()?;
    match parse_plugin_manifest(plugin_root, &manifest_path, &contents) {
        Ok(manifest) => Some(manifest),
        Err(err) => {
            tracing::warn!(
                path = %manifest_path.display(),
                "failed to parse plugin manifest: {err}"
            );
            None
        }
    }
}

pub(crate) fn parse_plugin_manifest(
    plugin_root: &Path,
    manifest_path: &Path,
    contents: &str,
) -> Result<PluginManifest, serde_json::Error> {
    let plugin_root_uri =
        PathUri::from_host_native_path(plugin_root).map_err(serde_json::Error::io)?;
    let manifest_path_uri =
        PathUri::from_host_native_path(manifest_path).map_err(serde_json::Error::io)?;
    parse_plugin_manifest_uri(&plugin_root_uri, &manifest_path_uri, contents)?
        .try_map_resources(|path| path.to_abs_path().map_err(serde_json::Error::io))
}

pub(crate) fn parse_plugin_manifest_uri(
    plugin_root: &PathUri,
    manifest_path: &PathUri,
    contents: &str,
) -> Result<UriPluginManifest, serde_json::Error> {
    let RawPluginManifest {
        name: raw_name,
        version,
        description,
        keywords,
        skills,
        mcp_servers,
        apps,
        hooks,
        interface,
    } = serde_json::from_str::<RawPluginManifest>(contents)?;
    let name = plugin_root
        .basename()
        .filter(|_| raw_name.trim().is_empty())
        .unwrap_or(raw_name);
    let manifest_path_for_warning = manifest_path.to_string();
    let version = version.and_then(|version| {
        let version = version.trim();
        (!version.is_empty()).then(|| version.to_string())
    });
    let interface = interface.and_then(|interface| {
        let RawPluginManifestInterface {
            display_name,
            short_description,
            long_description,
            developer_name,
            category,
            capabilities,
            website_url,
            privacy_policy_url,
            terms_of_service_url,
            default_prompt,
            brand_color,
            composer_icon,
            logo,
            logo_dark,
            screenshots,
        } = interface;

        let interface = codex_plugin::manifest::PluginManifestInterface {
            display_name,
            short_description,
            long_description,
            developer_name,
            category,
            capabilities,
            website_url,
            privacy_policy_url,
            terms_of_service_url,
            default_prompt: resolve_default_prompts(
                &manifest_path_for_warning,
                default_prompt.as_ref(),
            ),
            brand_color,
            composer_icon: resolve_interface_asset_path(
                plugin_root,
                "interface.composerIcon",
                composer_icon.as_deref(),
            ),
            logo: resolve_interface_asset_path(plugin_root, "interface.logo", logo.as_deref()),
            logo_dark: resolve_interface_asset_path(
                plugin_root,
                "interface.logoDark",
                logo_dark.as_deref(),
            ),
            screenshots: screenshots
                .iter()
                .filter_map(|screenshot| {
                    resolve_interface_asset_path(
                        plugin_root,
                        "interface.screenshots",
                        Some(screenshot),
                    )
                })
                .collect(),
        };

        let has_fields = interface.display_name.is_some()
            || interface.short_description.is_some()
            || interface.long_description.is_some()
            || interface.developer_name.is_some()
            || interface.category.is_some()
            || !interface.capabilities.is_empty()
            || interface.website_url.is_some()
            || interface.privacy_policy_url.is_some()
            || interface.terms_of_service_url.is_some()
            || interface.default_prompt.is_some()
            || interface.brand_color.is_some()
            || interface.composer_icon.is_some()
            || interface.logo.is_some()
            || interface.logo_dark.is_some()
            || !interface.screenshots.is_empty();

        has_fields.then_some(interface)
    });
    Ok(codex_plugin::manifest::PluginManifest {
        name,
        version,
        description,
        keywords,
        paths: codex_plugin::manifest::PluginManifestPaths {
            skills: resolve_manifest_paths(plugin_root, "skills", skills.as_ref()),
            mcp_servers: resolve_manifest_mcp_servers(plugin_root, mcp_servers),
            apps: resolve_manifest_path(plugin_root, "apps", apps.as_deref()),
            hooks: resolve_manifest_hooks(plugin_root, hooks),
        },
        interface,
    })
}

fn resolve_manifest_hooks(
    plugin_root: &PathUri,
    hooks: Option<RawPluginManifestHooks>,
) -> Option<codex_plugin::manifest::PluginManifestHooks<PathUri>> {
    match hooks? {
        RawPluginManifestHooks::Path(path) => {
            resolve_manifest_path(plugin_root, "hooks", Some(&path))
                .map(|path| codex_plugin::manifest::PluginManifestHooks::Paths(vec![path]))
        }
        RawPluginManifestHooks::Paths(paths) => {
            let hooks = paths
                .iter()
                .filter_map(|path| resolve_manifest_path(plugin_root, "hooks", Some(path)))
                .collect::<Vec<_>>();
            (!hooks.is_empty()).then_some(codex_plugin::manifest::PluginManifestHooks::Paths(hooks))
        }
        RawPluginManifestHooks::Inline(hooks) => {
            Some(codex_plugin::manifest::PluginManifestHooks::Inline(vec![
                hooks,
            ]))
        }
        RawPluginManifestHooks::InlineList(hooks) => (!hooks.is_empty())
            .then_some(codex_plugin::manifest::PluginManifestHooks::Inline(hooks)),
        RawPluginManifestHooks::Invalid(value) => {
            tracing::warn!(
                "ignoring hooks: expected a string, string array, object, or object array; found {}",
                json_value_type(&value)
            );
            None
        }
    }
}

fn resolve_manifest_mcp_servers(
    plugin_root: &PathUri,
    mcp_servers: Option<RawPluginManifestMcpServers>,
) -> Option<codex_plugin::manifest::PluginManifestMcpServers<PathUri>> {
    match mcp_servers? {
        RawPluginManifestMcpServers::Path(path) => {
            resolve_manifest_path(plugin_root, "mcpServers", Some(&path))
                .map(codex_plugin::manifest::PluginManifestMcpServers::Path)
        }
        RawPluginManifestMcpServers::Object(servers) => match serde_json::to_string(&servers) {
            Ok(servers) => Some(codex_plugin::manifest::PluginManifestMcpServers::Object(
                servers,
            )),
            Err(err) => {
                tracing::warn!("ignoring mcpServers: failed to serialize object: {err}");
                None
            }
        },
        RawPluginManifestMcpServers::Invalid(value) => {
            tracing::warn!(
                "ignoring mcpServers: expected a string or object; found {}",
                json_value_type(&value)
            );
            None
        }
    }
}

fn resolve_interface_asset_path(
    plugin_root: &PathUri,
    field: &'static str,
    path: Option<&str>,
) -> Option<PathUri> {
    resolve_manifest_path(plugin_root, field, path)
}

fn resolve_default_prompts(
    manifest_path: &str,
    value: Option<&RawPluginManifestDefaultPrompt>,
) -> Option<Vec<String>> {
    match value? {
        RawPluginManifestDefaultPrompt::String(prompt) => {
            resolve_default_prompt_str(manifest_path, "interface.defaultPrompt", prompt)
                .map(|prompt| vec![prompt])
        }
        RawPluginManifestDefaultPrompt::List(values) => {
            let mut prompts = Vec::new();
            for (index, item) in values.iter().enumerate() {
                if prompts.len() >= MAX_DEFAULT_PROMPT_COUNT {
                    warn_invalid_default_prompt(
                        manifest_path,
                        "interface.defaultPrompt",
                        &format!("maximum of {MAX_DEFAULT_PROMPT_COUNT} prompts is supported"),
                    );
                    break;
                }

                match item {
                    RawPluginManifestDefaultPromptEntry::String(prompt) => {
                        let field = format!("interface.defaultPrompt[{index}]");
                        if let Some(prompt) =
                            resolve_default_prompt_str(manifest_path, &field, prompt)
                        {
                            prompts.push(prompt);
                        }
                    }
                    RawPluginManifestDefaultPromptEntry::Invalid(value) => {
                        let field = format!("interface.defaultPrompt[{index}]");
                        warn_invalid_default_prompt(
                            manifest_path,
                            &field,
                            &format!("expected a string, found {}", json_value_type(value)),
                        );
                    }
                }
            }

            (!prompts.is_empty()).then_some(prompts)
        }
        RawPluginManifestDefaultPrompt::Invalid(value) => {
            warn_invalid_default_prompt(
                manifest_path,
                "interface.defaultPrompt",
                &format!(
                    "expected a string or array of strings, found {}",
                    json_value_type(value)
                ),
            );
            None
        }
    }
}

fn resolve_default_prompt_str(manifest_path: &str, field: &str, prompt: &str) -> Option<String> {
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        warn_invalid_default_prompt(manifest_path, field, "prompt must not be empty");
        return None;
    }
    if prompt.chars().count() > MAX_DEFAULT_PROMPT_LEN {
        warn_invalid_default_prompt(
            manifest_path,
            field,
            &format!("prompt must be at most {MAX_DEFAULT_PROMPT_LEN} characters"),
        );
        return None;
    }
    Some(prompt)
}

fn warn_invalid_default_prompt(manifest_path: &str, field: &str, message: &str) {
    tracing::warn!(path = %manifest_path, "ignoring {field}: {message}");
}

fn json_value_type(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn resolve_manifest_paths(
    plugin_root: &PathUri,
    field: &'static str,
    paths: Option<&RawPluginManifestPaths>,
) -> Vec<PathUri> {
    match paths {
        Some(RawPluginManifestPaths::Path(path)) => {
            resolve_manifest_path(plugin_root, field, Some(path))
                .map(|path| vec![path])
                .unwrap_or_default()
        }
        Some(RawPluginManifestPaths::Paths(paths)) => paths
            .iter()
            .filter_map(|path| resolve_manifest_path(plugin_root, field, Some(path)))
            .collect(),
        Some(RawPluginManifestPaths::Invalid(value)) => {
            tracing::warn!(
                "ignoring {field}: expected a string or string array; found {}",
                json_value_type(value)
            );
            Vec::new()
        }
        None => Vec::new(),
    }
}

fn resolve_manifest_path(
    plugin_root: &PathUri,
    field: &'static str,
    path: Option<&str>,
) -> Option<PathUri> {
    let path = path?;
    if path.is_empty() {
        return None;
    }
    let Some(relative_path) = path.strip_prefix("./") else {
        tracing::warn!("ignoring {field}: path must start with `./` relative to plugin root");
        return None;
    };
    if relative_path.is_empty() {
        tracing::warn!("ignoring {field}: path must not be `./`");
        return None;
    }

    let convention = plugin_root.infer_path_convention();
    let has_parent_component = match convention {
        Some(PathConvention::Windows) => relative_path
            .split(['/', '\\'])
            .any(|component| component == ".."),
        Some(PathConvention::Posix) | None => {
            relative_path.split('/').any(|component| component == "..")
        }
    };
    if has_parent_component {
        tracing::warn!("ignoring {field}: path must not contain '..'");
        return None;
    }

    let has_windows_root = convention == Some(PathConvention::Windows)
        && (relative_path.starts_with('\\')
            || matches!(relative_path.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic()));
    if relative_path.starts_with('/') || has_windows_root {
        tracing::warn!("ignoring {field}: path must stay within the plugin root");
        return None;
    }

    let resolved = match plugin_root.join(relative_path) {
        Ok(resolved) => resolved,
        Err(err) => {
            tracing::warn!("ignoring {field}: path must resolve under plugin root: {err}");
            return None;
        }
    };
    if !resolved.starts_with(plugin_root) {
        tracing::warn!("ignoring {field}: path must stay within the plugin root");
        return None;
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::MAX_DEFAULT_PROMPT_LEN;
    use super::PluginManifest;
    use super::load_plugin_manifest;
    use codex_exec_server::EnvironmentManager;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_plugin::PluginProvider;
    use codex_plugin::ResolvedPlugin;
    use codex_plugin::manifest::PluginManifest as GenericPluginManifest;
    use codex_plugin::manifest::PluginManifestHooks;
    use codex_plugin::manifest::PluginManifestInterface;
    use codex_plugin::manifest::PluginManifestMcpServers;
    use codex_plugin::manifest::PluginManifestPaths;
    use codex_protocol::capabilities::CapabilityRootLocation;
    use codex_protocol::capabilities::SelectedCapabilityRoot;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    use crate::ExecutorPluginProvider;

    const ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".claude-plugin/plugin.json";

    fn write_manifest(plugin_root: &Path, version: Option<&str>, interface: &str) {
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
        let version = version
            .map(|version| format!("  \"version\": \"{version}\",\n"))
            .unwrap_or_default();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(
                r#"{{
  "name": "demo-plugin",
{version}
  "interface": {interface}
}}"#
            ),
        )
        .expect("write manifest");
    }

    fn write_alternate_plugin_manifest(plugin_root: &Path, contents: &str) {
        let manifest_path = plugin_root.join(ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH);
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
            .expect("create manifest dir");
        fs::write(manifest_path, contents).expect("write manifest");
    }

    fn load_manifest(plugin_root: &Path) -> PluginManifest {
        load_plugin_manifest(plugin_root).expect("load plugin manifest")
    }

    #[test]
    fn plugin_interface_accepts_legacy_default_prompt_string() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            /*version*/ None,
            r#"{
    "displayName": "Demo Plugin",
    "defaultPrompt": "  Summarize   my inbox  "
  }"#,
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(
            interface.default_prompt,
            Some(vec!["Summarize my inbox".to_string()])
        );
    }

    #[test]
    fn plugin_interface_normalizes_default_prompt_array() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let too_long = "x".repeat(MAX_DEFAULT_PROMPT_LEN + 1);
        write_manifest(
            &plugin_root,
            /*version*/ None,
            &format!(
                r#"{{
    "displayName": "Demo Plugin",
    "defaultPrompt": [
      " Summarize my inbox ",
      123,
      "{too_long}",
      "   ",
      "Draft the reply  ",
      "Find   my next action",
      "Archive old mail"
    ]
  }}"#
            ),
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(
            interface.default_prompt,
            Some(vec![
                "Summarize my inbox".to_string(),
                "Draft the reply".to_string(),
                "Find my next action".to_string(),
            ])
        );
    }

    #[test]
    fn plugin_interface_ignores_invalid_default_prompt_shape() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            /*version*/ None,
            r#"{
    "displayName": "Demo Plugin",
    "defaultPrompt": { "text": "Summarize my inbox" }
  }"#,
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(interface.default_prompt, None);
    }

    #[test]
    fn plugin_interface_reads_dark_logo_path() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            /*version*/ None,
            r#"{
    "logoDark": "./assets/logo-dark.svg"
  }"#,
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(
            interface.logo_dark,
            Some(
                AbsolutePathBuf::from_absolute_path_checked(
                    plugin_root.join("assets/logo-dark.svg"),
                )
                .expect("absolute dark logo path")
            )
        );
    }

    #[test]
    fn plugin_manifest_reads_trimmed_version() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            Some(" 1.2.3-beta+7 "),
            r#"{
    "displayName": "Demo Plugin"
  }"#,
        );

        let manifest = load_manifest(&plugin_root);

        assert_eq!(manifest.version, Some("1.2.3-beta+7".to_string()));
    }

    #[test]
    fn plugin_manifest_reads_keywords() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{
  "name": "demo-plugin",
  "keywords": ["api-key", "developer tools"]
}"#,
        )
        .expect("write manifest");

        let manifest = load_manifest(&plugin_root);

        assert_eq!(
            manifest.keywords,
            vec!["api-key".to_string(), "developer tools".to_string()]
        );
    }

    #[test]
    fn plugin_manifest_uses_alternate_discoverable_path() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_alternate_plugin_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "version": " 2.0.0 ",
  "interface": {
    "displayName": "Fallback Plugin"
  }
}"#,
        );

        let manifest = load_manifest(&plugin_root);

        assert_eq!(manifest.version, Some("2.0.0".to_string()));
        assert_eq!(
            manifest
                .interface
                .as_ref()
                .and_then(|interface| interface.display_name.as_deref()),
            Some("Fallback Plugin")
        );
    }

    #[test]
    fn uri_manifest_uses_the_root_path_convention() {
        let windows_root =
            PathUri::parse("file:///C:/plugins/demo-plugin").expect("Windows plugin root URI");
        let posix_root =
            PathUri::parse("file:///plugins/demo-plugin").expect("POSIX plugin root URI");
        let composer_icon = r"./assets\..\icon.svg";

        assert_eq!(parse_uri_composer_icon(&windows_root, composer_icon), None);
        assert_eq!(
            parse_uri_composer_icon(&posix_root, composer_icon),
            Some(
                posix_root
                    .join(r"assets\..\icon.svg")
                    .expect("composer icon URI")
            )
        );
    }

    fn parse_uri_composer_icon(plugin_root: &PathUri, composer_icon: &str) -> Option<PathUri> {
        let manifest_path = plugin_root
            .join(".codex-plugin/plugin.json")
            .expect("manifest URI");
        let composer_icon_json =
            serde_json::to_string(composer_icon).expect("serialize composer icon");
        let contents = format!(
            r#"{{
  "name": "demo-plugin",
  "interface": {{
    "displayName": "Demo Plugin",
    "composerIcon": {composer_icon_json}
  }}
}}"#
        );
        super::parse_plugin_manifest_uri(plugin_root, &manifest_path, &contents)
            .expect("URI manifest")
            .interface
            .and_then(|interface| interface.composer_icon)
    }

    #[tokio::test]
    async fn host_and_executor_sources_parse_the_same_manifest() {
        let temp_dir = tempdir().expect("tempdir");
        let plugin_root = temp_dir.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            Some(" 1.2.3 "),
            r#"{
    "displayName": "Demo Plugin",
    "composerIcon": "./assets/icon.svg"
  }"#,
        );
        let plugin_root =
            AbsolutePathBuf::from_absolute_path_checked(plugin_root).expect("absolute plugin root");
        let plugin_root_uri = PathUri::from_abs_path(&plugin_root);
        let provider =
            ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));
        let selected_root = SelectedCapabilityRoot {
            id: "selected-demo".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                path: plugin_root_uri.clone(),
            },
        };

        let executor_plugin = provider
            .resolve(&selected_root)
            .await
            .expect("resolve executor plugin")
            .expect("plugin descriptor");
        let manifest_path = plugin_root_uri
            .join(".codex-plugin/plugin.json")
            .expect("manifest URI");
        let manifest_contents =
            fs::read_to_string(plugin_root.join(".codex-plugin/plugin.json")).expect("manifest");
        let expected_manifest =
            super::parse_plugin_manifest_uri(&plugin_root_uri, &manifest_path, &manifest_contents)
                .expect("URI manifest");
        let expected_plugin = ResolvedPlugin::from_environment(
            "selected-demo".to_string(),
            LOCAL_ENVIRONMENT_ID.to_string(),
            plugin_root_uri,
            manifest_path,
            expected_manifest,
        )
        .expect("valid expected descriptor");

        assert_eq!(executor_plugin, expected_plugin);
    }

    #[test]
    fn uri_manifest_resolves_resources_below_foreign_root() {
        let plugin_root =
            PathUri::parse("file:///C:/plugins/demo-plugin").expect("plugin root URI");
        let manifest_path = plugin_root
            .join(".codex-plugin/plugin.json")
            .expect("manifest URI");
        let manifest = super::parse_plugin_manifest_uri(
            &plugin_root,
            &manifest_path,
            r#"{
  "name": "demo-plugin",
  "skills": "./skills",
  "mcpServers": "./.mcp.json",
  "apps": "./apps",
  "hooks": "./hooks.json",
  "interface": {
    "displayName": "Demo Plugin",
    "composerIcon": "./assets/icon.svg"
  }
}"#,
        )
        .expect("URI manifest");

        assert_eq!(
            manifest,
            GenericPluginManifest {
                name: "demo-plugin".to_string(),
                version: None,
                description: None,
                keywords: Vec::new(),
                paths: PluginManifestPaths {
                    skills: vec![plugin_root.join("skills").expect("skills URI")],
                    mcp_servers: Some(PluginManifestMcpServers::Path(
                        plugin_root.join(".mcp.json").expect("MCP URI"),
                    )),
                    apps: Some(plugin_root.join("apps").expect("apps URI")),
                    hooks: Some(PluginManifestHooks::Paths(vec![
                        plugin_root.join("hooks.json").expect("hooks URI"),
                    ])),
                },
                interface: Some(PluginManifestInterface {
                    display_name: Some("Demo Plugin".to_string()),
                    composer_icon: Some(
                        plugin_root
                            .join("assets/icon.svg")
                            .expect("composer icon URI"),
                    ),
                    ..PluginManifestInterface::default()
                }),
            }
        );
    }
}
