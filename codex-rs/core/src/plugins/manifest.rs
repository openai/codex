use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PLUGIN_MANIFEST_PATH;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Component;
use std::path::Path;
const MAX_DEFAULT_PROMPT_COUNT: usize = 3;
const MAX_DEFAULT_PROMPT_LEN: usize = 128;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    // Keep manifest paths as raw strings so we can validate the required `./...` syntax before
    // resolving them under the plugin root.
    #[serde(default)]
    skills: Option<String>,
    #[serde(default)]
    mcp_servers: Option<String>,
    #[serde(default)]
    apps: Option<String>,
    #[serde(default)]
    interface: Option<RawPluginManifestInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginManifest {
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) paths: PluginManifestPaths,
    pub(crate) interface: Option<PluginManifestInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifestPaths {
    pub skills: Option<AbsolutePathBuf>,
    pub mcp_servers: Option<AbsolutePathBuf>,
    pub apps: Option<AbsolutePathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginManifestInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub developer_name: Option<String>,
    pub category: Option<String>,
    pub capabilities: Vec<String>,
    pub website_url: Option<String>,
    pub privacy_policy_url: Option<String>,
    pub terms_of_service_url: Option<String>,
    pub default_prompt: Option<Vec<String>>,
    pub brand_color: Option<String>,
    pub composer_icon: Option<AbsolutePathBuf>,
    pub logo: Option<AbsolutePathBuf>,
    pub screenshots: Vec<AbsolutePathBuf>,
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

pub(crate) fn load_plugin_manifest(plugin_root: &Path) -> Option<PluginManifest> {
    let manifest_path = plugin_root.join(PLUGIN_MANIFEST_PATH);
    if !manifest_path.is_file() {
        return None;
    }
    let contents = fs::read_to_string(&manifest_path).ok()?;
    match serde_json::from_str::<RawPluginManifest>(&contents) {
        Ok(manifest) => {
            let RawPluginManifest {
                name: raw_name,
                version,
                description,
                skills,
                mcp_servers,
                apps,
                interface,
            } = manifest;
            let name = plugin_root
                .file_name()
                .and_then(|entry| entry.to_str())
                .filter(|_| raw_name.trim().is_empty())
                .unwrap_or(&raw_name)
                .to_string();
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
                    screenshots,
                } = interface;

                let interface = PluginManifestInterface {
                    display_name,
                    short_description,
                    long_description,
                    developer_name,
                    category,
                    capabilities,
                    website_url,
                    privacy_policy_url,
                    terms_of_service_url,
                    default_prompt: resolve_default_prompts(plugin_root, default_prompt.as_ref()),
                    brand_color,
                    composer_icon: resolve_interface_asset_path(
                        plugin_root,
                        "interface.composerIcon",
                        composer_icon.as_deref(),
                    ),
                    logo: resolve_interface_asset_path(
                        plugin_root,
                        "interface.logo",
                        logo.as_deref(),
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
                    || !interface.screenshots.is_empty();

                has_fields.then_some(interface)
            });
            Some(PluginManifest {
                name,
                version,
                description,
                paths: PluginManifestPaths {
                    skills: resolve_manifest_path(plugin_root, "skills", skills.as_deref()),
                    mcp_servers: resolve_manifest_path(
                        plugin_root,
                        "mcpServers",
                        mcp_servers.as_deref(),
                    ),
                    apps: resolve_manifest_path(plugin_root, "apps", apps.as_deref()),
                },
                interface,
            })
        }
        Err(err) => {
            tracing::warn!(
                path = %manifest_path.display(),
                "failed to parse plugin manifest: {err}"
            );
            None
        }
    }
}

fn resolve_interface_asset_path(
    plugin_root: &Path,
    field: &'static str,
    path: Option<&str>,
) -> Option<AbsolutePathBuf> {
    resolve_manifest_path(plugin_root, field, path)
}

fn resolve_default_prompts(
    plugin_root: &Path,
    value: Option<&RawPluginManifestDefaultPrompt>,
) -> Option<Vec<String>> {
    match value? {
        RawPluginManifestDefaultPrompt::String(prompt) => {
            resolve_default_prompt_str(plugin_root, "interface.defaultPrompt", prompt)
                .map(|prompt| vec![prompt])
        }
        RawPluginManifestDefaultPrompt::List(values) => {
            let mut prompts = Vec::new();
            for (index, item) in values.iter().enumerate() {
                if prompts.len() >= MAX_DEFAULT_PROMPT_COUNT {
                    warn_invalid_default_prompt(
                        plugin_root,
                        "interface.defaultPrompt",
                        &format!("maximum of {MAX_DEFAULT_PROMPT_COUNT} prompts is supported"),
                    );
                    break;
                }

                match item {
                    RawPluginManifestDefaultPromptEntry::String(prompt) => {
                        let field = format!("interface.defaultPrompt[{index}]");
                        if let Some(prompt) =
                            resolve_default_prompt_str(plugin_root, &field, prompt)
                        {
                            prompts.push(prompt);
                        }
                    }
                    RawPluginManifestDefaultPromptEntry::Invalid(value) => {
                        let field = format!("interface.defaultPrompt[{index}]");
                        warn_invalid_default_prompt(
                            plugin_root,
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
                plugin_root,
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

fn resolve_default_prompt_str(plugin_root: &Path, field: &str, prompt: &str) -> Option<String> {
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        warn_invalid_default_prompt(plugin_root, field, "prompt must not be empty");
        return None;
    }
    if prompt.chars().count() > MAX_DEFAULT_PROMPT_LEN {
        warn_invalid_default_prompt(
            plugin_root,
            field,
            &format!("prompt must be at most {MAX_DEFAULT_PROMPT_LEN} characters"),
        );
        return None;
    }
    Some(prompt)
}

fn warn_invalid_default_prompt(plugin_root: &Path, field: &str, message: &str) {
    let manifest_path = plugin_root.join(PLUGIN_MANIFEST_PATH);
    tracing::warn!(
        path = %manifest_path.display(),
        "ignoring {field}: {message}"
    );
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

fn resolve_manifest_path(
    plugin_root: &Path,
    field: &'static str,
    path: Option<&str>,
) -> Option<AbsolutePathBuf> {
    // `plugin.json` paths are required to be relative to the plugin root and we return the
    // normalized absolute path to the rest of the system.
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

    let mut normalized = std::path::PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                tracing::warn!("ignoring {field}: path must not contain '..'");
                return None;
            }
            _ => {
                tracing::warn!("ignoring {field}: path must stay within the plugin root");
                return None;
            }
        }
    }

    AbsolutePathBuf::try_from(plugin_root.join(normalized))
        .map_err(|err| {
            tracing::warn!("ignoring {field}: path must resolve to an absolute path: {err}");
            err
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::MAX_DEFAULT_PROMPT_LEN;
    use super::PluginManifest;
    use super::load_plugin_manifest;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

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
}
