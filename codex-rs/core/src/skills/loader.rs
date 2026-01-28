use crate::config::Config;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::skills::model::SkillDependencies;
use crate::skills::model::SkillError;
use crate::skills::model::SkillInterface;
use crate::skills::model::SkillLoadOutcome;
use crate::skills::model::SkillMetadata;
use crate::skills::model::SkillToolDependency;
use crate::skills::system::system_cache_root_dir;
use codex_app_server_protocol::ConfigLayerSource;
use codex_protocol::protocol::SkillScope;
use dunce::canonicalize as canonicalize_path;
use serde::Deserialize;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use tracing::error;

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    metadata: SkillFrontmatterMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatterMetadata {
    #[serde(default, rename = "short-description")]
    short_description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SkillMetadataFile {
    #[serde(default)]
    interface: Option<Interface>,
    #[serde(default)]
    dependencies: Option<Dependencies>,
}

#[derive(Debug, Default, Deserialize)]
struct Interface {
    display_name: Option<String>,
    short_description: Option<String>,
    icon_small: Option<PathBuf>,
    icon_large: Option<PathBuf>,
    brand_color: Option<String>,
    default_prompt: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Dependencies {
    #[serde(default)]
    tools: Vec<DependencyTool>,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyTool {
    #[serde(rename = "type")]
    kind: Option<String>,
    value: Option<String>,
    description: Option<String>,
    transport: Option<String>,
    command: Option<String>,
    url: Option<String>,
}

const SKILLS_FILENAME: &str = "SKILL.md";
const SKILLS_JSON_FILENAME: &str = "SKILL.json";
const SKILLS_DIR_NAME: &str = "skills";
const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;
const MAX_SHORT_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEFAULT_PROMPT_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_TYPE_LEN: usize = MAX_NAME_LEN;
const MAX_DEPENDENCY_TRANSPORT_LEN: usize = MAX_NAME_LEN;
const MAX_DEPENDENCY_VALUE_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_COMMAND_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_URL_LEN: usize = MAX_DESCRIPTION_LEN;
// Traversal depth from the skills root.
const MAX_SCAN_DEPTH: usize = 6;
const MAX_SKILLS_DIRS_PER_ROOT: usize = 2000;

#[derive(Debug)]
enum SkillParseError {
    Read(std::io::Error),
    MissingFrontmatter,
    InvalidYaml(serde_yaml::Error),
    MissingField(&'static str),
    InvalidField { field: &'static str, reason: String },
}

impl fmt::Display for SkillParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillParseError::Read(e) => write!(f, "failed to read file: {e}"),
            SkillParseError::MissingFrontmatter => {
                write!(f, "missing YAML frontmatter delimited by ---")
            }
            SkillParseError::InvalidYaml(e) => write!(f, "invalid YAML: {e}"),
            SkillParseError::MissingField(field) => write!(f, "missing field `{field}`"),
            SkillParseError::InvalidField { field, reason } => {
                write!(f, "invalid {field}: {reason}")
            }
        }
    }
}

impl Error for SkillParseError {}

pub fn load_skills(config: &Config) -> SkillLoadOutcome {
    load_skills_from_roots(skill_roots(config))
}

pub(crate) struct SkillRoot {
    pub(crate) path: PathBuf,
    pub(crate) scope: SkillScope,
}

pub(crate) fn load_skills_from_roots<I>(roots: I) -> SkillLoadOutcome
where
    I: IntoIterator<Item = SkillRoot>,
{
    let mut outcome = SkillLoadOutcome::default();
    for root in roots {
        discover_skills_under_root(&root.path, root.scope, &mut outcome);
    }

    let mut seen: HashSet<PathBuf> = HashSet::new();
    outcome
        .skills
        .retain(|skill| seen.insert(skill.path.clone()));

    fn scope_rank(scope: SkillScope) -> u8 {
        // Higher-priority scopes first (matches root scan order for dedupe).
        match scope {
            SkillScope::Repo => 0,
            SkillScope::User => 1,
            SkillScope::System => 2,
            SkillScope::Admin => 3,
        }
    }

    outcome.skills.sort_by(|a, b| {
        scope_rank(a.scope)
            .cmp(&scope_rank(b.scope))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });

    outcome
}

fn skill_roots_from_layer_stack_inner(config_layer_stack: &ConfigLayerStack) -> Vec<SkillRoot> {
    let mut roots = Vec::new();

    for layer in
        config_layer_stack.get_layers(ConfigLayerStackOrdering::HighestPrecedenceFirst, true)
    {
        let Some(config_folder) = layer.config_folder() else {
            continue;
        };

        match &layer.name {
            ConfigLayerSource::Project { .. } => {
                roots.push(SkillRoot {
                    path: config_folder.as_path().join(SKILLS_DIR_NAME),
                    scope: SkillScope::Repo,
                });
            }
            ConfigLayerSource::User { .. } => {
                // `$CODEX_HOME/skills` (user-installed skills).
                roots.push(SkillRoot {
                    path: config_folder.as_path().join(SKILLS_DIR_NAME),
                    scope: SkillScope::User,
                });

                // Embedded system skills are cached under `$CODEX_HOME/skills/.system` and are a
                // special case (not a config layer).
                roots.push(SkillRoot {
                    path: system_cache_root_dir(config_folder.as_path()),
                    scope: SkillScope::System,
                });
            }
            ConfigLayerSource::System { .. } => {
                // The system config layer lives under `/etc/codex/` on Unix, so treat
                // `/etc/codex/skills` as admin-scoped skills.
                roots.push(SkillRoot {
                    path: config_folder.as_path().join(SKILLS_DIR_NAME),
                    scope: SkillScope::Admin,
                });
            }
            ConfigLayerSource::Mdm { .. }
            | ConfigLayerSource::SessionFlags
            | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
            | ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {}
        }
    }

    roots
}

fn skill_roots(config: &Config) -> Vec<SkillRoot> {
    skill_roots_from_layer_stack_inner(&config.config_layer_stack)
}

pub(crate) fn skill_roots_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
) -> Vec<SkillRoot> {
    skill_roots_from_layer_stack_inner(config_layer_stack)
}

fn discover_skills_under_root(root: &Path, scope: SkillScope, outcome: &mut SkillLoadOutcome) {
    let Ok(root) = canonicalize_path(root) else {
        return;
    };

    if !root.is_dir() {
        return;
    }

    fn enqueue_dir(
        queue: &mut VecDeque<(PathBuf, usize)>,
        visited_dirs: &mut HashSet<PathBuf>,
        truncated_by_dir_limit: &mut bool,
        path: PathBuf,
        depth: usize,
    ) {
        if depth > MAX_SCAN_DEPTH {
            return;
        }
        if visited_dirs.len() >= MAX_SKILLS_DIRS_PER_ROOT {
            *truncated_by_dir_limit = true;
            return;
        }
        if visited_dirs.insert(path.clone()) {
            queue.push_back((path, depth));
        }
    }

    // Follow symlinked directories for user, admin, and repo skills. System skills are written by Codex itself.
    let follow_symlinks = matches!(
        scope,
        SkillScope::Repo | SkillScope::User | SkillScope::Admin
    );

    let mut visited_dirs: HashSet<PathBuf> = HashSet::new();
    visited_dirs.insert(root.clone());

    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::from([(root.clone(), 0)]);
    let mut truncated_by_dir_limit = false;

    while let Some((dir, depth)) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!("failed to read skills dir {}: {e:#}", dir.display());
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|f| f.to_str()) {
                Some(name) => name,
                None => continue,
            };

            if file_name.starts_with('.') {
                continue;
            }

            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_symlink() {
                if !follow_symlinks {
                    continue;
                }

                // Follow the symlink to determine what it points to.
                let metadata = match fs::metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        error!(
                            "failed to stat skills entry {} (symlink): {e:#}",
                            path.display()
                        );
                        continue;
                    }
                };

                if metadata.is_dir() {
                    let Ok(resolved_dir) = canonicalize_path(&path) else {
                        continue;
                    };
                    enqueue_dir(
                        &mut queue,
                        &mut visited_dirs,
                        &mut truncated_by_dir_limit,
                        resolved_dir,
                        depth + 1,
                    );
                    continue;
                }

                continue;
            }

            if file_type.is_dir() {
                let Ok(resolved_dir) = canonicalize_path(&path) else {
                    continue;
                };
                enqueue_dir(
                    &mut queue,
                    &mut visited_dirs,
                    &mut truncated_by_dir_limit,
                    resolved_dir,
                    depth + 1,
                );
                continue;
            }

            if file_type.is_file() && file_name == SKILLS_FILENAME {
                match parse_skill_file(&path, scope) {
                    Ok(skill) => {
                        outcome.skills.push(skill);
                    }
                    Err(err) => {
                        if scope != SkillScope::System {
                            outcome.errors.push(SkillError {
                                path,
                                message: err.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    if truncated_by_dir_limit {
        tracing::warn!(
            "skills scan truncated after {} directories (root: {})",
            MAX_SKILLS_DIRS_PER_ROOT,
            root.display()
        );
    }
}

fn parse_skill_file(path: &Path, scope: SkillScope) -> Result<SkillMetadata, SkillParseError> {
    let contents = fs::read_to_string(path).map_err(SkillParseError::Read)?;

    let frontmatter = extract_frontmatter(&contents).ok_or(SkillParseError::MissingFrontmatter)?;

    let parsed: SkillFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(SkillParseError::InvalidYaml)?;

    let name = sanitize_single_line(&parsed.name);
    let description = sanitize_single_line(&parsed.description);
    let short_description = parsed
        .metadata
        .short_description
        .as_deref()
        .map(sanitize_single_line)
        .filter(|value| !value.is_empty());
    let (interface, dependencies) = load_skill_metadata(path);

    validate_len(&name, MAX_NAME_LEN, "name")?;
    validate_len(&description, MAX_DESCRIPTION_LEN, "description")?;
    if let Some(short_description) = short_description.as_deref() {
        validate_len(
            short_description,
            MAX_SHORT_DESCRIPTION_LEN,
            "metadata.short-description",
        )?;
    }

    let resolved_path = canonicalize_path(path).unwrap_or_else(|_| path.to_path_buf());

    Ok(SkillMetadata {
        name,
        description,
        short_description,
        interface,
        dependencies,
        path: resolved_path,
        scope,
    })
}

fn load_skill_metadata(skill_path: &Path) -> (Option<SkillInterface>, Option<SkillDependencies>) {
    // Fail open: optional metadata should not block loading SKILL.md.
    let Some(skill_dir) = skill_path.parent() else {
        return (None, None);
    };
    let metadata_path = skill_dir.join(SKILLS_JSON_FILENAME);
    if !metadata_path.exists() {
        return (None, None);
    }

    let contents = match fs::read_to_string(&metadata_path) {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!(
                "ignoring {path}: failed to read {label}: {error}",
                path = metadata_path.display(),
                label = SKILLS_JSON_FILENAME
            );
            return (None, None);
        }
    };

    let parsed: SkillMetadataFile = match serde_json::from_str(&contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!(
                "ignoring {path}: invalid {label}: {error}",
                path = metadata_path.display(),
                label = SKILLS_JSON_FILENAME
            );
            return (None, None);
        }
    };

    (
        resolve_interface(parsed.interface, skill_dir),
        resolve_dependencies(parsed.dependencies),
    )
}

fn resolve_interface(interface: Option<Interface>, skill_dir: &Path) -> Option<SkillInterface> {
    let interface = interface?;
    let interface = SkillInterface {
        display_name: resolve_str(
            interface.display_name,
            MAX_NAME_LEN,
            "interface.display_name",
        ),
        short_description: resolve_str(
            interface.short_description,
            MAX_SHORT_DESCRIPTION_LEN,
            "interface.short_description",
        ),
        icon_small: resolve_asset_path(skill_dir, "interface.icon_small", interface.icon_small),
        icon_large: resolve_asset_path(skill_dir, "interface.icon_large", interface.icon_large),
        brand_color: resolve_color_str(interface.brand_color, "interface.brand_color"),
        default_prompt: resolve_str(
            interface.default_prompt,
            MAX_DEFAULT_PROMPT_LEN,
            "interface.default_prompt",
        ),
    };
    let has_fields = interface.display_name.is_some()
        || interface.short_description.is_some()
        || interface.icon_small.is_some()
        || interface.icon_large.is_some()
        || interface.brand_color.is_some()
        || interface.default_prompt.is_some();
    if has_fields { Some(interface) } else { None }
}

fn resolve_dependencies(dependencies: Option<Dependencies>) -> Option<SkillDependencies> {
    let dependencies = dependencies?;
    let tools: Vec<SkillToolDependency> = dependencies
        .tools
        .into_iter()
        .filter_map(resolve_dependency_tool)
        .collect();
    if tools.is_empty() {
        None
    } else {
        Some(SkillDependencies { tools })
    }
}

fn resolve_dependency_tool(tool: DependencyTool) -> Option<SkillToolDependency> {
    let r#type = resolve_required_str(
        tool.kind,
        MAX_DEPENDENCY_TYPE_LEN,
        "dependencies.tools.type",
    )?;
    let value = resolve_required_str(
        tool.value,
        MAX_DEPENDENCY_VALUE_LEN,
        "dependencies.tools.value",
    )?;
    let description = resolve_str(
        tool.description,
        MAX_DEPENDENCY_DESCRIPTION_LEN,
        "dependencies.tools.description",
    );
    let transport = resolve_str(
        tool.transport,
        MAX_DEPENDENCY_TRANSPORT_LEN,
        "dependencies.tools.transport",
    );
    let command = resolve_str(
        tool.command,
        MAX_DEPENDENCY_COMMAND_LEN,
        "dependencies.tools.command",
    );
    let url = resolve_str(tool.url, MAX_DEPENDENCY_URL_LEN, "dependencies.tools.url");

    Some(SkillToolDependency {
        r#type,
        value,
        description,
        transport,
        command,
        url,
    })
}

fn resolve_asset_path(
    skill_dir: &Path,
    field: &'static str,
    path: Option<PathBuf>,
) -> Option<PathBuf> {
    // Icons must be relative paths under the skill's assets/ directory; otherwise return None.
    let path = path?;
    if path.as_os_str().is_empty() {
        return None;
    }

    let assets_dir = skill_dir.join("assets");
    if path.is_absolute() {
        tracing::warn!(
            "ignoring {field}: icon must be a relative assets path (not {})",
            assets_dir.display()
        );
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                tracing::warn!("ignoring {field}: icon path must not contain '..'");
                return None;
            }
            _ => {
                tracing::warn!("ignoring {field}: icon path must be under assets/");
                return None;
            }
        }
    }

    let mut components = normalized.components();
    match components.next() {
        Some(Component::Normal(component)) if component == "assets" => {}
        _ => {
            tracing::warn!("ignoring {field}: icon path must be under assets/");
            return None;
        }
    }

    Some(skill_dir.join(normalized))
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_len(
    value: &str,
    max_len: usize,
    field_name: &'static str,
) -> Result<(), SkillParseError> {
    if value.is_empty() {
        return Err(SkillParseError::MissingField(field_name));
    }
    if value.chars().count() > max_len {
        return Err(SkillParseError::InvalidField {
            field: field_name,
            reason: format!("exceeds maximum length of {max_len} characters"),
        });
    }
    Ok(())
}

fn resolve_str(value: Option<String>, max_len: usize, field: &'static str) -> Option<String> {
    let value = value?;
    let value = sanitize_single_line(&value);
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    if value.chars().count() > max_len {
        tracing::warn!("ignoring {field}: exceeds maximum length of {max_len} characters");
        return None;
    }
    Some(value)
}

fn resolve_required_str(
    value: Option<String>,
    max_len: usize,
    field: &'static str,
) -> Option<String> {
    let Some(value) = value else {
        tracing::warn!("ignoring {field}: value is missing");
        return None;
    };
    resolve_str(Some(value), max_len, field)
}

fn resolve_color_str(value: Option<String>, field: &'static str) -> Option<String> {
    let value = value?;
    let value = value.trim();
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    let mut chars = value.chars();
    if value.len() == 7 && chars.next() == Some('#') && chars.all(|c| c.is_ascii_hexdigit()) {
        Some(value.to_string())
    } else {
        tracing::warn!("ignoring {field}: expected #RRGGBB, got {value}");
        None
    }
}

fn extract_frontmatter(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return None;
    }

    let mut frontmatter_lines: Vec<&str> = Vec::new();
    let mut found_closing = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            found_closing = true;
            break;
        }
        frontmatter_lines.push(line);
    }

    if frontmatter_lines.is_empty() || !found_closing {
        return None;
    }

    Some(frontmatter_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG_TOML_FILE;
    use crate::config::ConfigBuilder;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::config::ProjectConfig;
    use crate::config_loader::ConfigLayerEntry;
    use crate::config_loader::ConfigLayerStack;
    use crate::config_loader::ConfigRequirements;
    use crate::config_loader::ConfigRequirementsToml;
    use codex_protocol::config_types::TrustLevel;
    use codex_protocol::protocol::SkillScope;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::Path;
    use tempfile::TempDir;
    use toml::Value as TomlValue;

    const REPO_ROOT_CONFIG_DIR_NAME: &str = ".codex";

    async fn make_config(codex_home: &TempDir) -> Config {
        make_config_for_cwd(codex_home, codex_home.path().to_path_buf()).await
    }

    async fn make_config_for_cwd(codex_home: &TempDir, cwd: PathBuf) -> Config {
        let trust_root = cwd
            .ancestors()
            .find(|ancestor| ancestor.join(".git").exists())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cwd.clone());

        fs::write(
            codex_home.path().join(CONFIG_TOML_FILE),
            toml::to_string(&ConfigToml {
                projects: Some(HashMap::from([(
                    trust_root.to_string_lossy().to_string(),
                    ProjectConfig {
                        trust_level: Some(TrustLevel::Trusted),
                    },
                )])),
                ..Default::default()
            })
            .expect("serialize config"),
        )
        .unwrap();

        let harness_overrides = ConfigOverrides {
            cwd: Some(cwd),
            ..Default::default()
        };

        ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .harness_overrides(harness_overrides)
            .build()
            .await
            .expect("defaults for test should always succeed")
    }

    fn mark_as_git_repo(dir: &Path) {
        // Config/project-root discovery only checks for the presence of `.git` (file or dir),
        // so we can avoid shelling out to `git init` in tests.
        fs::write(dir.join(".git"), "gitdir: fake\n").unwrap();
    }

    fn normalized(path: &Path) -> PathBuf {
        canonicalize_path(path).unwrap_or_else(|_| path.to_path_buf())
    }

    #[test]
    fn skill_roots_from_layer_stack_maps_user_to_user_and_system_cache_and_system_to_admin()
    -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;

        let system_folder = tmp.path().join("etc/codex");
        let user_folder = tmp.path().join("home/codex");
        fs::create_dir_all(&system_folder)?;
        fs::create_dir_all(&user_folder)?;

        // The file path doesn't need to exist; it's only used to derive the config folder.
        let system_file = AbsolutePathBuf::from_absolute_path(system_folder.join("config.toml"))?;
        let user_file = AbsolutePathBuf::from_absolute_path(user_folder.join("config.toml"))?;

        let layers = vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::System { file: system_file },
                TomlValue::Table(toml::map::Map::new()),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::User { file: user_file },
                TomlValue::Table(toml::map::Map::new()),
            ),
        ];
        let stack = ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )?;

        let got = skill_roots_from_layer_stack(&stack)
            .into_iter()
            .map(|root| (root.scope, root.path))
            .collect::<Vec<_>>();

        assert_eq!(
            got,
            vec![
                (SkillScope::User, user_folder.join("skills")),
                (
                    SkillScope::System,
                    user_folder.join("skills").join(".system")
                ),
                (SkillScope::Admin, system_folder.join("skills")),
            ]
        );

        Ok(())
    }

    #[test]
    fn skill_roots_from_layer_stack_includes_disabled_project_layers() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;

        let user_folder = tmp.path().join("home/codex");
        fs::create_dir_all(&user_folder)?;

        let project_root = tmp.path().join("repo");
        let dot_codex = project_root.join(".codex");
        fs::create_dir_all(&dot_codex)?;

        let user_file = AbsolutePathBuf::from_absolute_path(user_folder.join("config.toml"))?;
        let project_dot_codex = AbsolutePathBuf::from_absolute_path(&dot_codex)?;

        let layers = vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User { file: user_file },
                TomlValue::Table(toml::map::Map::new()),
            ),
            ConfigLayerEntry::new_disabled(
                ConfigLayerSource::Project {
                    dot_codex_folder: project_dot_codex,
                },
                TomlValue::Table(toml::map::Map::new()),
                "marked untrusted",
            ),
        ];
        let stack = ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )?;

        let got = skill_roots_from_layer_stack(&stack)
            .into_iter()
            .map(|root| (root.scope, root.path))
            .collect::<Vec<_>>();

        assert_eq!(
            got,
            vec![
                (SkillScope::Repo, dot_codex.join("skills")),
                (SkillScope::User, user_folder.join("skills")),
                (
                    SkillScope::System,
                    user_folder.join("skills").join(".system")
                ),
            ]
        );

        Ok(())
    }

    fn write_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) -> PathBuf {
        write_skill_at(&codex_home.path().join("skills"), dir, name, description)
    }

    fn write_system_skill(
        codex_home: &TempDir,
        dir: &str,
        name: &str,
        description: &str,
    ) -> PathBuf {
        write_skill_at(
            &codex_home.path().join("skills/.system"),
            dir,
            name,
            description,
        )
    }

    fn write_skill_at(root: &Path, dir: &str, name: &str, description: &str) -> PathBuf {
        let skill_dir = root.join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        let indented_description = description.replace('\n', "\n  ");
        let content = format!(
            "---\nname: {name}\ndescription: |-\n  {indented_description}\n---\n\n# Body\n"
        );
        let path = skill_dir.join(SKILLS_FILENAME);
        fs::write(&path, content).unwrap();
        path
    }

    fn write_skill_metadata_at(skill_dir: &Path, filename: &str, contents: &str) -> PathBuf {
        let path = skill_dir.join(filename);
        fs::write(&path, contents).unwrap();
        path
    }

    fn write_skill_interface_at(skill_dir: &Path, contents: &str) -> PathBuf {
        write_skill_metadata_at(skill_dir, SKILLS_JSON_FILENAME, contents)
    }

    #[tokio::test]
    async fn loads_skill_dependencies_metadata_from_json() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "dep-skill", "from json");
        let skill_dir = skill_path.parent().expect("skill dir");

        write_skill_metadata_at(
            skill_dir,
            SKILLS_JSON_FILENAME,
            r#"
{
  "dependencies": {
    "tools": [
      {
        "type": "env_var",
        "value": "GITHUB_TOKEN",
        "description": "GitHub API token with repo scopes"
      },
      {
        "type": "mcp",
        "value": "github",
        "description": "GitHub MCP server",
        "transport": "streamable_http",
        "url": "https://example.com/mcp"
      },
      {
        "type": "cli",
        "value": "gh",
        "description": "GitHub CLI"
      },
      {
        "type": "mcp",
        "value": "local-gh",
        "description": "Local GH MCP server",
        "transport": "stdio",
        "command": "gh-mcp"
      }
    ]
  }
}
"#,
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "dep-skill".to_string(),
                description: "from json".to_string(),
                short_description: None,
                interface: None,
                dependencies: Some(SkillDependencies {
                    tools: vec![
                        SkillToolDependency {
                            r#type: "env_var".to_string(),
                            value: "GITHUB_TOKEN".to_string(),
                            description: Some("GitHub API token with repo scopes".to_string()),
                            transport: None,
                            command: None,
                            url: None,
                        },
                        SkillToolDependency {
                            r#type: "mcp".to_string(),
                            value: "github".to_string(),
                            description: Some("GitHub MCP server".to_string()),
                            transport: Some("streamable_http".to_string()),
                            command: None,
                            url: Some("https://example.com/mcp".to_string()),
                        },
                        SkillToolDependency {
                            r#type: "cli".to_string(),
                            value: "gh".to_string(),
                            description: Some("GitHub CLI".to_string()),
                            transport: None,
                            command: None,
                            url: None,
                        },
                        SkillToolDependency {
                            r#type: "mcp".to_string(),
                            value: "local-gh".to_string(),
                            description: Some("Local GH MCP server".to_string()),
                            transport: Some("stdio".to_string()),
                            command: Some("gh-mcp".to_string()),
                            url: None,
                        },
                    ],
                }),
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn loads_skill_interface_metadata_from_json() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
        let skill_dir = skill_path.parent().expect("skill dir");
        let normalized_skill_dir = normalized(skill_dir);

        write_skill_interface_at(
            skill_dir,
            r##"
{
  "interface": {
    "display_name": "UI Skill",
    "short_description": "  short    desc   ",
    "icon_small": "./assets/small-400px.png",
    "icon_large": "./assets/large-logo.svg",
    "brand_color": "#3B82F6",
    "default_prompt": "  default   prompt   "
  }
}
"##,
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "ui-skill".to_string(),
                description: "from json".to_string(),
                short_description: None,
                interface: Some(SkillInterface {
                    display_name: Some("UI Skill".to_string()),
                    short_description: Some("short desc".to_string()),
                    icon_small: Some(normalized_skill_dir.join("assets/small-400px.png")),
                    icon_large: Some(normalized_skill_dir.join("assets/large-logo.svg")),
                    brand_color: Some("#3B82F6".to_string()),
                    default_prompt: Some("default prompt".to_string()),
                }),
                dependencies: None,
                path: normalized(skill_path.as_path()),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn accepts_icon_paths_under_assets_dir() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
        let skill_dir = skill_path.parent().expect("skill dir");
        let normalized_skill_dir = normalized(skill_dir);

        write_skill_interface_at(
            skill_dir,
            r#"
{
  "interface": {
    "display_name": "UI Skill",
    "icon_small": "assets/icon.png",
    "icon_large": "./assets/logo.svg"
  }
}
"#,
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "ui-skill".to_string(),
                description: "from json".to_string(),
                short_description: None,
                interface: Some(SkillInterface {
                    display_name: Some("UI Skill".to_string()),
                    short_description: None,
                    icon_small: Some(normalized_skill_dir.join("assets/icon.png")),
                    icon_large: Some(normalized_skill_dir.join("assets/logo.svg")),
                    brand_color: None,
                    default_prompt: None,
                }),
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn ignores_invalid_brand_color() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
        let skill_dir = skill_path.parent().expect("skill dir");

        write_skill_interface_at(
            skill_dir,
            r#"
{
  "interface": {
    "brand_color": "blue"
  }
}
"#,
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "ui-skill".to_string(),
                description: "from json".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn ignores_default_prompt_over_max_length() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
        let skill_dir = skill_path.parent().expect("skill dir");
        let normalized_skill_dir = normalized(skill_dir);
        let too_long = "x".repeat(MAX_DEFAULT_PROMPT_LEN + 1);

        write_skill_interface_at(
            skill_dir,
            &format!(
                r##"
{{
  "interface": {{
    "display_name": "UI Skill",
    "icon_small": "./assets/small-400px.png",
    "default_prompt": "{too_long}"
  }}
}}
"##
            ),
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "ui-skill".to_string(),
                description: "from json".to_string(),
                short_description: None,
                interface: Some(SkillInterface {
                    display_name: Some("UI Skill".to_string()),
                    short_description: None,
                    icon_small: Some(normalized_skill_dir.join("assets/small-400px.png")),
                    icon_large: None,
                    brand_color: None,
                    default_prompt: None,
                }),
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn drops_interface_when_icons_are_invalid() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
        let skill_dir = skill_path.parent().expect("skill dir");

        write_skill_interface_at(
            skill_dir,
            r#"
{
  "interface": {
    "icon_small": "icon.png",
    "icon_large": "./assets/../logo.svg"
  }
}
"#,
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "ui-skill".to_string(),
                description: "from json".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[cfg(unix)]
    fn symlink_dir(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(unix)]
    fn symlink_file(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn loads_skills_via_symlinked_subdir_for_user_scope() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let shared = tempfile::tempdir().expect("tempdir");

        let shared_skill_path = write_skill_at(shared.path(), "demo", "linked-skill", "from link");

        fs::create_dir_all(codex_home.path().join("skills")).unwrap();
        symlink_dir(shared.path(), &codex_home.path().join("skills/shared"));

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "linked-skill".to_string(),
                description: "from link".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&shared_skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn ignores_symlinked_skill_file_for_user_scope() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let shared = tempfile::tempdir().expect("tempdir");

        let shared_skill_path =
            write_skill_at(shared.path(), "demo", "linked-file-skill", "from link");

        let skill_dir = codex_home.path().join("skills/demo");
        fs::create_dir_all(&skill_dir).unwrap();
        symlink_file(&shared_skill_path, &skill_dir.join(SKILLS_FILENAME));

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills, Vec::new());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn does_not_loop_on_symlink_cycle_for_user_scope() {
        let codex_home = tempfile::tempdir().expect("tempdir");

        // Create a cycle:
        //   $CODEX_HOME/skills/cycle/loop -> $CODEX_HOME/skills/cycle
        let cycle_dir = codex_home.path().join("skills/cycle");
        fs::create_dir_all(&cycle_dir).unwrap();
        symlink_dir(&cycle_dir, &cycle_dir.join("loop"));

        let skill_path = write_skill_at(&cycle_dir, "demo", "cycle-skill", "still loads");

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "cycle-skill".to_string(),
                description: "still loads".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[test]
    #[cfg(unix)]
    fn loads_skills_via_symlinked_subdir_for_admin_scope() {
        let admin_root = tempfile::tempdir().expect("tempdir");
        let shared = tempfile::tempdir().expect("tempdir");

        let shared_skill_path =
            write_skill_at(shared.path(), "demo", "admin-linked-skill", "from link");
        fs::create_dir_all(admin_root.path()).unwrap();
        symlink_dir(shared.path(), &admin_root.path().join("shared"));

        let outcome = load_skills_from_roots([SkillRoot {
            path: admin_root.path().to_path_buf(),
            scope: SkillScope::Admin,
        }]);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "admin-linked-skill".to_string(),
                description: "from link".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&shared_skill_path),
                scope: SkillScope::Admin,
            }]
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn loads_skills_via_symlinked_subdir_for_repo_scope() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");
        mark_as_git_repo(repo_dir.path());
        let shared = tempfile::tempdir().expect("tempdir");

        let linked_skill_path =
            write_skill_at(shared.path(), "demo", "repo-linked-skill", "from link");
        let repo_skills_root = repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME);
        fs::create_dir_all(&repo_skills_root).unwrap();
        symlink_dir(shared.path(), &repo_skills_root.join("shared"));

        let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "repo-linked-skill".to_string(),
                description: "from link".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&linked_skill_path),
                scope: SkillScope::Repo,
            }]
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn system_scope_ignores_symlinked_subdir() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let shared = tempfile::tempdir().expect("tempdir");

        write_skill_at(shared.path(), "demo", "system-linked-skill", "from link");

        let system_root = codex_home.path().join("skills/.system");
        fs::create_dir_all(&system_root).unwrap();
        symlink_dir(shared.path(), &system_root.join("shared"));

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 0);
    }

    #[tokio::test]
    async fn respects_max_scan_depth_for_user_scope() {
        let codex_home = tempfile::tempdir().expect("tempdir");

        let within_depth_path = write_skill(
            &codex_home,
            "d0/d1/d2/d3/d4/d5",
            "within-depth-skill",
            "loads",
        );
        let _too_deep_path = write_skill(
            &codex_home,
            "d0/d1/d2/d3/d4/d5/d6",
            "too-deep-skill",
            "should not load",
        );

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "within-depth-skill".to_string(),
                description: "loads".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&within_depth_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn loads_valid_skill() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_path = write_skill(&codex_home, "demo", "demo-skill", "does things\ncarefully");
        let cfg = make_config(&codex_home).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "demo-skill".to_string(),
                description: "does things carefully".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn loads_short_description_from_metadata() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_dir = codex_home.path().join("skills/demo");
        fs::create_dir_all(&skill_dir).unwrap();
        let contents = "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: short summary\n---\n\n# Body\n";
        let skill_path = skill_dir.join(SKILLS_FILENAME);
        fs::write(&skill_path, contents).unwrap();

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "demo-skill".to_string(),
                description: "long description".to_string(),
                short_description: Some("short summary".to_string()),
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::User,
            }]
        );
    }

    #[tokio::test]
    async fn enforces_short_description_length_limits() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_dir = codex_home.path().join("skills/demo");
        fs::create_dir_all(&skill_dir).unwrap();
        let too_long = "x".repeat(MAX_SHORT_DESCRIPTION_LEN + 1);
        let contents = format!(
            "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: {too_long}\n---\n\n# Body\n"
        );
        fs::write(skill_dir.join(SKILLS_FILENAME), contents).unwrap();

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);
        assert_eq!(outcome.skills.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0]
                .message
                .contains("invalid metadata.short-description"),
            "expected length error, got: {:?}",
            outcome.errors
        );
    }

    #[tokio::test]
    async fn skips_hidden_and_invalid() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let hidden_dir = codex_home.path().join("skills/.hidden");
        fs::create_dir_all(&hidden_dir).unwrap();
        fs::write(
            hidden_dir.join(SKILLS_FILENAME),
            "---\nname: hidden\ndescription: hidden\n---\n",
        )
        .unwrap();

        // Invalid because missing closing frontmatter.
        let invalid_dir = codex_home.path().join("skills/invalid");
        fs::create_dir_all(&invalid_dir).unwrap();
        fs::write(invalid_dir.join(SKILLS_FILENAME), "---\nname: bad").unwrap();

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);
        assert_eq!(outcome.skills.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0]
                .message
                .contains("missing YAML frontmatter"),
            "expected frontmatter error"
        );
    }

    #[tokio::test]
    async fn enforces_length_limits() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let max_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN);
        write_skill(&codex_home, "max-len", "max-len", &max_desc);
        let cfg = make_config(&codex_home).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);

        let too_long_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN + 1);
        write_skill(&codex_home, "too-long", "too-long", &too_long_desc);
        let outcome = load_skills(&cfg);
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0].message.contains("invalid description"),
            "expected length error"
        );
    }

    #[tokio::test]
    async fn loads_skills_from_repo_root() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");
        mark_as_git_repo(repo_dir.path());

        let skills_root = repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME);
        let skill_path = write_skill_at(&skills_root, "repo", "repo-skill", "from repo");
        let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "repo-skill".to_string(),
                description: "from repo".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::Repo,
            }]
        );
    }

    #[tokio::test]
    async fn loads_skills_from_all_codex_dirs_under_project_root() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");
        mark_as_git_repo(repo_dir.path());

        let nested_dir = repo_dir.path().join("nested/inner");
        fs::create_dir_all(&nested_dir).unwrap();

        let root_skill_path = write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "root",
            "root-skill",
            "from root",
        );
        let nested_skill_path = write_skill_at(
            &repo_dir
                .path()
                .join("nested")
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "nested",
            "nested-skill",
            "from nested",
        );

        let cfg = make_config_for_cwd(&codex_home, nested_dir).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![
                SkillMetadata {
                    name: "nested-skill".to_string(),
                    description: "from nested".to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: normalized(&nested_skill_path),
                    scope: SkillScope::Repo,
                },
                SkillMetadata {
                    name: "root-skill".to_string(),
                    description: "from root".to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: normalized(&root_skill_path),
                    scope: SkillScope::Repo,
                },
            ]
        );
    }

    #[tokio::test]
    async fn loads_skills_from_codex_dir_when_not_git_repo() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let work_dir = tempfile::tempdir().expect("tempdir");

        let skill_path = write_skill_at(
            &work_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "local",
            "local-skill",
            "from cwd",
        );

        let cfg = make_config_for_cwd(&codex_home, work_dir.path().to_path_buf()).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "local-skill".to_string(),
                description: "from cwd".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::Repo,
            }]
        );
    }

    #[tokio::test]
    async fn deduplicates_by_path_preferring_first_root() {
        let root = tempfile::tempdir().expect("tempdir");

        let skill_path = write_skill_at(root.path(), "dupe", "dupe-skill", "from repo");

        let outcome = load_skills_from_roots([
            SkillRoot {
                path: root.path().to_path_buf(),
                scope: SkillScope::Repo,
            },
            SkillRoot {
                path: root.path().to_path_buf(),
                scope: SkillScope::User,
            },
        ]);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "dupe-skill".to_string(),
                description: "from repo".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::Repo,
            }]
        );
    }

    #[tokio::test]
    async fn keeps_duplicate_names_from_repo_and_user() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");
        mark_as_git_repo(repo_dir.path());

        let user_skill_path = write_skill(&codex_home, "user", "dupe-skill", "from user");
        let repo_skill_path = write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "repo",
            "dupe-skill",
            "from repo",
        );

        let cfg = make_config_for_cwd(&codex_home, repo_dir.path().to_path_buf()).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![
                SkillMetadata {
                    name: "dupe-skill".to_string(),
                    description: "from repo".to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: normalized(&repo_skill_path),
                    scope: SkillScope::Repo,
                },
                SkillMetadata {
                    name: "dupe-skill".to_string(),
                    description: "from user".to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: normalized(&user_skill_path),
                    scope: SkillScope::User,
                },
            ]
        );
    }

    #[tokio::test]
    async fn keeps_duplicate_names_from_nested_codex_dirs() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");
        mark_as_git_repo(repo_dir.path());

        let nested_dir = repo_dir.path().join("nested/inner");
        fs::create_dir_all(&nested_dir).unwrap();

        let root_skill_path = write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "root",
            "dupe-skill",
            "from root",
        );
        let nested_skill_path = write_skill_at(
            &repo_dir
                .path()
                .join("nested")
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "nested",
            "dupe-skill",
            "from nested",
        );

        let cfg = make_config_for_cwd(&codex_home, nested_dir).await;
        let outcome = load_skills(&cfg);

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        let root_path =
            canonicalize_path(&root_skill_path).unwrap_or_else(|_| root_skill_path.clone());
        let nested_path =
            canonicalize_path(&nested_skill_path).unwrap_or_else(|_| nested_skill_path.clone());
        let (first_path, second_path, first_description, second_description) =
            if root_path <= nested_path {
                (root_path, nested_path, "from root", "from nested")
            } else {
                (nested_path, root_path, "from nested", "from root")
            };
        assert_eq!(
            outcome.skills,
            vec![
                SkillMetadata {
                    name: "dupe-skill".to_string(),
                    description: first_description.to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: first_path,
                    scope: SkillScope::Repo,
                },
                SkillMetadata {
                    name: "dupe-skill".to_string(),
                    description: second_description.to_string(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: second_path,
                    scope: SkillScope::Repo,
                },
            ]
        );
    }

    #[tokio::test]
    async fn repo_skills_search_does_not_escape_repo_root() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let outer_dir = tempfile::tempdir().expect("tempdir");
        let repo_dir = outer_dir.path().join("repo");
        fs::create_dir_all(&repo_dir).unwrap();

        let _skill_path = write_skill_at(
            &outer_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "outer",
            "outer-skill",
            "from outer",
        );
        mark_as_git_repo(&repo_dir);

        let cfg = make_config_for_cwd(&codex_home, repo_dir).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 0);
    }

    #[tokio::test]
    async fn loads_skills_when_cwd_is_file_in_repo() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");
        mark_as_git_repo(repo_dir.path());

        let skill_path = write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "repo",
            "repo-skill",
            "from repo",
        );
        let file_path = repo_dir.path().join("some-file.txt");
        fs::write(&file_path, "contents").unwrap();

        let cfg = make_config_for_cwd(&codex_home, file_path).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "repo-skill".to_string(),
                description: "from repo".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::Repo,
            }]
        );
    }

    #[tokio::test]
    async fn non_git_repo_skills_search_does_not_walk_parents() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let outer_dir = tempfile::tempdir().expect("tempdir");
        let nested_dir = outer_dir.path().join("nested/inner");
        fs::create_dir_all(&nested_dir).unwrap();

        write_skill_at(
            &outer_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "outer",
            "outer-skill",
            "from outer",
        );

        let cfg = make_config_for_cwd(&codex_home, nested_dir).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 0);
    }

    #[tokio::test]
    async fn loads_skills_from_system_cache_when_present() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let work_dir = tempfile::tempdir().expect("tempdir");

        let skill_path = write_system_skill(&codex_home, "system", "system-skill", "from system");

        let cfg = make_config_for_cwd(&codex_home, work_dir.path().to_path_buf()).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(
            outcome.skills,
            vec![SkillMetadata {
                name: "system-skill".to_string(),
                description: "from system".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                path: normalized(&skill_path),
                scope: SkillScope::System,
            }]
        );
    }

    #[tokio::test]
    async fn skill_roots_include_admin_with_lowest_priority_on_unix() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let cfg = make_config(&codex_home).await;

        let scopes: Vec<SkillScope> = skill_roots(&cfg)
            .into_iter()
            .map(|root| root.scope)
            .collect();
        let mut expected = vec![SkillScope::User, SkillScope::System];
        if cfg!(unix) {
            expected.push(SkillScope::Admin);
        }
        assert_eq!(scopes, expected);
    }
}
