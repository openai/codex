use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigDetectOptions {
    pub include_home: bool,
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalAgentConfigMigrationItemType {
    Config,
    Skills,
    AgentsMd,
    McpServerConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigMigrationItem {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone)]
pub struct ExternalAgentConfigService {
    codex_home: PathBuf,
    claude_home: PathBuf,
}

impl ExternalAgentConfigService {
    pub fn new(codex_home: PathBuf) -> Self {
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

    pub fn detect(
        &self,
        params: ExternalAgentConfigDetectOptions,
    ) -> io::Result<Vec<ExternalAgentConfigMigrationItem>> {
        let mut items = Vec::new();
        if params.include_home {
            self.detect_migrations(None, &mut items)?;
        }

        for cwd in params.cwds.as_deref().unwrap_or(&[]) {
            let Some(repo_root) = find_repo_root(Some(cwd))? else {
                continue;
            };
            self.detect_migrations(Some(&repo_root), &mut items)?;
        }

        Ok(items)
    }

    pub fn import(&self, migration_items: Vec<ExternalAgentConfigMigrationItem>) -> io::Result<()> {
        for migration_item in migration_items {
            match migration_item.item_type {
                ExternalAgentConfigMigrationItemType::Config => {
                    self.import_config(migration_item.cwd.as_deref())?
                }
                ExternalAgentConfigMigrationItemType::Skills => {
                    self.import_skills(migration_item.cwd.as_deref())?
                }
                ExternalAgentConfigMigrationItemType::AgentsMd => {
                    self.import_agents_md(migration_item.cwd.as_deref())?
                }
                ExternalAgentConfigMigrationItemType::McpServerConfig => {}
            }
        }

        Ok(())
    }

    fn detect_migrations(
        &self,
        repo_root: Option<&Path>,
        items: &mut Vec<ExternalAgentConfigMigrationItem>,
    ) -> io::Result<()> {
        let cwd = repo_root.map(Path::to_path_buf);
        let source_settings = repo_root.map_or_else(
            || self.claude_home.join("settings.json"),
            |repo_root| repo_root.join(".claude").join("settings.json"),
        );
        let target_config = repo_root.map_or_else(
            || self.codex_home.join("config.toml"),
            |repo_root| repo_root.join(".codex").join("config.toml"),
        );
        if source_settings.is_file() {
            let raw_settings = fs::read_to_string(&source_settings)?;
            let settings: JsonValue = serde_json::from_str(&raw_settings)
                .map_err(|err| invalid_data_error(err.to_string()))?;
            let migrated = build_config_from_external(&settings)?;
            if !is_empty_toml_table(&migrated) {
                let mut should_include = true;
                if target_config.exists() {
                    let existing_raw = fs::read_to_string(&target_config)?;
                    let mut existing = if existing_raw.trim().is_empty() {
                        TomlValue::Table(Default::default())
                    } else {
                        toml::from_str::<TomlValue>(&existing_raw).map_err(|err| {
                            invalid_data_error(format!("invalid existing config.toml: {err}"))
                        })?
                    };
                    should_include = merge_missing_toml_values(&mut existing, &migrated)?;
                }

                if should_include {
                    items.push(ExternalAgentConfigMigrationItem {
                        item_type: ExternalAgentConfigMigrationItemType::Config,
                        description: format!(
                            "Migrate {} into {}.",
                            source_settings.display(),
                            target_config.display()
                        ),
                        cwd: cwd.clone(),
                    });
                }
            }
        }

        let source_skills = repo_root.map_or_else(
            || self.claude_home.join("skills"),
            |repo_root| repo_root.join(".claude").join("skills"),
        );
        let target_skills = repo_root.map_or_else(
            || self.home_target_skills_dir(),
            |repo_root| repo_root.join(".agents").join("skills"),
        );
        let source_skill_names = collect_subdirectory_names(&source_skills)?;
        let target_skill_names = collect_subdirectory_names(&target_skills)?;
        if source_skill_names
            .iter()
            .any(|skill_name| !target_skill_names.contains(skill_name))
        {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: format!(
                    "Copy skill folders from {} to {}.",
                    source_skills.display(),
                    target_skills.display()
                ),
                cwd: cwd.clone(),
            });
        }

        let source_agents_md = repo_root.map_or_else(
            || self.claude_home.join("CLAUDE.md"),
            |repo_root| repo_root.join("CLAUDE.md"),
        );
        let target_agents_md = repo_root.map_or_else(
            || self.codex_home.join("AGENTS.md"),
            |repo_root| repo_root.join("AGENTS.md"),
        );
        if source_agents_md.is_file() && is_missing_or_empty_text_file(&target_agents_md)? {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Import {} to {}.",
                    source_agents_md.display(),
                    target_agents_md.display()
                ),
                cwd,
            });
        }

        Ok(())
    }

    fn home_target_skills_dir(&self) -> PathBuf {
        self.codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"))
    }

    fn import_config(&self, cwd: Option<&Path>) -> io::Result<()> {
        let (source_settings, target_config) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(".claude").join("settings.json"),
                repo_root.join(".codex").join("config.toml"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(());
        } else {
            (
                self.claude_home.join("settings.json"),
                self.codex_home.join("config.toml"),
            )
        };
        if !source_settings.is_file() {
            return Ok(());
        }

        let raw_settings = fs::read_to_string(&source_settings)?;
        let settings: JsonValue = serde_json::from_str(&raw_settings)
            .map_err(|err| invalid_data_error(err.to_string()))?;
        let migrated = build_config_from_external(&settings)?;
        if is_empty_toml_table(&migrated) {
            return Ok(());
        }

        let Some(target_parent) = target_config.parent() else {
            return Err(invalid_data_error("config target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;
        if !target_config.exists() {
            write_toml_file(&target_config, &migrated)?;
            return Ok(());
        }

        let existing_raw = fs::read_to_string(&target_config)?;
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

        write_toml_file(&target_config, &existing)?;
        Ok(())
    }

    fn import_skills(&self, cwd: Option<&Path>) -> io::Result<()> {
        let (source_skills, target_skills) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(".claude").join("skills"),
                repo_root.join(".agents").join("skills"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(());
        } else {
            (
                self.claude_home.join("skills"),
                self.home_target_skills_dir(),
            )
        };
        if !source_skills.is_dir() {
            return Ok(());
        }

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

    fn import_agents_md(&self, cwd: Option<&Path>) -> io::Result<()> {
        let (source_agents_md, target_agents_md) = if let Some(repo_root) = find_repo_root(cwd)? {
            (repo_root.join("CLAUDE.md"), repo_root.join("AGENTS.md"))
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(());
        } else {
            (
                self.claude_home.join("CLAUDE.md"),
                self.codex_home.join("AGENTS.md"),
            )
        };
        if !source_agents_md.is_file() || !is_missing_or_empty_text_file(&target_agents_md)? {
            return Ok(());
        }

        let Some(target_parent) = target_agents_md.parent() else {
            return Err(invalid_data_error("AGENTS.md target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;

        rewrite_and_copy_text_file(&source_agents_md, &target_agents_md)
    }
}

fn default_claude_home() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return PathBuf::from(home).join(".claude");
    }

    PathBuf::from(".claude")
}

fn find_repo_root(cwd: Option<&Path>) -> io::Result<Option<PathBuf>> {
    let Some(cwd) = cwd.filter(|cwd| !cwd.as_os_str().is_empty()) else {
        return Ok(None);
    };

    let mut current = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        std::env::current_dir()?.join(cwd)
    };

    if !current.exists() {
        return Ok(None);
    }

    if current.is_file() {
        let Some(parent) = current.parent() else {
            return Ok(None);
        };
        current = parent.to_path_buf();
    }

    let fallback = current.clone();
    loop {
        let git_path = current.join(".git");
        if git_path.is_dir() || git_path.is_file() {
            return Ok(Some(current));
        }
        if !current.pop() {
            break;
        }
    }

    Ok(Some(fallback))
}

fn collect_subdirectory_names(path: &Path) -> io::Result<HashSet<OsString>> {
    let mut names = HashSet::new();
    if !path.is_dir() {
        return Ok(names);
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.insert(entry.file_name());
        }
    }

    Ok(names)
}

fn is_missing_or_empty_text_file(path: &Path) -> io::Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    if !path.is_file() {
        return Ok(false);
    }

    Ok(fs::read_to_string(path)?.trim().is_empty())
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
            if is_skill_md(&source_path) {
                rewrite_and_copy_text_file(&source_path, &target_path)?;
            } else {
                fs::copy(source_path, target_path)?;
            }
        }
    }

    Ok(())
}

fn is_skill_md(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
}

fn rewrite_and_copy_text_file(source: &Path, target: &Path) -> io::Result<()> {
    let source_contents = fs::read_to_string(source)?;
    let rewritten = rewrite_claude_terms(&source_contents);
    fs::write(target, rewritten)
}

fn rewrite_claude_terms(content: &str) -> String {
    let mut rewritten = replace_case_insensitive_with_boundaries(content, "claude.md", "AGENTS.md");
    for from in [
        "claude code",
        "claude-code",
        "claude_code",
        "claudecode",
        "claude",
    ] {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, from, "Codex");
    }
    rewritten
}

fn replace_case_insensitive_with_boundaries(
    input: &str,
    needle: &str,
    replacement: &str,
) -> String {
    let needle_lower = needle.to_ascii_lowercase();
    if needle_lower.is_empty() {
        return input.to_string();
    }

    let haystack_lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = haystack_lower[search_start..].find(&needle_lower) {
        let start = search_start + relative_pos;
        let end = start + needle_lower.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_byte(bytes[end]);

        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }

        search_start = start + 1;
    }

    if last_emitted == 0 {
        return input.to_string();
    }

    output.push_str(&input[last_emitted..]);
    output
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn build_config_from_external(settings: &JsonValue) -> io::Result<TomlValue> {
    let Some(settings_obj) = settings.as_object() else {
        return Err(invalid_data_error(
            "external agent settings root must be an object",
        ));
    };

    let mut root = toml::map::Map::new();

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

    if let Some(sandbox_enabled) = settings_obj
        .get("sandbox")
        .and_then(JsonValue::as_object)
        .and_then(|sandbox| sandbox.get("enabled"))
        .and_then(JsonValue::as_bool)
        && sandbox_enabled
    {
        root.insert(
            "sandbox_mode".to_string(),
            TomlValue::String("workspace-write".to_string()),
        );
    }

    Ok(TomlValue::Table(root))
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

fn is_empty_toml_table(value: &TomlValue) -> bool {
    match value {
        TomlValue::Table(table) => table.is_empty(),
        TomlValue::String(_)
        | TomlValue::Integer(_)
        | TomlValue::Float(_)
        | TomlValue::Boolean(_)
        | TomlValue::Datetime(_)
        | TomlValue::Array(_) => false,
    }
}

fn invalid_data_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
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

    fn service_for_paths(claude_home: PathBuf, codex_home: PathBuf) -> ExternalAgentConfigService {
        ExternalAgentConfigService::new_for_test(codex_home, claude_home)
    }

    #[test]
    fn detect_home_lists_config_skills_and_agents_md() {
        let (_root, claude_home, codex_home) = fixture_paths();
        let agents_skills = codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
        fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
        fs::write(claude_home.join("CLAUDE.md"), "claude rules").expect("write claude md");
        fs::write(
            claude_home.join("settings.json"),
            r#"{"model":"claude","env":{"FOO":"bar"}}"#,
        )
        .expect("write settings");

        let items = service_for_paths(claude_home.clone(), codex_home.clone())
            .detect(ExternalAgentConfigDetectOptions {
                include_home: true,
                cwds: None,
            })
            .expect("detect");

        let expected = vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description: format!(
                    "Migrate {} into {}.",
                    claude_home.join("settings.json").display(),
                    codex_home.join("config.toml").display()
                ),
                cwd: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: format!(
                    "Copy skill folders from {} to {}.",
                    claude_home.join("skills").display(),
                    agents_skills.display()
                ),
                cwd: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Import {} to {}.",
                    claude_home.join("CLAUDE.md").display(),
                    codex_home.join("AGENTS.md").display()
                ),
                cwd: None,
            },
        ];

        assert_eq!(items, expected);
    }

    #[test]
    fn detect_repo_lists_agents_md_for_each_cwd() {
        let root = TempDir::new().expect("create tempdir");
        let repo_root = root.path().join("repo");
        let nested = repo_root.join("nested").join("child");
        fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
        fs::create_dir_all(&nested).expect("create nested");
        fs::write(repo_root.join("CLAUDE.md"), "Claude code guidance").expect("write source");

        let items = service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
            .detect(ExternalAgentConfigDetectOptions {
                include_home: false,
                cwds: Some(vec![nested, repo_root.clone()]),
            })
            .expect("detect");

        let expected = vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Import {} to {}.",
                    repo_root.join("CLAUDE.md").display(),
                    repo_root.join("AGENTS.md").display(),
                ),
                cwd: Some(repo_root.clone()),
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Import {} to {}.",
                    repo_root.join("CLAUDE.md").display(),
                    repo_root.join("AGENTS.md").display(),
                ),
                cwd: Some(repo_root),
            },
        ];

        assert_eq!(items, expected);
    }

    #[test]
    fn import_home_migrates_supported_config_fields_skills_and_agents_md() {
        let (_root, claude_home, codex_home) = fixture_paths();
        let agents_skills = codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
        fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
        fs::write(
            claude_home.join("settings.json"),
            r#"{"model":"claude","permissions":{"ask":["git push"]},"env":{"FOO":"bar"},"sandbox":{"enabled":true,"network":{"allowLocalBinding":true}}}"#,
        )
        .expect("write settings");
        fs::write(
            claude_home.join("skills").join("skill-a").join("SKILL.md"),
            "Use Claude Code and CLAUDE utilities.",
        )
        .expect("write skill");
        fs::write(claude_home.join("CLAUDE.md"), "Claude code guidance").expect("write agents");

        service_for_paths(claude_home, codex_home.clone())
            .import(vec![
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                    description: String::new(),
                    cwd: None,
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Config,
                    description: String::new(),
                    cwd: None,
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Skills,
                    description: String::new(),
                    cwd: None,
                },
            ])
            .expect("import");

        assert_eq!(
            fs::read_to_string(codex_home.join("AGENTS.md")).expect("read agents"),
            "Codex guidance"
        );

        let parsed_config: TomlValue = toml::from_str(
            &fs::read_to_string(codex_home.join("config.toml")).expect("read config"),
        )
        .expect("parse config");
        let expected_config: TomlValue = toml::from_str(
            r#"
            sandbox_mode = "workspace-write"

            [shell_environment_policy]
            inherit = "core"

            [shell_environment_policy.set]
            FOO = "bar"
            "#,
        )
        .expect("parse expected");
        assert_eq!(parsed_config, expected_config);
        assert_eq!(
            fs::read_to_string(agents_skills.join("skill-a").join("SKILL.md"))
                .expect("read copied skill"),
            "Use Codex and Codex utilities."
        );
    }

    #[test]
    fn import_home_skips_empty_config_migration() {
        let (_root, claude_home, codex_home) = fixture_paths();
        fs::create_dir_all(&claude_home).expect("create claude home");
        fs::write(
            claude_home.join("settings.json"),
            r#"{"model":"claude","sandbox":{"enabled":false}}"#,
        )
        .expect("write settings");

        service_for_paths(claude_home, codex_home.clone())
            .import(vec![ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description: String::new(),
                cwd: None,
            }])
            .expect("import");

        assert!(!codex_home.join("config.toml").exists());
    }

    #[test]
    fn detect_home_skips_config_when_target_already_has_supported_fields() {
        let (_root, claude_home, codex_home) = fixture_paths();
        fs::create_dir_all(&claude_home).expect("create claude home");
        fs::create_dir_all(&codex_home).expect("create codex home");
        fs::write(
            claude_home.join("settings.json"),
            r#"{"env":{"FOO":"bar"},"sandbox":{"enabled":true}}"#,
        )
        .expect("write settings");
        fs::write(
            codex_home.join("config.toml"),
            r#"
            sandbox_mode = "workspace-write"

            [shell_environment_policy]
            inherit = "core"

            [shell_environment_policy.set]
            FOO = "bar"
            "#,
        )
        .expect("write config");

        let items = service_for_paths(claude_home, codex_home)
            .detect(ExternalAgentConfigDetectOptions {
                include_home: true,
                cwds: None,
            })
            .expect("detect");

        assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
    }

    #[test]
    fn detect_home_skips_skills_when_all_skill_directories_exist() {
        let (_root, claude_home, codex_home) = fixture_paths();
        let agents_skills = codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
        fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create source");
        fs::create_dir_all(agents_skills.join("skill-a")).expect("create target");

        let items = service_for_paths(claude_home, codex_home)
            .detect(ExternalAgentConfigDetectOptions {
                include_home: true,
                cwds: None,
            })
            .expect("detect");

        assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
    }

    #[test]
    fn import_repo_agents_md_rewrites_terms_and_skips_non_empty_targets() {
        let root = TempDir::new().expect("create tempdir");
        let repo_root = root.path().join("repo-a");
        let repo_with_existing_target = root.path().join("repo-b");
        fs::create_dir_all(repo_root.join(".git")).expect("create git");
        fs::create_dir_all(repo_with_existing_target.join(".git")).expect("create git");
        fs::write(
            repo_root.join("CLAUDE.md"),
            "Claude code\nclaude\nCLAUDE-CODE\nSee CLAUDE.md\n",
        )
        .expect("write source");
        fs::write(repo_with_existing_target.join("CLAUDE.md"), "new source").expect("write source");
        fs::write(
            repo_with_existing_target.join("AGENTS.md"),
            "keep existing target",
        )
        .expect("write target");

        service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
            .import(vec![
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                    description: String::new(),
                    cwd: Some(repo_root.clone()),
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                    description: String::new(),
                    cwd: Some(repo_with_existing_target.clone()),
                },
            ])
            .expect("import");

        assert_eq!(
            fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
            "Codex\nCodex\nCodex\nSee AGENTS.md\n"
        );
        assert_eq!(
            fs::read_to_string(repo_with_existing_target.join("AGENTS.md"))
                .expect("read existing target"),
            "keep existing target"
        );
    }

    #[test]
    fn import_repo_agents_md_overwrites_empty_targets() {
        let root = TempDir::new().expect("create tempdir");
        let repo_root = root.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).expect("create git");
        fs::write(repo_root.join("CLAUDE.md"), "Claude code guidance").expect("write source");
        fs::write(repo_root.join("AGENTS.md"), " \n\t").expect("write empty target");

        service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
            .import(vec![ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: Some(repo_root.clone()),
            }])
            .expect("import");

        assert_eq!(
            fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
            "Codex guidance"
        );
    }
}
