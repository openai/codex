use crate::config::ConfigToml;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::path_utils::write_atomically;
use crate::rollout::ARCHIVED_SESSIONS_SUBDIR;
use crate::rollout::SESSIONS_SUBDIR;
use crate::rollout::list::ThreadListConfig;
use crate::rollout::list::ThreadListLayout;
use crate::rollout::list::ThreadSortKey;
use crate::rollout::list::get_threads_in_root;
use crate::state_db;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use dirs::home_dir;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use time::OffsetDateTime;
use toml_edit::Array as TomlArray;
use toml_edit::InlineTable;
use toml_edit::Item as TomlItem;
use toml_edit::value;

pub const CLAUDE_MIGRATION_STATE_RELATIVE_PATH: &str = "state/claude_migration_state.json";
pub const CLAUDE_MIGRATION_DEFAULT_NEW_USER_THREAD_THRESHOLD: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaudeMigrationMarkerState {
    Pending,
    Imported,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeMigrationHomeAvailable {
    pub marker_state: ClaudeMigrationMarkerState,
    pub prior_codex_thread_count: usize,
    pub detected: ClaudeHomeDetection,
    pub proposed: ClaudeHomeMigrationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeHomeDetection {
    pub claude_home_exists: bool,
    pub settings_json: bool,
    pub claude_md: bool,
    pub skills_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeMigrationRepoAvailable {
    pub marker_state: ClaudeMigrationMarkerState,
    pub repo_root: PathBuf,
    pub detected: ClaudeRepoDetection,
    pub proposed: ClaudeRepoMigrationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeRepoDetection {
    pub claude_md: bool,
    pub agents_md_exists: bool,
    pub mcp_json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeHomeMigrationStatus {
    SkippedAlreadyImported,
    SkippedNever,
    SkippedNoHomeDir,
    SkippedNoClaudeData,
    Applied(ClaudeHomeMigrationSummary),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeHomeMigrationSummary {
    pub imported_config_keys: Vec<String>,
    pub imported_skills: Vec<String>,
    pub imported_user_agents_md: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeRepoMigrationSummary {
    pub copied_agents_md: bool,
    pub imported_mcp_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeMigrationStateFile {
    schema_version: u32,
    state: ClaudeMigrationMarkerState,
    updated_at_unix: i64,
    last_result: Option<ClaudeMigrationLastResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClaudeMigrationResultScope {
    Home,
    Repo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeMigrationLastResult {
    scope: ClaudeMigrationResultScope,
    imported_config_keys: Vec<String>,
    copied_skills: Vec<String>,
    copied_agents_md: bool,
    imported_mcp_servers: Vec<String>,
}

pub async fn maybe_migrate_claude_home(
    codex_home: &Path,
    config_toml: &ConfigToml,
) -> io::Result<ClaudeHomeMigrationStatus> {
    let Some(user_home) = home_dir() else {
        return Ok(ClaudeHomeMigrationStatus::SkippedNoHomeDir);
    };
    maybe_migrate_claude_home_with_paths(codex_home, &user_home.join(".claude"), config_toml).await
}

pub async fn detect_claude_home_migration(
    codex_home: &Path,
    config_toml: &ConfigToml,
    default_provider: &str,
    max_prior_threads: usize,
) -> io::Result<Option<ClaudeMigrationHomeAvailable>> {
    let Some(user_home) = home_dir() else {
        return Ok(None);
    };
    detect_claude_home_migration_with_paths(
        codex_home,
        &user_home.join(".claude"),
        config_toml,
        default_provider,
        max_prior_threads,
    )
    .await
}

pub async fn apply_claude_home_migration(
    codex_home: &Path,
    config_toml: &ConfigToml,
) -> io::Result<ClaudeHomeMigrationSummary> {
    let Some(user_home) = home_dir() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "could not resolve home directory",
        ));
    };
    apply_claude_home_migration_with_paths(codex_home, &user_home.join(".claude"), config_toml)
        .await
}

pub async fn set_claude_home_migration_state(
    codex_home: &Path,
    state: ClaudeMigrationMarkerState,
) -> io::Result<()> {
    persist_import_state(
        &codex_home.join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH),
        state,
        None,
    )
    .await
}

pub async fn detect_claude_repo_migration(
    cwd: &Path,
) -> io::Result<Option<ClaudeMigrationRepoAvailable>> {
    let project_root = find_project_root(cwd);
    let marker_state = read_import_state(
        &project_root
            .join(".codex")
            .join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH),
    )
    .await?
    .unwrap_or(ClaudeMigrationMarkerState::Pending);
    if matches!(
        marker_state,
        ClaudeMigrationMarkerState::Imported | ClaudeMigrationMarkerState::Never
    ) {
        return Ok(None);
    }

    let preview = preview_repo_migration(&project_root).await?;
    let detected = ClaudeRepoDetection {
        claude_md: tokio::fs::try_exists(project_root.join("CLAUDE.md")).await?,
        agents_md_exists: tokio::fs::try_exists(project_root.join("AGENTS.md")).await?,
        mcp_json: tokio::fs::try_exists(project_root.join(".mcp.json")).await?,
    };
    let has_any = detected.claude_md || detected.mcp_json;
    let would_change = preview.copied_agents_md || !preview.imported_mcp_servers.is_empty();
    if !has_any {
        return Ok(None);
    }
    if !would_change {
        return Ok(None);
    }
    Ok(Some(ClaudeMigrationRepoAvailable {
        marker_state,
        repo_root: project_root,
        detected,
        proposed: preview,
    }))
}

pub async fn apply_claude_repo_migration(cwd: &Path) -> io::Result<ClaudeRepoMigrationSummary> {
    let project_root = find_project_root(cwd);
    let summary = apply_claude_repo_migration_at_root(&project_root).await?;
    persist_import_state(
        &project_root
            .join(".codex")
            .join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH),
        ClaudeMigrationMarkerState::Imported,
        Some(ClaudeMigrationLastResult {
            scope: ClaudeMigrationResultScope::Repo,
            imported_config_keys: Vec::new(),
            copied_skills: Vec::new(),
            copied_agents_md: summary.copied_agents_md,
            imported_mcp_servers: summary.imported_mcp_servers.clone(),
        }),
    )
    .await?;
    Ok(summary)
}

pub async fn set_claude_repo_migration_state(
    cwd: &Path,
    state: ClaudeMigrationMarkerState,
) -> io::Result<()> {
    let project_root = find_project_root(cwd);
    persist_import_state(
        &project_root
            .join(".codex")
            .join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH),
        state,
        None,
    )
    .await
}

async fn maybe_migrate_claude_home_with_paths(
    codex_home: &Path,
    claude_home: &Path,
    config_toml: &ConfigToml,
) -> io::Result<ClaudeHomeMigrationStatus> {
    match read_import_state(&codex_home.join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH)).await? {
        Some(ClaudeMigrationMarkerState::Imported) => {
            return Ok(ClaudeHomeMigrationStatus::SkippedAlreadyImported);
        }
        Some(ClaudeMigrationMarkerState::Never) => {
            return Ok(ClaudeHomeMigrationStatus::SkippedNever);
        }
        Some(ClaudeMigrationMarkerState::Pending) | None => {}
    }
    if !tokio::fs::try_exists(claude_home).await? {
        return Ok(ClaudeHomeMigrationStatus::SkippedNoClaudeData);
    }

    let summary =
        apply_claude_home_migration_with_paths(codex_home, claude_home, config_toml).await?;
    Ok(ClaudeHomeMigrationStatus::Applied(summary))
}

async fn detect_claude_home_migration_with_paths(
    codex_home: &Path,
    claude_home: &Path,
    config_toml: &ConfigToml,
    default_provider: &str,
    max_prior_threads: usize,
) -> io::Result<Option<ClaudeMigrationHomeAvailable>> {
    let marker_state = read_import_state(&codex_home.join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH))
        .await?
        .unwrap_or(ClaudeMigrationMarkerState::Pending);
    if matches!(
        marker_state,
        ClaudeMigrationMarkerState::Imported | ClaudeMigrationMarkerState::Never
    ) {
        return Ok(None);
    }
    if !tokio::fs::try_exists(claude_home).await? {
        return Ok(None);
    }

    let prior_codex_thread_count = count_prior_codex_threads_up_to_threshold(
        codex_home,
        default_provider,
        max_prior_threads.saturating_add(1),
    )
    .await?;
    if prior_codex_thread_count > max_prior_threads {
        return Ok(None);
    }

    let proposed = preview_home_migration(codex_home, claude_home, config_toml).await?;
    let detected = ClaudeHomeDetection {
        claude_home_exists: true,
        settings_json: tokio::fs::try_exists(claude_home.join("settings.json")).await?,
        claude_md: tokio::fs::try_exists(claude_home.join("CLAUDE.md")).await?,
        skills_count: count_claude_skills(claude_home).await?,
    };
    let would_change = proposed.imported_user_agents_md
        || !proposed.imported_config_keys.is_empty()
        || !proposed.imported_skills.is_empty();
    if !would_change {
        return Ok(None);
    }
    Ok(Some(ClaudeMigrationHomeAvailable {
        marker_state,
        prior_codex_thread_count,
        detected,
        proposed,
    }))
}

async fn apply_claude_home_migration_with_paths(
    codex_home: &Path,
    claude_home: &Path,
    config_toml: &ConfigToml,
) -> io::Result<ClaudeHomeMigrationSummary> {
    if !tokio::fs::try_exists(claude_home).await? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Claude config dir not found: {}", claude_home.display()),
        ));
    }

    let mut summary = ClaudeHomeMigrationSummary::default();
    let settings_path = claude_home.join("settings.json");
    if tokio::fs::try_exists(&settings_path).await? {
        let settings_contents = tokio::fs::read_to_string(&settings_path).await?;
        let settings_json: JsonValue = serde_json::from_str(&settings_contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let (edits, imported_keys) = collect_settings_edits(&settings_json, config_toml);
        if !edits.is_empty() {
            ConfigEditsBuilder::new(codex_home)
                .with_edits(edits)
                .apply()
                .await
                .map_err(|err| {
                    io::Error::other(format!("failed to persist imported Claude settings: {err}"))
                })?;
        }
        summary.imported_config_keys = imported_keys;
    }

    summary.imported_user_agents_md = copy_file_if_missing(
        &claude_home.join("CLAUDE.md"),
        &codex_home.join("AGENTS.md"),
    )
    .await?;
    summary.imported_skills =
        copy_skills_from_claude(&claude_home.join("skills"), &codex_home.join("skills")).await?;

    persist_import_state(
        &codex_home.join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH),
        ClaudeMigrationMarkerState::Imported,
        Some(ClaudeMigrationLastResult {
            scope: ClaudeMigrationResultScope::Home,
            imported_config_keys: summary.imported_config_keys.clone(),
            copied_skills: summary.imported_skills.clone(),
            copied_agents_md: summary.imported_user_agents_md,
            imported_mcp_servers: Vec::new(),
        }),
    )
    .await?;
    Ok(summary)
}

fn collect_settings_edits(
    settings_json: &JsonValue,
    config_toml: &ConfigToml,
) -> (Vec<ConfigEdit>, Vec<String>) {
    let mut edits = Vec::new();
    let mut imported_keys = Vec::new();

    if config_toml.model.is_none()
        && let Some(model) = find_json_string(
            settings_json,
            &[
                &["model"],
                &["defaultModel"],
                &["default_model"],
                &["modelName"],
                &["model_name"],
            ],
        )
    {
        edits.push(ConfigEdit::SetModel {
            model: Some(model),
            effort: None,
        });
        imported_keys.push("model".to_string());
    }

    if config_toml.approval_policy.is_none()
        && let Some(approval_policy) = find_json_string(
            settings_json,
            &[
                &["approvalPolicy"],
                &["approval_policy"],
                &["permissions", "approvalPolicy"],
                &["permissions", "approval_policy"],
                &["permissions", "approvalMode"],
                &["permissions", "approval_mode"],
            ],
        )
        && let Some(mapped_policy) = parse_approval_policy(&approval_policy)
    {
        edits.push(ConfigEdit::SetPath {
            segments: vec!["approval_policy".to_string()],
            value: value(mapped_policy.to_string()),
        });
        imported_keys.push("approval_policy".to_string());
    } else if config_toml.approval_policy.is_none()
        && let Some(derived_policy) = derive_approval_policy_from_permissions(settings_json)
    {
        edits.push(ConfigEdit::SetPath {
            segments: vec!["approval_policy".to_string()],
            value: value(derived_policy.to_string()),
        });
        imported_keys.push("approval_policy".to_string());
    }

    if config_toml.sandbox_mode.is_none()
        && let Some(mapped_mode) = find_json_string(
            settings_json,
            &[
                &["sandboxMode"],
                &["sandbox_mode"],
                &["permissions", "sandboxMode"],
                &["permissions", "sandbox_mode"],
            ],
        )
        .and_then(|mode| parse_sandbox_mode(&mode))
        .or_else(|| derive_sandbox_mode_from_settings(settings_json))
    {
        edits.push(ConfigEdit::SetPath {
            segments: vec!["sandbox_mode".to_string()],
            value: value(mapped_mode.to_string()),
        });
        imported_keys.push("sandbox_mode".to_string());
    }

    if config_toml.sandbox_workspace_write.is_none()
        && let Some(network_access) = derive_workspace_network_access(settings_json)
    {
        edits.push(ConfigEdit::SetPath {
            segments: vec![
                "sandbox_workspace_write".to_string(),
                "network_access".to_string(),
            ],
            value: value(network_access),
        });
        imported_keys.push("sandbox_workspace_write.network_access".to_string());
    }

    if config_toml.sandbox_workspace_write.is_none()
        && let Some(writable_roots) = extract_workspace_writable_roots(settings_json)
        && !writable_roots.is_empty()
    {
        let mut writable_roots_array = TomlArray::new();
        for root in writable_roots {
            writable_roots_array.push(root);
        }
        edits.push(ConfigEdit::SetPath {
            segments: vec![
                "sandbox_workspace_write".to_string(),
                "writable_roots".to_string(),
            ],
            value: TomlItem::Value(writable_roots_array.into()),
        });
        imported_keys.push("sandbox_workspace_write.writable_roots".to_string());
    }

    if config_toml.shell_environment_policy.r#set.is_none()
        && let Some(set_values) = extract_shell_environment_set(settings_json)
        && !set_values.is_empty()
    {
        let mut set_table = InlineTable::new();
        for (key, value_str) in set_values {
            set_table.insert(key, value_str.into());
        }
        edits.push(ConfigEdit::SetPath {
            segments: vec!["shell_environment_policy".to_string(), "set".to_string()],
            value: TomlItem::Value(set_table.into()),
        });
        imported_keys.push("shell_environment_policy.set".to_string());
    }

    (edits, imported_keys)
}

fn derive_approval_policy_from_permissions(settings_json: &JsonValue) -> Option<AskForApproval> {
    let permissions = settings_json.get("permissions")?.as_object()?;
    let ask_has_entries = permissions
        .get("ask")
        .and_then(JsonValue::as_array)
        .is_some_and(|entries| !entries.is_empty());
    let deny_has_entries = permissions
        .get("deny")
        .and_then(JsonValue::as_array)
        .is_some_and(|entries| !entries.is_empty());
    let allow_has_entries = permissions
        .get("allow")
        .and_then(JsonValue::as_array)
        .is_some_and(|entries| !entries.is_empty());
    let default_mode = permissions
        .get("defaultMode")
        .or_else(|| permissions.get("default_mode"))
        .and_then(JsonValue::as_str)
        .map(normalize_enum_token);
    let has_permission_signal =
        ask_has_entries || deny_has_entries || allow_has_entries || default_mode.is_some();

    if !has_permission_signal {
        return None;
    }
    if ask_has_entries {
        return Some(AskForApproval::OnRequest);
    }
    if deny_has_entries {
        return Some(AskForApproval::UnlessTrusted);
    }

    if let Some(mode) = default_mode
        && mode.contains("bypass")
    {
        return Some(AskForApproval::Never);
    }

    Some(AskForApproval::Never)
}

fn derive_sandbox_mode_from_settings(settings_json: &JsonValue) -> Option<SandboxMode> {
    let enabled = settings_json
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("enabled"))
        .and_then(JsonValue::as_bool)?;
    Some(if enabled {
        SandboxMode::WorkspaceWrite
    } else {
        SandboxMode::DangerFullAccess
    })
}

fn derive_workspace_network_access(settings_json: &JsonValue) -> Option<bool> {
    let network = settings_json
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("network"))
        .and_then(JsonValue::as_object)?;

    let any_enabled = network
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

    Some(any_enabled)
}

fn extract_workspace_writable_roots(settings_json: &JsonValue) -> Option<Vec<String>> {
    let directories = settings_json
        .get("permissions")
        .and_then(JsonValue::as_object)
        .and_then(|permissions| {
            permissions
                .get("additionalDirectories")
                .or_else(|| permissions.get("additional_directories"))
        })
        .and_then(JsonValue::as_array)?;

    let writable_roots = directories
        .iter()
        .filter_map(JsonValue::as_str)
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();

    Some(writable_roots)
}

fn extract_shell_environment_set(settings_json: &JsonValue) -> Option<BTreeMap<String, String>> {
    let env = settings_json.get("env").and_then(JsonValue::as_object)?;
    let mut set_values = BTreeMap::new();
    for (key, value) in env {
        match value {
            JsonValue::String(string) => {
                set_values.insert(key.to_string(), string.to_string());
            }
            JsonValue::Bool(boolean) => {
                set_values.insert(key.to_string(), boolean.to_string());
            }
            JsonValue::Number(number) => {
                set_values.insert(key.to_string(), number.to_string());
            }
            JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_) => {}
        };
    }
    Some(set_values)
}

fn parse_approval_policy(raw: &str) -> Option<AskForApproval> {
    let normalized = normalize_enum_token(raw);
    match normalized.as_str() {
        "untrusted" | "unless-trusted" => Some(AskForApproval::UnlessTrusted),
        "on-failure" => Some(AskForApproval::OnFailure),
        "on-request" | "ask" | "auto" => Some(AskForApproval::OnRequest),
        "never" => Some(AskForApproval::Never),
        _ => None,
    }
}

fn parse_sandbox_mode(raw: &str) -> Option<SandboxMode> {
    let normalized = normalize_enum_token(raw);
    match normalized.as_str() {
        "read-only" => Some(SandboxMode::ReadOnly),
        "workspace-write" => Some(SandboxMode::WorkspaceWrite),
        "danger-full-access" | "dangerously-skip-permissions" => {
            Some(SandboxMode::DangerFullAccess)
        }
        _ => None,
    }
}

fn normalize_enum_token(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('_', "-")
}

fn find_json_string(root: &JsonValue, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        let mut current = root;
        let mut found = true;
        for segment in *path {
            let Some(next) = current.get(segment) else {
                found = false;
                break;
            };
            current = next;
        }
        if found && let Some(value) = current.as_str() {
            return Some(value.to_string());
        }
    }
    None
}

async fn copy_file_if_missing(from: &Path, to: &Path) -> io::Result<bool> {
    if !tokio::fs::try_exists(from).await? || tokio::fs::try_exists(to).await? {
        return Ok(false);
    }
    if let Some(parent) = to.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(from, to).await?;
    Ok(true)
}

async fn copy_skills_from_claude(
    source_root: &Path,
    target_root: &Path,
) -> io::Result<Vec<String>> {
    if !tokio::fs::try_exists(source_root).await? {
        return Ok(Vec::new());
    }
    tokio::fs::create_dir_all(target_root).await?;

    let mut imported = Vec::new();
    let mut entries = tokio::fs::read_dir(source_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let source_skill_dir = entry.path();
        if !tokio::fs::try_exists(source_skill_dir.join("SKILL.md")).await? {
            continue;
        }
        let target_skill_dir = target_root.join(entry.file_name());
        if tokio::fs::try_exists(&target_skill_dir).await? {
            continue;
        }

        copy_dir_recursive(&source_skill_dir, &target_skill_dir).await?;
        imported.push(entry.file_name().to_string_lossy().to_string());
    }
    imported.sort();
    Ok(imported)
}

async fn copy_dir_recursive(source: &Path, target: &Path) -> io::Result<()> {
    let mut stack = vec![(source.to_path_buf(), target.to_path_buf())];
    while let Some((source_dir, target_dir)) = stack.pop() {
        tokio::fs::create_dir_all(&target_dir).await?;
        let mut entries = tokio::fs::read_dir(&source_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            let source_path = entry.path();
            let target_path = target_dir.join(entry.file_name());
            if file_type.is_dir() {
                stack.push((source_path, target_path));
                continue;
            }
            if file_type.is_file() {
                tokio::fs::copy(source_path, target_path).await?;
            }
        }
    }
    Ok(())
}

async fn read_import_state(state_path: &Path) -> io::Result<Option<ClaudeMigrationMarkerState>> {
    Ok(read_migration_state_file(state_path)
        .await?
        .map(|file| file.state))
}

async fn persist_import_state(
    state_path: &Path,
    state: ClaudeMigrationMarkerState,
    last_result: Option<ClaudeMigrationLastResult>,
) -> io::Result<()> {
    let existing = read_migration_state_file(state_path).await?;
    let state = ClaudeMigrationStateFile {
        schema_version: 1,
        state,
        updated_at_unix: OffsetDateTime::now_utc().unix_timestamp(),
        last_result: last_result.or_else(|| existing.and_then(|file| file.last_result)),
    };

    let serialized = serde_json::to_string_pretty(&state)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(parent) = state_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    write_atomically(&state_path, &format!("{serialized}\n"))
}

async fn read_migration_state_file(
    state_path: &Path,
) -> io::Result<Option<ClaudeMigrationStateFile>> {
    if !tokio::fs::try_exists(state_path).await? {
        return Ok(None);
    }
    let contents = tokio::fs::read_to_string(state_path).await?;
    let state: ClaudeMigrationStateFile = serde_json::from_str(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(Some(state))
}

async fn preview_home_migration(
    codex_home: &Path,
    claude_home: &Path,
    config_toml: &ConfigToml,
) -> io::Result<ClaudeHomeMigrationSummary> {
    let mut summary = ClaudeHomeMigrationSummary::default();

    let settings_path = claude_home.join("settings.json");
    if tokio::fs::try_exists(&settings_path).await? {
        let settings_contents = tokio::fs::read_to_string(&settings_path).await?;
        let settings_json: JsonValue = serde_json::from_str(&settings_contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let (_edits, imported_keys) = collect_settings_edits(&settings_json, config_toml);
        summary.imported_config_keys = imported_keys;
    }

    summary.imported_user_agents_md = tokio::fs::try_exists(claude_home.join("CLAUDE.md")).await?
        && !tokio::fs::try_exists(codex_home.join("AGENTS.md")).await?;
    summary.imported_skills =
        preview_skills_to_copy(&claude_home.join("skills"), &codex_home.join("skills")).await?;
    Ok(summary)
}

fn find_project_root(cwd: &Path) -> PathBuf {
    for ancestor in cwd.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor.to_path_buf();
        }
    }
    cwd.to_path_buf()
}

pub async fn migrate_claude_repo(cwd: &Path) -> io::Result<ClaudeRepoMigrationSummary> {
    apply_claude_repo_migration(cwd).await
}

async fn preview_repo_migration(project_root: &Path) -> io::Result<ClaudeRepoMigrationSummary> {
    Ok(ClaudeRepoMigrationSummary {
        copied_agents_md: tokio::fs::try_exists(project_root.join("CLAUDE.md")).await?
            && !tokio::fs::try_exists(project_root.join("AGENTS.md")).await?,
        imported_mcp_servers: preview_project_mcp_servers_to_add(project_root).await?,
    })
}

async fn apply_claude_repo_migration_at_root(
    project_root: &Path,
) -> io::Result<ClaudeRepoMigrationSummary> {
    let mut summary = ClaudeRepoMigrationSummary::default();
    if copy_file_if_missing(
        &project_root.join("CLAUDE.md"),
        &project_root.join("AGENTS.md"),
    )
    .await?
    {
        summary.copied_agents_md = true;
    }
    summary.imported_mcp_servers = import_project_mcp_servers(project_root).await?;
    Ok(summary)
}

async fn preview_project_mcp_servers_to_add(project_root: &Path) -> io::Result<Vec<String>> {
    let mcp_path = project_root.join(".mcp.json");
    if !tokio::fs::try_exists(&mcp_path).await? {
        return Ok(Vec::new());
    }
    let mcp_contents = tokio::fs::read_to_string(&mcp_path).await?;
    let mcp_json: JsonValue = serde_json::from_str(&mcp_contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let Some(source_servers) = extract_mcp_servers_map(&mcp_json) else {
        return Ok(Vec::new());
    };
    let project_config_path = project_root.join(".codex").join("config.toml");
    let root_toml = read_or_init_project_config(&project_config_path).await?;
    let existing = root_toml
        .as_table()
        .and_then(|root| root.get("mcp_servers"))
        .and_then(toml::Value::as_table);

    let mut to_add = source_servers
        .keys()
        .filter(|name| match existing {
            Some(table) => !table.contains_key(*name),
            None => true,
        })
        .cloned()
        .collect::<Vec<_>>();
    to_add.sort();
    Ok(to_add)
}

async fn import_project_mcp_servers(project_root: &Path) -> io::Result<Vec<String>> {
    let mcp_path = project_root.join(".mcp.json");
    if !tokio::fs::try_exists(&mcp_path).await? {
        return Ok(Vec::new());
    }

    let mcp_contents = tokio::fs::read_to_string(&mcp_path).await?;
    let mcp_json: JsonValue = serde_json::from_str(&mcp_contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let Some(source_servers) = extract_mcp_servers_map(&mcp_json) else {
        return Ok(Vec::new());
    };

    let project_config_path = project_root.join(".codex").join("config.toml");
    let mut root_toml = read_or_init_project_config(&project_config_path).await?;
    let root_table = root_toml.as_table_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "project config root must be a table: {}",
                project_config_path.display()
            ),
        )
    })?;
    let mcp_servers = root_table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "project mcp_servers must be a table: {}",
                    project_config_path.display()
                ),
            )
        })?;

    let mut imported = Vec::new();
    for (name, server_json) in source_servers {
        if mcp_servers.contains_key(name) {
            continue;
        }
        let Some(normalized) = normalize_mcp_server_config(server_json) else {
            continue;
        };
        let Some(server_toml) = json_to_toml_value(&normalized) else {
            continue;
        };
        mcp_servers.insert(name.to_string(), server_toml);
        imported.push(name.to_string());
    }

    if imported.is_empty() {
        return Ok(imported);
    }

    let serialized = toml::to_string_pretty(&root_toml)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(parent) = project_config_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    write_atomically(&project_config_path, &serialized)?;
    imported.sort();
    Ok(imported)
}

async fn preview_skills_to_copy(source_root: &Path, target_root: &Path) -> io::Result<Vec<String>> {
    if !tokio::fs::try_exists(source_root).await? {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    let mut entries = tokio::fs::read_dir(source_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let source_skill_dir = entry.path();
        if !tokio::fs::try_exists(source_skill_dir.join("SKILL.md")).await? {
            continue;
        }
        if tokio::fs::try_exists(target_root.join(entry.file_name())).await? {
            continue;
        }
        names.push(entry.file_name().to_string_lossy().to_string());
    }
    names.sort();
    Ok(names)
}

async fn count_claude_skills(claude_home: &Path) -> io::Result<usize> {
    let source_root = claude_home.join("skills");
    if !tokio::fs::try_exists(&source_root).await? {
        return Ok(0);
    }
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(&source_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        if tokio::fs::try_exists(entry.path().join("SKILL.md")).await? {
            count += 1;
        }
    }
    Ok(count)
}

async fn count_prior_codex_threads_up_to_threshold(
    codex_home: &Path,
    default_provider: &str,
    limit: usize,
) -> io::Result<usize> {
    let allowed_sources: &[SessionSource] = &[];

    if let Some(state_db_ctx) = state_db::open_if_present(codex_home, default_provider).await
        && let Some(ids) = state_db::list_thread_ids_db(
            Some(state_db_ctx.as_ref()),
            codex_home,
            limit,
            None,
            ThreadSortKey::CreatedAt,
            allowed_sources,
            None,
            false,
            "claude_migration_nux",
        )
        .await
    {
        return Ok(ids.len());
    }

    let sessions = get_threads_in_root(
        codex_home.join(SESSIONS_SUBDIR),
        limit,
        None,
        ThreadSortKey::CreatedAt,
        ThreadListConfig {
            allowed_sources,
            model_providers: None,
            default_provider,
            layout: ThreadListLayout::NestedByDate,
        },
    )
    .await?;
    if sessions.items.len() >= limit {
        return Ok(limit);
    }

    let remaining = limit.saturating_sub(sessions.items.len());
    let archived = get_threads_in_root(
        codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
        remaining,
        None,
        ThreadSortKey::CreatedAt,
        ThreadListConfig {
            allowed_sources,
            model_providers: None,
            default_provider,
            layout: ThreadListLayout::Flat,
        },
    )
    .await?;
    Ok(sessions.items.len() + archived.items.len())
}

fn extract_mcp_servers_map(
    mcp_json: &JsonValue,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    if let Some(explicit) = mcp_json
        .get("mcpServers")
        .or_else(|| mcp_json.get("mcp_servers"))
        .and_then(serde_json::Value::as_object)
    {
        return Some(explicit);
    }

    if let Some(root_obj) = mcp_json.as_object() {
        let all_serverish = root_obj.values().all(|value| {
            value
                .as_object()
                .is_some_and(|obj| obj.contains_key("command") || obj.contains_key("url"))
        });
        if all_serverish {
            return Some(root_obj);
        }
    }
    None
}

fn normalize_mcp_server_config(server_json: &JsonValue) -> Option<JsonValue> {
    let server_obj = server_json.as_object()?;
    let mut normalized = serde_json::Map::new();
    for (key, value) in server_obj {
        normalized.insert(normalize_mcp_server_key(key), value.clone());
    }
    Some(JsonValue::Object(normalized))
}

fn normalize_mcp_server_key(key: &str) -> String {
    match key {
        "envVars" => "env_vars".to_string(),
        "httpHeaders" => "http_headers".to_string(),
        "envHttpHeaders" => "env_http_headers".to_string(),
        "bearerTokenEnvVar" => "bearer_token_env_var".to_string(),
        "startupTimeoutSec" => "startup_timeout_sec".to_string(),
        "startupTimeoutMs" => "startup_timeout_ms".to_string(),
        "toolTimeoutSec" => "tool_timeout_sec".to_string(),
        "enabledTools" => "enabled_tools".to_string(),
        "disabledTools" => "disabled_tools".to_string(),
        _ => key.to_string(),
    }
}

fn json_to_toml_value(value: &JsonValue) -> Option<toml::Value> {
    match value {
        JsonValue::Null => None,
        JsonValue::Bool(boolean) => Some(toml::Value::Boolean(*boolean)),
        JsonValue::Number(number) => {
            if let Some(integer) = number.as_i64() {
                return Some(toml::Value::Integer(integer));
            }
            if let Some(unsigned) = number.as_u64() {
                if let Ok(integer) = i64::try_from(unsigned) {
                    return Some(toml::Value::Integer(integer));
                }
                return Some(toml::Value::Float(unsigned as f64));
            }
            number.as_f64().map(toml::Value::Float)
        }
        JsonValue::String(string) => Some(toml::Value::String(string.to_string())),
        JsonValue::Array(items) => {
            let mut toml_items = Vec::new();
            for item in items {
                if let Some(converted) = json_to_toml_value(item) {
                    toml_items.push(converted);
                }
            }
            Some(toml::Value::Array(toml_items))
        }
        JsonValue::Object(object) => {
            let mut table = toml::map::Map::new();
            for (key, value) in object {
                if let Some(converted) = json_to_toml_value(value) {
                    table.insert(key.to_string(), converted);
                }
            }
            Some(toml::Value::Table(table))
        }
    }
}

async fn read_or_init_project_config(config_path: &Path) -> io::Result<toml::Value> {
    if !tokio::fs::try_exists(config_path).await? {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let contents = tokio::fs::read_to_string(config_path).await?;
    toml::from_str(&contents).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn parse_config(contents: &str) -> io::Result<ConfigToml> {
        toml::from_str(contents).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    #[tokio::test]
    async fn home_migration_copies_skills_settings_and_agents_doc() -> io::Result<()> {
        let temp = TempDir::new()?;
        let home = temp.path().join("home");
        let codex_home = home.join(".codex");
        let claude_home = home.join(".claude");
        tokio::fs::create_dir_all(claude_home.join("skills/rust-helper/scripts")).await?;

        tokio::fs::write(
            claude_home.join("settings.json"),
            r#"{
  "model": "gpt-5.1-codex",
  "approvalPolicy": "on-request",
  "sandboxMode": "workspace-write"
}"#,
        )
        .await?;
        tokio::fs::write(claude_home.join("CLAUDE.md"), "follow these rules").await?;
        tokio::fs::write(
            claude_home.join("skills/rust-helper/SKILL.md"),
            "---\nname: rust-helper\n---\n",
        )
        .await?;
        tokio::fs::write(
            claude_home.join("skills/rust-helper/scripts/main.sh"),
            "#!/bin/sh\n",
        )
        .await?;

        let status =
            maybe_migrate_claude_home_with_paths(&codex_home, &claude_home, &ConfigToml::default())
                .await?;

        let expected_summary = ClaudeHomeMigrationSummary {
            imported_config_keys: vec![
                "model".to_string(),
                "approval_policy".to_string(),
                "sandbox_mode".to_string(),
            ],
            imported_skills: vec!["rust-helper".to_string()],
            imported_user_agents_md: true,
        };
        assert_eq!(
            status,
            ClaudeHomeMigrationStatus::Applied(expected_summary.clone())
        );

        let config_contents = tokio::fs::read_to_string(codex_home.join("config.toml")).await?;
        let persisted = parse_config(&config_contents)?;
        assert_eq!(persisted.model, Some("gpt-5.1-codex".to_string()));
        assert_eq!(persisted.approval_policy, Some(AskForApproval::OnRequest));
        assert_eq!(persisted.sandbox_mode, Some(SandboxMode::WorkspaceWrite));
        assert_eq!(
            tokio::fs::read_to_string(codex_home.join("AGENTS.md")).await?,
            "follow these rules"
        );
        assert_eq!(
            tokio::fs::try_exists(codex_home.join("skills/rust-helper/SKILL.md")).await?,
            true
        );
        assert_eq!(
            tokio::fs::try_exists(codex_home.join(CLAUDE_MIGRATION_STATE_RELATIVE_PATH)).await?,
            true
        );

        let second_status =
            maybe_migrate_claude_home_with_paths(&codex_home, &claude_home, &persisted).await?;
        assert_eq!(
            second_status,
            ClaudeHomeMigrationStatus::SkippedAlreadyImported
        );
        Ok(())
    }

    #[tokio::test]
    async fn home_migration_does_not_override_existing_config() -> io::Result<()> {
        let temp = TempDir::new()?;
        let home = temp.path().join("home");
        let codex_home = home.join(".codex");
        let claude_home = home.join(".claude");
        tokio::fs::create_dir_all(&claude_home).await?;
        tokio::fs::create_dir_all(&codex_home).await?;

        tokio::fs::write(
            codex_home.join("config.toml"),
            r#"model = "existing-model"
approval_policy = "never"
sandbox_mode = "read-only"
"#,
        )
        .await?;
        tokio::fs::write(
            claude_home.join("settings.json"),
            r#"{
  "model": "new-model",
  "approvalPolicy": "on-request",
  "sandboxMode": "workspace-write"
}"#,
        )
        .await?;
        let config_toml =
            parse_config(&tokio::fs::read_to_string(codex_home.join("config.toml")).await?)?;

        let status =
            maybe_migrate_claude_home_with_paths(&codex_home, &claude_home, &config_toml).await?;
        assert_eq!(
            status,
            ClaudeHomeMigrationStatus::Applied(ClaudeHomeMigrationSummary::default())
        );

        let updated =
            parse_config(&tokio::fs::read_to_string(codex_home.join("config.toml")).await?)?;
        assert_eq!(updated.model, Some("existing-model".to_string()));
        assert_eq!(updated.approval_policy, Some(AskForApproval::Never));
        assert_eq!(updated.sandbox_mode, Some(SandboxMode::ReadOnly));
        Ok(())
    }

    #[tokio::test]
    async fn project_import_copies_claude_md_and_imports_mcp_servers() -> io::Result<()> {
        let temp = TempDir::new()?;
        let repo = temp.path().join("repo");
        tokio::fs::create_dir_all(repo.join(".git")).await?;
        tokio::fs::create_dir_all(repo.join("subdir")).await?;
        tokio::fs::write(repo.join("CLAUDE.md"), "project rules").await?;
        tokio::fs::write(
            repo.join(".mcp.json"),
            r#"{
  "mcpServers": {
    "docs": {
      "command": "node",
      "args": ["docs-server.js"],
      "startupTimeoutSec": 15
    }
  }
}"#,
        )
        .await?;

        let summary = migrate_claude_repo(&repo.join("subdir")).await?;
        assert_eq!(summary.copied_agents_md, true);
        assert_eq!(summary.imported_mcp_servers, vec!["docs".to_string()]);
        assert_eq!(
            tokio::fs::read_to_string(repo.join("AGENTS.md")).await?,
            "project rules"
        );

        let config_contents = tokio::fs::read_to_string(repo.join(".codex/config.toml")).await?;
        assert!(config_contents.contains("[mcp_servers.docs]"));
        assert!(config_contents.contains("startup_timeout_sec = 15"));

        let second_summary = migrate_claude_repo(&repo.join("subdir")).await?;
        assert_eq!(second_summary.copied_agents_md, false);
        assert_eq!(second_summary.imported_mcp_servers, Vec::<String>::new());
        Ok(())
    }

    #[tokio::test]
    async fn project_import_preserves_existing_mcp_server_entries() -> io::Result<()> {
        let temp = TempDir::new()?;
        let repo = temp.path().join("repo");
        tokio::fs::create_dir_all(repo.join(".git")).await?;
        tokio::fs::create_dir_all(repo.join(".codex")).await?;
        tokio::fs::write(
            repo.join(".codex/config.toml"),
            r#"[mcp_servers.docs]
command = "existing"
"#,
        )
        .await?;
        tokio::fs::write(
            repo.join(".mcp.json"),
            r#"{
  "mcpServers": {
    "docs": {"command": "new"},
    "logs": {"command": "logs"}
  }
}"#,
        )
        .await?;

        let summary = migrate_claude_repo(&repo).await?;
        assert_eq!(summary.imported_mcp_servers, vec!["logs".to_string()]);
        let config_contents = tokio::fs::read_to_string(repo.join(".codex/config.toml")).await?;
        assert!(config_contents.contains("command = \"existing\""));
        assert!(config_contents.contains("[mcp_servers.logs]"));
        Ok(())
    }
}
