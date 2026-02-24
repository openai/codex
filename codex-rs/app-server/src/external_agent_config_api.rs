use crate::error_code::INTERNAL_ERROR_CODE;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportParams;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::JSONRPCErrorError;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigApi {
    codex_home: PathBuf,
    claude_home: PathBuf,
}

impl ExternalAgentConfigApi {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        let claude_home = default_claude_home();
        Self {
            codex_home,
            claude_home,
        }
    }

    #[cfg(test)]
    fn new_for_test(codex_home: PathBuf, claude_home: PathBuf) -> Self {
        Self {
            codex_home,
            claude_home,
        }
    }

    pub(crate) async fn detect(
        &self,
    ) -> Result<ExternalAgentConfigDetectResponse, JSONRPCErrorError> {
        let mut items = Vec::new();
        let claude_md = self.claude_home.join("CLAUDE.md");
        let agents_md = self.codex_home.join("AGENTS.md");
        if claude_md.is_file() && !agents_md.exists() {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!("Import {} to {}.", claude_md.display(), agents_md.display()),
            });
        }

        let settings_json = self.claude_home.join("settings.json");
        // TODO: check if the migratable fields exists in the settings.json and the mapped config.toml fields are empty
        if settings_json.is_file() {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description: format!(
                    "Migrate {} into {}.",
                    settings_json.display(),
                    self.codex_home.join("config.toml").display()
                ),
            });
        }

        let claude_skills = self.claude_home.join("skills");
        if has_subdirectories(&claude_skills).map_err(map_io_error)? {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: format!(
                    "Copy skill folders from {} to {}.",
                    claude_skills.display(),
                    self.codex_home.join("skills").display()
                ),
            });
        }

        //TODO: add MCP server config detection

        Ok(ExternalAgentConfigDetectResponse { items })
    }

    pub(crate) async fn import(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> Result<ExternalAgentConfigImportResponse, JSONRPCErrorError> {
        let mut seen = HashSet::new();

        for migration_item_type in params.migration_item_types {
            if !seen.insert(migration_item_type) {
                continue;
            }

            match migration_item_type {
                ExternalAgentConfigMigrationItemType::AgentsMd => self.import_agents_md(),
                ExternalAgentConfigMigrationItemType::Config => self.import_config(),
                ExternalAgentConfigMigrationItemType::Skills => self.import_skills(),
                ExternalAgentConfigMigrationItemType::McpServerConfig => Ok(()),
            }
            .map_err(map_io_error)?;
        }

        Ok(ExternalAgentConfigImportResponse {})
    }

    fn import_agents_md(&self) -> io::Result<()> {
        let source = self.claude_home.join("CLAUDE.md");
        if !source.is_file() {
            return Ok(());
        }

        let target = self.codex_home.join("AGENTS.md");
        if target.exists() {
            return Ok(());
        }

        fs::create_dir_all(&self.codex_home)?;
        fs::copy(source, target)?;
        Ok(())
    }

    fn import_config(&self) -> io::Result<()> {
        let source = self.claude_home.join("settings.json");
        if !source.is_file() {
            return Ok(());
        }

        let raw_settings = fs::read_to_string(&source)?;
        let settings: JsonValue = serde_json::from_str(&raw_settings)
            .map_err(|err| invalid_data_error(err.to_string()))?;
        let migrated = build_config_from_external(&settings, &source)?;

        fs::create_dir_all(&self.codex_home)?;
        let target = self.codex_home.join("config.toml");
        if !target.exists() {
            write_toml_file(&target, &migrated)?;
            return Ok(());
        }

        let existing_raw = fs::read_to_string(&target)?;
        let mut existing = if existing_raw.trim().is_empty() {
            TomlValue::Table(Default::default())
        } else {
            toml::from_str::<TomlValue>(&existing_raw)
                .map_err(|err| invalid_data_error(format!("invalid existing config.toml: {err}")))?
        };

        let changed = merge_missing_toml_values(&mut existing, &migrated)?;
        if !changed {
            return Ok(());
        }

        write_toml_file(&target, &existing)?;
        Ok(())
    }

    fn import_skills(&self) -> io::Result<()> {
        let source_skills = self.claude_home.join("skills");
        if !source_skills.is_dir() {
            return Ok(());
        }

        let target_skills = self.codex_home.join("skills");
        fs::create_dir_all(&target_skills)?;

        for entry in fs::read_dir(&source_skills)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }

            let target = target_skills.join(entry.file_name());
            if target.exists() {
                continue;
            }

            copy_dir_recursive(&entry.path(), &target)?;
        }

        Ok(())
    }
}

fn default_claude_home() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return PathBuf::from(home).join(".claude");
    }

    PathBuf::from(".claude")
}

fn has_subdirectories(path: &Path) -> io::Result<bool> {
    if !path.is_dir() {
        return Ok(false);
    }

    for entry in fs::read_dir(path)? {
        if entry?.file_type()?.is_dir() {
            return Ok(true);
        }
    }

    Ok(false)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
            continue;
        }

        if file_type.is_file() {
            fs::copy(source_path, target_path)?;
        }
    }

    Ok(())
}

fn build_config_from_external(settings: &JsonValue, settings_path: &Path) -> io::Result<TomlValue> {
    let Some(settings_obj) = settings.as_object() else {
        return Err(invalid_data_error(
            "external agent settings root must be an object",
        ));
    };

    let mut root = toml::map::Map::new();

    if let Some(permissions) = settings_obj
        .get("permissions")
        .and_then(JsonValue::as_object)
    {
        if let Some(approval_policy) = derive_approval_policy(permissions) {
            root.insert(
                "approval_policy".to_string(),
                TomlValue::String(approval_policy.to_string()),
            );
        }
    }

    if let Some(env) = settings_obj.get("env").and_then(JsonValue::as_object)
        && !env.is_empty()
    {
        let mut shell_policy = toml::map::Map::new();
        shell_policy.insert("inherit".to_string(), TomlValue::String("core".to_string()));
        shell_policy.insert(
            "set".to_string(),
            TomlValue::Table(json_object_to_toml_table(env)?),
        );
        root.insert(
            "shell_environment_policy".to_string(),
            TomlValue::Table(shell_policy),
        );
    }

    if let Some(sandbox) = settings_obj.get("sandbox").and_then(JsonValue::as_object) {
        let sandbox_enabled = sandbox
            .get("enabled")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        root.insert(
            "sandbox_mode".to_string(),
            TomlValue::String(if sandbox_enabled {
                "workspace-write".to_string()
            } else {
                "danger-full-access".to_string()
            }),
        );

        if sandbox_enabled
            && let Some(network) = sandbox.get("network").and_then(JsonValue::as_object)
        {
            let network_access = network
                .get("allowLocalBinding")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
                || network
                    .get("allowUnixSockets")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false)
                || network
                    .get("httpProxyPort")
                    .and_then(JsonValue::as_i64)
                    .is_some()
                || network
                    .get("socksProxyPort")
                    .and_then(JsonValue::as_i64)
                    .is_some();
            let mut sandbox_write = toml::map::Map::new();
            sandbox_write.insert(
                "network_access".to_string(),
                TomlValue::Boolean(network_access),
            );
            root.insert(
                "sandbox_workspace_write".to_string(),
                TomlValue::Table(sandbox_write),
            );
        }
    }

    let mut migrated = toml::map::Map::new();
    migrated.insert(
        "original_settings_path".to_string(),
        TomlValue::String(settings_path.display().to_string()),
    );
    if let Some(original_model) = settings_obj.get("model").and_then(JsonValue::as_str) {
        migrated.insert(
            "original_model".to_string(),
            TomlValue::String(original_model.to_string()),
        );
    }

    if let Some(company_announcements) = settings_obj.get("companyAnnouncements") {
        migrated.insert(
            "company_announcements".to_string(),
            json_to_toml_value(company_announcements)?,
        );
    }

    if let Some(permissions) = settings_obj
        .get("permissions")
        .and_then(JsonValue::as_object)
    {
        let permissions_table = json_object_to_toml_table(permissions)?;
        if !permissions_table.is_empty() {
            migrated.insert(
                "permissions".to_string(),
                TomlValue::Table(permissions_table),
            );
        }
    }

    let mut mcp_hints = toml::map::Map::new();
    for key in [
        "enableAllProjectMcpServers",
        "enabledMcpjsonServers",
        "disabledMcpjsonServers",
    ] {
        if let Some(value) = settings_obj.get(key) {
            mcp_hints.insert(to_snake_case(key), json_to_toml_value(value)?);
        }
    }
    if let Some(allowed) = settings_obj
        .get("allowedMcpServers")
        .and_then(JsonValue::as_array)
    {
        let names = allowed
            .iter()
            .filter_map(JsonValue::as_object)
            .filter_map(|obj| obj.get("serverName"))
            .filter_map(JsonValue::as_str)
            .map(|s| TomlValue::String(s.to_string()))
            .collect::<Vec<_>>();
        if !names.is_empty() {
            mcp_hints.insert("allowed_mcp_servers".to_string(), TomlValue::Array(names));
        }
    }
    if let Some(denied) = settings_obj
        .get("deniedMcpServers")
        .and_then(JsonValue::as_array)
    {
        let names = denied
            .iter()
            .filter_map(JsonValue::as_object)
            .filter_map(|obj| obj.get("serverName"))
            .filter_map(JsonValue::as_str)
            .map(|s| TomlValue::String(s.to_string()))
            .collect::<Vec<_>>();
        if !names.is_empty() {
            mcp_hints.insert("denied_mcp_servers".to_string(), TomlValue::Array(names));
        }
    }
    if !mcp_hints.is_empty() {
        migrated.insert("mcp".to_string(), TomlValue::Table(mcp_hints));
    }

    root.insert(
        "migrated_from_external_agent".to_string(),
        TomlValue::Table(migrated),
    );
    Ok(TomlValue::Table(root))
}

fn derive_approval_policy(
    permissions: &serde_json::Map<String, JsonValue>,
) -> Option<&'static str> {
    let ask_non_empty = permissions
        .get("ask")
        .and_then(JsonValue::as_array)
        .is_some_and(|v| !v.is_empty());
    let deny_non_empty = permissions
        .get("deny")
        .and_then(JsonValue::as_array)
        .is_some_and(|v| !v.is_empty());
    let allow_non_empty = permissions
        .get("allow")
        .and_then(JsonValue::as_array)
        .is_some_and(|v| !v.is_empty());
    let additional_dirs_non_empty = permissions
        .get("additionalDirectories")
        .and_then(JsonValue::as_array)
        .is_some_and(|v| !v.is_empty());
    let other_fields_present = permissions.contains_key("defaultMode")
        || permissions.contains_key("disableBypassPermissionsMode");
    let any_present = ask_non_empty
        || deny_non_empty
        || allow_non_empty
        || additional_dirs_non_empty
        || other_fields_present;

    if !any_present {
        return None;
    }
    if ask_non_empty {
        return Some("on-request");
    }
    if deny_non_empty {
        return Some("untrusted");
    }
    Some("never")
}

fn json_object_to_toml_table(
    object: &serde_json::Map<String, JsonValue>,
) -> io::Result<toml::map::Map<String, TomlValue>> {
    let mut table = toml::map::Map::new();
    for (key, value) in object {
        table.insert(key.clone(), json_to_toml_value(value)?);
    }
    Ok(table)
}

fn json_to_toml_value(value: &JsonValue) -> io::Result<TomlValue> {
    match value {
        JsonValue::Null => Ok(TomlValue::String("null".to_string())),
        JsonValue::Bool(v) => Ok(TomlValue::Boolean(*v)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                return Ok(TomlValue::Integer(i));
            }
            if let Some(f) = n.as_f64() {
                return Ok(TomlValue::Float(f));
            }
            Err(invalid_data_error("unsupported JSON number"))
        }
        JsonValue::String(v) => Ok(TomlValue::String(v.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(json_to_toml_value)
            .collect::<io::Result<Vec<_>>>()
            .map(TomlValue::Array),
        JsonValue::Object(map) => json_object_to_toml_table(map).map(TomlValue::Table),
    }
}

fn merge_missing_toml_values(existing: &mut TomlValue, incoming: &TomlValue) -> io::Result<bool> {
    match (existing, incoming) {
        (TomlValue::Table(existing_table), TomlValue::Table(incoming_table)) => {
            let mut changed = false;
            for (key, incoming_value) in incoming_table {
                match existing_table.get_mut(key) {
                    Some(existing_value) => {
                        if matches!(
                            (&*existing_value, incoming_value),
                            (TomlValue::Table(_), TomlValue::Table(_))
                        ) && merge_missing_toml_values(existing_value, incoming_value)?
                        {
                            changed = true;
                        }
                    }
                    None => {
                        existing_table.insert(key.clone(), incoming_value.clone());
                        changed = true;
                    }
                }
            }
            Ok(changed)
        }
        _ => Err(invalid_data_error(
            "expected TOML table while merging migrated config values",
        )),
    }
}

fn write_toml_file(path: &Path, value: &TomlValue) -> io::Result<()> {
    let serialized = toml::to_string_pretty(value)
        .map_err(|err| invalid_data_error(format!("failed to serialize config.toml: {err}")))?;
    fs::write(path, format!("{serialized}\n"))
}

fn invalid_data_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn to_snake_case(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for (i, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn map_io_error(err: io::Error) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: err.to_string(),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn fixture_paths() -> (TempDir, PathBuf, PathBuf) {
        let root = TempDir::new().expect("create tempdir");
        let claude_home = root.path().join(".claude");
        let codex_home = root.path().join(".codex");
        (root, claude_home, codex_home)
    }

    fn api_for_paths(claude_home: PathBuf, codex_home: PathBuf) -> ExternalAgentConfigApi {
        ExternalAgentConfigApi::new_for_test(codex_home, claude_home)
    }

    #[tokio::test]
    async fn detect_lists_supported_migrations() {
        let (_root, claude_home, codex_home) = fixture_paths();
        fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
        fs::write(claude_home.join("CLAUDE.md"), "claude rules").expect("write claude md");
        fs::write(claude_home.join("settings.json"), "{\"model\":\"claude\"}")
            .expect("write settings");
        fs::write(
            claude_home.join("skills").join("skill-a").join("SKILL.md"),
            "# skill",
        )
        .expect("write skill");

        let response = api_for_paths(claude_home.clone(), codex_home.clone())
            .detect()
            .await
            .expect("detect");

        let expected = ExternalAgentConfigDetectResponse {
            items: vec![
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                    description: format!(
                        "Import {} to {} (skips if destination already exists).",
                        claude_home.join("CLAUDE.md").display(),
                        codex_home.join("AGENTS.md").display()
                    ),
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Config,
                    description: format!(
                        "Migrate {} into {} (creates file if missing; only appends missing Codex fields).",
                        claude_home.join("settings.json").display(),
                        codex_home.join("config.toml").display()
                    ),
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Skills,
                    description: format!(
                        "Copy skill folders from {} to {} (existing skill names are skipped).",
                        claude_home.join("skills").display(),
                        codex_home.join("skills").display()
                    ),
                },
            ],
        };

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn import_copies_agents_md_migrates_config_and_skills() {
        let (_root, claude_home, codex_home) = fixture_paths();
        fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
        fs::write(claude_home.join("CLAUDE.md"), "claude rules").expect("write claude md");
        fs::write(
            claude_home.join("settings.json"),
            r#"{"model":"claude","permissions":{"ask":["git push"]},"env":{"FOO":"bar"},"sandbox":{"enabled":true}}"#,
        )
        .expect("write settings");
        fs::write(
            claude_home.join("skills").join("skill-a").join("SKILL.md"),
            "# skill",
        )
        .expect("write skill");

        let response = api_for_paths(claude_home.clone(), codex_home.clone())
            .import(ExternalAgentConfigImportParams {
                migration_item_types: vec![
                    ExternalAgentConfigMigrationItemType::AgentsMd,
                    ExternalAgentConfigMigrationItemType::Config,
                    ExternalAgentConfigMigrationItemType::Skills,
                ],
            })
            .await
            .expect("import");

        assert_eq!(response, ExternalAgentConfigImportResponse {});
        assert_eq!(
            fs::read_to_string(codex_home.join("AGENTS.md")).expect("read agents"),
            "claude rules"
        );
        let config_contents =
            fs::read_to_string(codex_home.join("config.toml")).expect("read config");
        assert!(config_contents.contains("model = \"gpt-5.1-codex-max\""));
        assert!(config_contents.contains("model_reasoning_effort = \"medium\""));
        assert!(config_contents.contains("approval_policy = \"on-request\""));
        assert!(config_contents.contains("[migrated_from_claude]"));
        assert!(config_contents.contains("original_model = \"claude\""));
        assert_eq!(
            fs::read_to_string(codex_home.join("skills").join("skill-a").join("SKILL.md"))
                .expect("read copied skill"),
            "# skill"
        );
    }

    #[tokio::test]
    async fn import_skips_existing_targets() {
        let (_root, claude_home, codex_home) = fixture_paths();
        fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
        fs::create_dir_all(codex_home.join("skills").join("skill-a")).expect("create existing");
        fs::write(claude_home.join("CLAUDE.md"), "new").expect("write claude md");
        fs::write(codex_home.join("AGENTS.md"), "existing").expect("write agents md");
        fs::write(
            claude_home.join("settings.json"),
            r#"{"model":"claude","permissions":{"ask":["x"]},"env":{"FOO":"bar"}}"#,
        )
        .expect("write settings");
        fs::write(
            codex_home.join("config.toml"),
            concat!(
                "model = \"keep-model\"\n",
                "\n",
                "[shell_environment_policy]\n",
                "inherit = \"all\"\n",
                "\n",
                "[shell_environment_policy.set]\n",
                "KEEP = \"yes\"\n"
            ),
        )
        .expect("write existing config");
        fs::write(
            claude_home.join("skills").join("skill-a").join("SKILL.md"),
            "# incoming",
        )
        .expect("write skill");

        let response = api_for_paths(claude_home, codex_home.clone())
            .import(ExternalAgentConfigImportParams {
                migration_item_types: vec![
                    ExternalAgentConfigMigrationItemType::AgentsMd,
                    ExternalAgentConfigMigrationItemType::Config,
                    ExternalAgentConfigMigrationItemType::Skills,
                ],
            })
            .await
            .expect("import");

        assert_eq!(response, ExternalAgentConfigImportResponse {});
        assert_eq!(
            fs::read_to_string(codex_home.join("AGENTS.md")).expect("read agents"),
            "existing"
        );
        let config_contents =
            fs::read_to_string(codex_home.join("config.toml")).expect("read config");
        assert!(config_contents.contains("model = \"keep-model\""));
        assert!(config_contents.contains("model_reasoning_effort = \"medium\""));
        assert!(config_contents.contains("approval_policy = \"on-request\""));
        assert!(config_contents.contains("inherit = \"all\""));
        assert!(config_contents.contains("KEEP = \"yes\""));
        assert!(config_contents.contains("[migrated_from_claude]"));
    }
}
