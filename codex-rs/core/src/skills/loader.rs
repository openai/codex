use crate::config::Config;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::skills::model::SkillError;
use crate::skills::model::SkillLoadOutcome;
use crate::skills::model::SkillMetadata;
use crate::skills::system::system_cache_root_dir;
use codex_protocol::protocol::SkillScope;
use dunce::canonicalize as normalize_path;
use serde::Deserialize;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::fs;
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

const SKILLS_FILENAME: &str = "SKILL.md";
const SKILLS_DIR_NAME: &str = "skills";
const REPO_ROOT_CONFIG_DIR_NAME: &str = ".codex";
const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;
const MAX_SHORT_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;

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

    let mut seen: HashSet<String> = HashSet::new();
    outcome
        .skills
        .retain(|skill| seen.insert(skill.name.clone()));

    outcome
        .skills
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

    outcome
}

pub(crate) fn user_skills_root(codex_home: &Path) -> SkillRoot {
    SkillRoot {
        path: codex_home.join(SKILLS_DIR_NAME),
        scope: SkillScope::User,
    }
}

pub(crate) fn system_skills_root(codex_home: &Path) -> SkillRoot {
    SkillRoot {
        path: system_cache_root_dir(codex_home),
        scope: SkillScope::System,
    }
}

pub(crate) fn repo_skills_root(cwd: &Path) -> Option<SkillRoot> {
    let base = if cwd.is_dir() { cwd } else { cwd.parent()? };
    let base = normalize_path(base).unwrap_or_else(|_| base.to_path_buf());

    let repo_root =
        resolve_root_git_project_for_trust(&base).map(|root| normalize_path(&root).unwrap_or(root));

    let scope = SkillScope::Repo;
    if let Some(repo_root) = repo_root.as_deref() {
        for dir in base.ancestors() {
            let skills_root = dir.join(REPO_ROOT_CONFIG_DIR_NAME).join(SKILLS_DIR_NAME);
            if skills_root.is_dir() {
                return Some(SkillRoot {
                    path: skills_root,
                    scope,
                });
            }

            if dir == repo_root {
                break;
            }
        }
        return None;
    }

    let skills_root = base.join(REPO_ROOT_CONFIG_DIR_NAME).join(SKILLS_DIR_NAME);
    skills_root.is_dir().then_some(SkillRoot {
        path: skills_root,
        scope,
    })
}

fn skill_roots(config: &Config) -> Vec<SkillRoot> {
    let mut roots = Vec::new();

    if let Some(repo_root) = repo_skills_root(&config.cwd) {
        roots.push(repo_root);
    }

    // Load order matters: we dedupe by name, keeping the first occurrence.
    // This makes repo/user skills win over system skills.
    roots.push(user_skills_root(&config.codex_home));
    roots.push(system_skills_root(&config.codex_home));

    roots
}

fn discover_skills_under_root(root: &Path, scope: SkillScope, outcome: &mut SkillLoadOutcome) {
    let Ok(root) = normalize_path(root) else {
        return;
    };

    if !root.is_dir() {
        return;
    }

    let mut queue: VecDeque<PathBuf> = VecDeque::from([root]);
    while let Some(dir) = queue.pop_front() {
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
                continue;
            }

            if file_type.is_dir() {
                queue.push_back(path);
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

    validate_field(&name, MAX_NAME_LEN, "name")?;
    validate_field(&description, MAX_DESCRIPTION_LEN, "description")?;
    if let Some(short_description) = short_description.as_deref() {
        validate_field(
            short_description,
            MAX_SHORT_DESCRIPTION_LEN,
            "metadata.short-description",
        )?;
    }

    let resolved_path = normalize_path(path).unwrap_or_else(|_| path.to_path_buf());

    Ok(SkillMetadata {
        name,
        description,
        short_description,
        path: resolved_path,
        scope,
    })
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_field(
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
    use crate::config::ConfigBuilder;
    use codex_protocol::protocol::SkillScope;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    async fn make_config(codex_home: &TempDir) -> Config {
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("defaults for test should always succeed");

        config.cwd = codex_home.path().to_path_buf();
        config
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

    #[tokio::test]
    async fn loads_valid_skill() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        write_skill(&codex_home, "demo", "demo-skill", "does things\ncarefully");
        let cfg = make_config(&codex_home).await;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        let skill = &outcome.skills[0];
        assert_eq!(skill.name, "demo-skill");
        assert_eq!(skill.description, "does things carefully");
        assert_eq!(skill.short_description, None);
        let path_str = skill.path.to_string_lossy().replace('\\', "/");
        assert!(
            path_str.ends_with("skills/demo/SKILL.md"),
            "unexpected path {path_str}"
        );
    }

    #[tokio::test]
    async fn loads_short_description_from_metadata() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let skill_dir = codex_home.path().join("skills/demo");
        fs::create_dir_all(&skill_dir).unwrap();
        let contents = "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: short summary\n---\n\n# Body\n";
        fs::write(skill_dir.join(SKILLS_FILENAME), contents).unwrap();

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(
            outcome.skills[0].short_description,
            Some("short summary".to_string())
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

        let status = Command::new("git")
            .arg("init")
            .current_dir(repo_dir.path())
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");

        let skills_root = repo_dir
            .path()
            .join(REPO_ROOT_CONFIG_DIR_NAME)
            .join(SKILLS_DIR_NAME);
        write_skill_at(&skills_root, "repo", "repo-skill", "from repo");
        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = repo_dir.path().to_path_buf();
        let repo_root = normalize_path(&skills_root).unwrap_or_else(|_| skills_root.clone());

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        let skill = &outcome.skills[0];
        assert_eq!(skill.name, "repo-skill");
        assert!(skill.path.starts_with(&repo_root));
    }

    #[tokio::test]
    async fn loads_skills_from_nearest_codex_dir_under_repo_root() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");

        let status = Command::new("git")
            .arg("init")
            .current_dir(repo_dir.path())
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");

        let nested_dir = repo_dir.path().join("nested/inner");
        fs::create_dir_all(&nested_dir).unwrap();

        write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "root",
            "root-skill",
            "from root",
        );
        write_skill_at(
            &repo_dir
                .path()
                .join("nested")
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "nested",
            "nested-skill",
            "from nested",
        );

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = nested_dir;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "nested-skill");
    }

    #[tokio::test]
    async fn loads_skills_from_codex_dir_when_not_git_repo() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let work_dir = tempfile::tempdir().expect("tempdir");

        write_skill_at(
            &work_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "local",
            "local-skill",
            "from cwd",
        );

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = work_dir.path().to_path_buf();

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "local-skill");
        assert_eq!(outcome.skills[0].scope, SkillScope::Repo);
    }

    #[tokio::test]
    async fn deduplicates_by_name_preferring_repo_over_user() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");

        let status = Command::new("git")
            .arg("init")
            .current_dir(repo_dir.path())
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");

        write_skill(&codex_home, "user", "dupe-skill", "from user");
        write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "repo",
            "dupe-skill",
            "from repo",
        );

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = repo_dir.path().to_path_buf();

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "dupe-skill");
        assert_eq!(outcome.skills[0].scope, SkillScope::Repo);
    }

    #[tokio::test]
    async fn loads_system_skills_with_lowest_priority() {
        let codex_home = tempfile::tempdir().expect("tempdir");

        write_system_skill(&codex_home, "system", "dupe-skill", "from system");
        write_skill(&codex_home, "user", "dupe-skill", "from user");

        let cfg = make_config(&codex_home).await;
        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].description, "from user");
        assert_eq!(outcome.skills[0].scope, SkillScope::User);
    }

    #[tokio::test]
    async fn repo_skills_search_does_not_escape_repo_root() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let outer_dir = tempfile::tempdir().expect("tempdir");
        let repo_dir = outer_dir.path().join("repo");
        fs::create_dir_all(&repo_dir).unwrap();

        write_skill_at(
            &outer_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "outer",
            "outer-skill",
            "from outer",
        );

        let status = Command::new("git")
            .arg("init")
            .current_dir(&repo_dir)
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = repo_dir;

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

        let status = Command::new("git")
            .arg("init")
            .current_dir(repo_dir.path())
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");

        write_skill_at(
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

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = file_path;

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "repo-skill");
        assert_eq!(outcome.skills[0].scope, SkillScope::Repo);
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

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = nested_dir;

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

        write_system_skill(&codex_home, "system", "system-skill", "from system");

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = work_dir.path().to_path_buf();

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "system-skill");
        assert_eq!(outcome.skills[0].scope, SkillScope::System);
    }

    #[tokio::test]
    async fn deduplicates_by_name_preferring_user_over_system() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let work_dir = tempfile::tempdir().expect("tempdir");

        write_skill(&codex_home, "user", "dupe-skill", "from user");
        write_system_skill(&codex_home, "system", "dupe-skill", "from system");

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = work_dir.path().to_path_buf();

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "dupe-skill");
        assert_eq!(outcome.skills[0].scope, SkillScope::User);
    }

    #[tokio::test]
    async fn deduplicates_by_name_preferring_repo_over_system() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let repo_dir = tempfile::tempdir().expect("tempdir");

        let status = Command::new("git")
            .arg("init")
            .current_dir(repo_dir.path())
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");

        write_skill_at(
            &repo_dir
                .path()
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
            "repo",
            "dupe-skill",
            "from repo",
        );
        write_system_skill(&codex_home, "system", "dupe-skill", "from system");

        let mut cfg = make_config(&codex_home).await;
        cfg.cwd = repo_dir.path().to_path_buf();

        let outcome = load_skills(&cfg);
        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "dupe-skill");
        assert_eq!(outcome.skills[0].scope, SkillScope::Repo);
    }
}
