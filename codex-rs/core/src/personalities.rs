/*!
Loads built-in and file-backed personality definitions, resolves precedence
between repo and user scopes, and exposes the catalog used by config, prompt
construction, and the app-server personality picker.
*/

use crate::config::Config;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_protocol::config_types::Personality;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

const PERSONALITIES_DIR_NAME: &str = "personalities";
const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;
const BUILTIN_FRIENDLY_INSTRUCTIONS: &str =
    include_str!("../templates/personalities/gpt-5.2-codex_friendly.md");
const BUILTIN_PRAGMATIC_INSTRUCTIONS: &str =
    include_str!("../templates/personalities/gpt-5.2-codex_pragmatic.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalityScope {
    Builtin,
    User,
    Repo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalityDefinition {
    pub name: Personality,
    pub description: String,
    pub instructions: Option<String>,
    pub scope: PersonalityScope,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PersonalityCatalog {
    personalities: Vec<PersonalityDefinition>,
    by_name: HashMap<Personality, PersonalityDefinition>,
}

impl PersonalityCatalog {
    pub fn personalities(&self) -> &[PersonalityDefinition] {
        self.personalities.as_slice()
    }

    pub fn get(&self, personality: &Personality) -> Option<&PersonalityDefinition> {
        self.by_name.get(personality)
    }

    pub fn instructions_for(&self, personality: &Personality) -> Option<&str> {
        self.get(personality)
            .and_then(|definition| definition.instructions.as_deref())
    }
}

#[derive(Debug, Default)]
struct PersonalityLoadOutcome {
    personalities: Vec<PersonalityDefinition>,
    errors: Vec<PersonalityError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersonalityError {
    path: PathBuf,
    message: String,
}

#[derive(Debug, Deserialize)]
struct PersonalityFrontmatter {
    name: String,
    description: String,
}

#[derive(Debug)]
enum PersonalityParseError {
    Read(std::io::Error),
    MissingFrontmatter,
    InvalidYaml(serde_yaml::Error),
    MissingBody,
    InvalidName(String),
    InvalidField { field: &'static str, reason: String },
}

impl fmt::Display for PersonalityParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersonalityParseError::Read(error) => write!(f, "failed to read file: {error}"),
            PersonalityParseError::MissingFrontmatter => {
                write!(f, "missing YAML frontmatter delimited by ---")
            }
            PersonalityParseError::InvalidYaml(error) => write!(f, "invalid YAML: {error}"),
            PersonalityParseError::MissingBody => write!(f, "missing markdown body"),
            PersonalityParseError::InvalidName(reason) => write!(f, "invalid name: {reason}"),
            PersonalityParseError::InvalidField { field, reason } => {
                write!(f, "invalid {field}: {reason}")
            }
        }
    }
}

impl Error for PersonalityParseError {}

pub fn catalog_for_config(config: &Config) -> PersonalityCatalog {
    catalog_from_layer_stack(&config.config_layer_stack)
}

pub fn catalog_from_layer_stack(config_layer_stack: &ConfigLayerStack) -> PersonalityCatalog {
    let mut outcome = load_personalities_from_layer_stack(config_layer_stack);
    for error in &outcome.errors {
        warn!(
            path = %error.path.display(),
            error = %error.message,
            "ignoring invalid personality definition"
        );
    }

    outcome.personalities.sort_by(|left, right| {
        left.is_builtin
            .cmp(&right.is_builtin)
            .reverse()
            .then_with(|| {
                left.name
                    .as_str()
                    .to_ascii_lowercase()
                    .cmp(&right.name.as_str().to_ascii_lowercase())
            })
            .then_with(|| left.name.as_str().cmp(right.name.as_str()))
    });

    let mut by_name = HashMap::new();
    for personality in outcome.personalities.iter().cloned() {
        by_name.insert(personality.name.clone(), personality);
    }
    PersonalityCatalog {
        personalities: outcome.personalities,
        by_name,
    }
}

fn load_personalities_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
) -> PersonalityLoadOutcome {
    let mut outcome = PersonalityLoadOutcome {
        personalities: builtin_personalities(),
        ..Default::default()
    };
    let mut seen: HashSet<Personality> = outcome
        .personalities
        .iter()
        .map(|personality| personality.name.clone())
        .collect();

    for (root, scope) in personality_roots(config_layer_stack) {
        discover_personalities_under_root(root.as_path(), scope, &mut seen, &mut outcome);
    }

    outcome
}

fn builtin_personalities() -> Vec<PersonalityDefinition> {
    vec![
        PersonalityDefinition {
            name: Personality::none(),
            description: "No personality instructions.".to_string(),
            instructions: Some(String::new()),
            scope: PersonalityScope::Builtin,
            is_builtin: true,
        },
        PersonalityDefinition {
            name: Personality::friendly(),
            description: "Warm, collaborative, and helpful.".to_string(),
            instructions: Some(BUILTIN_FRIENDLY_INSTRUCTIONS.to_string()),
            scope: PersonalityScope::Builtin,
            is_builtin: true,
        },
        PersonalityDefinition {
            name: Personality::pragmatic(),
            description: "Concise, task-focused, and direct.".to_string(),
            instructions: Some(BUILTIN_PRAGMATIC_INSTRUCTIONS.to_string()),
            scope: PersonalityScope::Builtin,
            is_builtin: true,
        },
    ]
}

fn personality_roots(config_layer_stack: &ConfigLayerStack) -> Vec<(PathBuf, PersonalityScope)> {
    let mut roots = Vec::new();
    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::HighestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        let Some(config_folder) = layer.config_folder() else {
            continue;
        };

        let scope = match &layer.name {
            ConfigLayerSource::Project { .. } => PersonalityScope::Repo,
            ConfigLayerSource::User { .. } => PersonalityScope::User,
            _ => continue,
        };
        roots.push((config_folder.as_path().join(PERSONALITIES_DIR_NAME), scope));
    }

    let mut seen = HashSet::new();
    roots.retain(|(path, _)| seen.insert(path.clone()));
    roots
}

fn discover_personalities_under_root(
    root: &Path,
    scope: PersonalityScope,
    seen: &mut HashSet<Personality>,
    outcome: &mut PersonalityLoadOutcome,
) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            outcome.errors.push(PersonalityError {
                path: root.to_path_buf(),
                message: error.to_string(),
            });
            return;
        }
    };

    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        if !path.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            continue;
        }

        match parse_personality_file(path.as_path(), scope) {
            Ok(personality) => {
                if seen.insert(personality.name.clone()) {
                    outcome.personalities.push(personality);
                }
            }
            Err(error) => {
                outcome.errors.push(PersonalityError {
                    path,
                    message: error.to_string(),
                });
            }
        }
    }
}

fn parse_personality_file(
    path: &Path,
    scope: PersonalityScope,
) -> Result<PersonalityDefinition, PersonalityParseError> {
    let contents = fs::read_to_string(path).map_err(PersonalityParseError::Read)?;
    let (frontmatter, body) =
        extract_frontmatter_and_body(&contents).ok_or(PersonalityParseError::MissingFrontmatter)?;
    let parsed: PersonalityFrontmatter =
        serde_yaml::from_str(frontmatter.as_str()).map_err(PersonalityParseError::InvalidYaml)?;

    let name = sanitize_single_line(parsed.name.as_str());
    validate_name(name.as_str())?;

    let description = sanitize_single_line(parsed.description.as_str());
    validate_len(description.as_str(), MAX_DESCRIPTION_LEN, "description")?;

    let instructions = body.trim();
    if instructions.is_empty() {
        return Err(PersonalityParseError::MissingBody);
    }

    Ok(PersonalityDefinition {
        name: Personality::from(name),
        description,
        instructions: Some(instructions.to_string()),
        scope,
        is_builtin: false,
    })
}

fn validate_name(name: &str) -> Result<(), PersonalityParseError> {
    validate_len(name, MAX_NAME_LEN, "name")?;
    let normalized = name.to_ascii_lowercase();
    if Personality::builtin_names().contains(&normalized.as_str()) {
        return Err(PersonalityParseError::InvalidName(format!(
            "`{name}` is reserved for a built-in personality"
        )));
    }
    Ok(())
}

fn validate_len(
    value: &str,
    max_len: usize,
    field_name: &'static str,
) -> Result<(), PersonalityParseError> {
    if value.is_empty() {
        return Err(PersonalityParseError::InvalidField {
            field: field_name,
            reason: "value is empty".to_string(),
        });
    }
    if value.chars().count() > max_len {
        return Err(PersonalityParseError::InvalidField {
            field: field_name,
            reason: format!("exceeds maximum length of {max_len} characters"),
        });
    }
    Ok(())
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_frontmatter_and_body(contents: &str) -> Option<(String, String)> {
    let mut lines = contents.lines();
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return None;
    }

    let mut frontmatter_lines = Vec::new();
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

    let body = lines.collect::<Vec<_>>().join("\n");
    Some((frontmatter_lines.join("\n"), body))
}

#[cfg(test)]
mod tests {
    use super::PERSONALITIES_DIR_NAME;
    use super::catalog_from_layer_stack;
    use crate::config_loader::CloudRequirementsLoader;
    use crate::config_loader::LoaderOverrides;
    use crate::config_loader::load_config_layers_state;
    use codex_app_server_protocol::ConfigLayerSource;
    use codex_config::ConfigLayerEntry;
    use codex_config::ConfigLayerStack;
    use codex_config::ConfigRequirements;
    use codex_config::ConfigRequirementsToml;
    use codex_protocol::config_types::Personality;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;
    use toml::Value as TomlValue;

    fn write_personality(root: &std::path::Path, name: &str, description: &str, body: &str) {
        fs::create_dir_all(root).unwrap();
        let path = root.join(format!("{name}.md"));
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn repo_personality_overrides_user_personality_name() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join(".codex-home");
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join(".codex")).unwrap();
        fs::create_dir_all(&codex_home).unwrap();

        write_personality(
            codex_home.join("personalities").as_path(),
            "night-owl",
            "User personality",
            "user body",
        );
        write_personality(
            repo_root.join(".codex/personalities").as_path(),
            "night-owl",
            "Repo personality",
            "repo body",
        );

        let stack = load_config_layers_state(
            &codex_home,
            Some(repo_root.clone().try_into().unwrap()),
            &[],
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
        )
        .await
        .unwrap();

        let catalog = catalog_from_layer_stack(&stack);
        let personality = catalog.get(&Personality::from("night-owl")).unwrap();
        assert_eq!(personality.description, "Repo personality");
        assert_eq!(personality.instructions.as_deref(), Some("repo body"));
    }

    #[tokio::test]
    async fn invalid_personality_is_ignored() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join(".codex-home");
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join(".codex/personalities")).unwrap();
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            repo_root.join(".codex/personalities/bad.md"),
            "---\nname: friendly\ndescription: bad\n---\n\nbody\n",
        )
        .unwrap();

        let stack = load_config_layers_state(
            &codex_home,
            Some(repo_root.clone().try_into().unwrap()),
            &[],
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
        )
        .await
        .unwrap();

        let catalog = catalog_from_layer_stack(&stack);
        assert!(
            catalog
                .get(&Personality::from("friendly"))
                .unwrap()
                .is_builtin
        );
        assert!(catalog.get(&Personality::from("bad")).is_none());
    }

    #[tokio::test]
    async fn personalities_are_sorted_with_builtins_first_then_name() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join(".codex-home");
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(repo_root.join(".codex")).unwrap();
        fs::create_dir_all(&codex_home).unwrap();

        write_personality(
            codex_home.join("personalities").as_path(),
            "zebra",
            "User personality",
            "zebra body",
        );
        write_personality(
            repo_root.join(".codex/personalities").as_path(),
            "alpha",
            "Repo personality",
            "alpha body",
        );

        let stack = load_config_layers_state(
            &codex_home,
            Some(repo_root.clone().try_into().unwrap()),
            &[],
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
        )
        .await
        .unwrap();

        let catalog = catalog_from_layer_stack(&stack);
        let names = catalog
            .personalities()
            .iter()
            .map(|personality| personality.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["friendly", "none", "pragmatic", "alpha", "zebra"]
        );
    }

    #[tokio::test]
    async fn duplicate_personality_names_in_one_directory_are_resolved_by_path_order() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join(".codex-home");
        let repo_root = temp.path().join("repo");
        let repo_personalities = repo_root.join(".codex/personalities");
        fs::create_dir_all(&repo_personalities).unwrap();
        fs::create_dir_all(&codex_home).unwrap();

        fs::write(
            repo_personalities.join("a-first.md"),
            "---\nname: duped\ndescription: first\n---\n\nfirst body\n",
        )
        .unwrap();
        fs::write(
            repo_personalities.join("z-second.md"),
            "---\nname: duped\ndescription: second\n---\n\nsecond body\n",
        )
        .unwrap();

        let stack = load_config_layers_state(
            &codex_home,
            Some(repo_root.clone().try_into().unwrap()),
            &[],
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
        )
        .await
        .unwrap();

        let catalog = catalog_from_layer_stack(&stack);
        let personality = catalog.get(&Personality::from("duped")).unwrap();
        assert_eq!(personality.description, "first");
        assert_eq!(personality.instructions.as_deref(), Some("first body"));
    }

    #[test]
    fn disabled_project_layers_do_not_override_active_personalities() {
        let user_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        write_personality(
            &user_dir.path().join(PERSONALITIES_DIR_NAME),
            "night-owl",
            "User personality",
            "user body",
        );
        write_personality(
            &repo_dir.path().join(PERSONALITIES_DIR_NAME),
            "night-owl",
            "Repo personality",
            "repo body",
        );

        let user_layer = ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: user_dir.path().join("config.toml").try_into().unwrap(),
            },
            TomlValue::Table(toml::map::Map::new()),
        );
        let disabled_repo_layer = ConfigLayerEntry::new_disabled(
            ConfigLayerSource::Project {
                dot_codex_folder: repo_dir.path().try_into().unwrap(),
            },
            TomlValue::Table(toml::map::Map::new()),
            "inactive",
        );
        let stack = ConfigLayerStack::new(
            vec![user_layer, disabled_repo_layer],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .unwrap();

        let catalog = catalog_from_layer_stack(&stack);
        let personality = catalog.get(&Personality::from("night-owl")).unwrap();
        assert_eq!(personality.description, "User personality");
        assert_eq!(personality.instructions.as_deref(), Some("user body"));
    }
}
