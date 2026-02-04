use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use codex_core::SESSIONS_SUBDIR;
use codex_core::config::Config;
use codex_core::mcp::collect_mcp_snapshot;
use serde::Serialize;
use serde_json::Value;
use tokio::fs;

use crate::connectors::AppInfo;
use crate::connectors::connector_display_label;

const APPS_SUBDIR: &str = "apps";
const DEFAULT_PATH_COMPONENT: &str = "item";

#[derive(Debug, Clone)]
struct AppTool {
    name: String,
    qualified_name: String,
    title: Option<String>,
    description: Option<String>,
    input_schema: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolFile<'a> {
    app_id: &'a str,
    app_name: String,
    tool_name: &'a str,
    qualified_tool_name: &'a str,
    tool_title: Option<&'a str>,
    description: Option<&'a str>,
    input_schema: &'a Value,
}

pub async fn materialize_apps_to_filesystem(
    config: &Config,
    apps: &[AppInfo],
) -> anyhow::Result<()> {
    let apps_root = config.codex_home.join(SESSIONS_SUBDIR).join(APPS_SUBDIR);
    let tools_by_app = collect_tools_by_app(config, apps).await;
    write_mapping(&apps_root, apps, &tools_by_app).await
}

async fn collect_tools_by_app(config: &Config, apps: &[AppInfo]) -> HashMap<String, Vec<AppTool>> {
    let snapshot = collect_mcp_snapshot(config).await;
    let app_ids: HashSet<&str> = apps.iter().map(|app| app.id.as_str()).collect();
    let mut tools_by_app: HashMap<String, Vec<AppTool>> = HashMap::new();

    for (qualified_name, tool) in snapshot.tools {
        let Some(connector_id) = connector_id_from_meta(tool.meta.as_ref()) else {
            continue;
        };
        if !app_ids.contains(connector_id.as_str()) {
            continue;
        }

        tools_by_app.entry(connector_id).or_default().push(AppTool {
            name: tool.name,
            qualified_name,
            title: tool.title,
            description: tool.description,
            input_schema: tool.input_schema,
        });
    }

    for tools in tools_by_app.values_mut() {
        tools.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.qualified_name.cmp(&right.qualified_name))
        });
    }

    tools_by_app
}

async fn write_mapping(
    apps_root: &Path,
    apps: &[AppInfo],
    tools_by_app: &HashMap<String, Vec<AppTool>>,
) -> anyhow::Result<()> {
    if fs::try_exists(apps_root).await? {
        fs::remove_dir_all(apps_root).await?;
    }
    fs::create_dir_all(apps_root).await?;

    let mut dir_name_counts: HashMap<String, usize> = HashMap::new();
    for app in apps {
        let Some(tools) = tools_by_app.get(&app.id) else {
            continue;
        };
        if tools.is_empty() {
            continue;
        }

        let app_name = connector_display_label(app);
        let dir_name = next_unique_path_component(&mut dir_name_counts, &app_name);
        let app_dir = apps_root.join(dir_name);
        fs::create_dir_all(&app_dir)
            .await
            .with_context(|| format!("create app directory {}", app_dir.display()))?;

        let mut file_stem_counts: HashMap<String, usize> = HashMap::new();
        for tool in tools {
            let preferred_tool_name = tool
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&tool.name);
            let file_stem = next_unique_path_component(&mut file_stem_counts, preferred_tool_name);
            let tool_path = app_dir.join(format!("{file_stem}.tool"));
            let contents = serde_json::to_string_pretty(&ToolFile {
                app_id: &app.id,
                app_name: app_name.clone(),
                tool_name: &tool.name,
                qualified_tool_name: &tool.qualified_name,
                tool_title: tool.title.as_deref(),
                description: tool.description.as_deref(),
                input_schema: &tool.input_schema,
            })
            .context("serialize app tool mapping")?;

            fs::write(&tool_path, format!("{contents}\n"))
                .await
                .with_context(|| format!("write tool file {}", tool_path.display()))?;
        }
    }

    Ok(())
}

fn connector_id_from_meta(meta: Option<&Value>) -> Option<String> {
    let meta = meta?.as_object()?;
    for key in ["connector_id", "connectorId"] {
        if let Some(value) = meta.get(key).and_then(Value::as_str) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn next_unique_path_component(stem_counts: &mut HashMap<String, usize>, value: &str) -> String {
    let base = sanitize_path_component(value);
    let count = stem_counts.entry(base.clone()).or_insert(0);
    *count += 1;
    let ordinal = *count;
    if ordinal == 1 {
        base
    } else {
        format!("{base}_{ordinal}")
    }
}

fn sanitize_path_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut previous_underscore = false;

    for character in value.trim().chars() {
        let mapped = if character.is_ascii_alphanumeric() {
            character.to_ascii_lowercase()
        } else if matches!(character, '-' | '_' | '.') {
            character
        } else {
            '_'
        };

        if mapped == '_' && previous_underscore {
            continue;
        }

        previous_underscore = mapped == '_';
        sanitized.push(mapped);
    }

    while sanitized.starts_with('_') {
        sanitized.remove(0);
    }
    while sanitized.ends_with('_') {
        let _ = sanitized.pop();
    }

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        DEFAULT_PATH_COMPONENT.to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn app(id: &str, name: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: true,
        }
    }

    #[test]
    fn extracts_connector_id_from_tool_meta() {
        let snake_case = json!({
            "connector_id": "  connector_a  "
        });
        let camel_case = json!({
            "connectorId": "connector_b"
        });
        let missing = json!({
            "other": "value"
        });

        assert_eq!(
            connector_id_from_meta(Some(&snake_case)),
            Some("connector_a".to_string())
        );
        assert_eq!(
            connector_id_from_meta(Some(&camel_case)),
            Some("connector_b".to_string())
        );
        assert_eq!(connector_id_from_meta(Some(&missing)), None);
        assert_eq!(connector_id_from_meta(None), None);
    }

    #[test]
    fn sanitizes_path_components() {
        assert_eq!(sanitize_path_component("mail/send"), "mail_send");
        assert_eq!(sanitize_path_component(".."), "item");
        assert_eq!(sanitize_path_component("  hi there  "), "hi_there");
        assert_eq!(sanitize_path_component("__weird__"), "weird");
        assert_eq!(sanitize_path_component("Upper Case"), "upper_case");
    }

    #[tokio::test]
    async fn writes_app_directories_and_tool_files() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let apps_root = temp_dir.path().join("apps");
        let apps = vec![
            app("connector_mail", "Mail App"),
            app("connector_docs", "Docs App"),
        ];

        let mut tools_by_app = HashMap::new();
        tools_by_app.insert(
            "connector_mail".to_string(),
            vec![
                AppTool {
                    name: "send/email".to_string(),
                    qualified_name: "mcp__codex_apps__send_email".to_string(),
                    title: Some("Send Email".to_string()),
                    description: Some("Sends an email".to_string()),
                    input_schema: json!({"type": "object"}),
                },
                AppTool {
                    name: "send email".to_string(),
                    qualified_name: "mcp__codex_apps__send_email_v2".to_string(),
                    title: None,
                    description: None,
                    input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}}),
                },
            ],
        );

        write_mapping(&apps_root, &apps, &tools_by_app).await?;

        assert!(apps_root.join("mail_app").is_dir());
        assert!(!apps_root.join("connector_mail").exists());
        assert!(!apps_root.join("docs_app").exists());
        assert!(apps_root.join("mail_app/send_email.tool").is_file());
        assert!(apps_root.join("mail_app/send_email_2.tool").is_file());

        let first_tool: Value = serde_json::from_str(
            &fs::read_to_string(apps_root.join("mail_app/send_email.tool")).await?,
        )?;
        assert_eq!(
            first_tool,
            json!({
                "appId": "connector_mail",
                "appName": "Mail App",
                "toolName": "send/email",
                "qualifiedToolName": "mcp__codex_apps__send_email",
                "toolTitle": "Send Email",
                "description": "Sends an email",
                "inputSchema": {
                    "type": "object"
                }
            })
        );

        Ok(())
    }
}
