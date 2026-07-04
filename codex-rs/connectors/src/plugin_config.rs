use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginAppFile {
    #[serde(default)]
    apps: IndexMap<String, PluginAppConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct PluginAppConfig {
    id: String,
    category: Option<String>,
}

/// Parses connector declarations from a plugin app configuration file.
pub fn parse_plugin_app_config(contents: &str) -> serde_json::Result<Vec<AppDeclaration>> {
    serde_json::from_str(contents).map(app_declarations_from_file)
}

/// Parses connector declarations from an already-decoded plugin app configuration.
pub fn parse_plugin_app_config_value(value: Value) -> serde_json::Result<Vec<AppDeclaration>> {
    serde_json::from_value(value).map(app_declarations_from_file)
}

fn app_declarations_from_file(parsed: PluginAppFile) -> Vec<AppDeclaration> {
    parsed
        .apps
        .into_iter()
        .map(|(name, app)| AppDeclaration {
            name,
            connector_id: AppConnectorId(app.id),
            category: cleaned_category(app.category),
        })
        .collect()
}

fn cleaned_category(category: Option<String>) -> Option<String> {
    category
        .map(|category| category.trim().to_string())
        .filter(|category| !category.is_empty())
}

#[cfg(test)]
#[path = "plugin_config_tests.rs"]
mod tests;
