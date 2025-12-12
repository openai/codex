use crate::config::Config;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::skills::model::SkillError;
use crate::skills::model::SkillLoadOutcome;
use crate::skills::model::SkillMetadata;
use dunce::canonicalize as normalize_path;
use serde::Deserialize;
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
}

const SKILLS_FILENAME: &str = "SKILL.md";
const SKILLS_DIR_NAME: &str = "skills";
const REPO_ROOT_CONFIG_DIR_NAME: &str = ".codex";
const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;

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
    let mut outcome = SkillLoadOutcome::default();
    let roots = skill_roots(config);
    for root in roots {
        discover_skills_under_root(&root, &mut outcome);
    }

    outcome
        .skills
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

    outcome
}

fn skill_roots(config: &Config) -> Vec<PathBuf> {
    let mut roots = vec![config.codex_home.join(SKILLS_DIR_NAME)];

    if let Some(repo_root) = resolve_root_git_project_for_trust(&config.cwd) {
        roots.push(
            repo_root
                .join(REPO_ROOT_CONFIG_DIR_NAME)
                .join(SKILLS_DIR_NAME),
        );
    }

    roots
}

fn discover_skills_under_root(root: &Path, outcome: &mut SkillLoadOutcome) {
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
                match parse_skill_file(&path) {
                    Ok(skill) => outcome.skills.push(skill),
                    Err(err) => outcome.errors.push(SkillError {
                        path,
                        message: err.to_string(),
                    }),
                }
            }
        }
    }
}

fn parse_skill_file(path: &Path) -> Result<SkillMetadata, SkillParseError> {
    let contents = fs::read_to_string(path).map_err(SkillParseError::Read)?;

    let frontmatter = extract_frontmatter(&contents).ok_or(SkillParseError::MissingFrontmatter)?;

    let parsed: SkillFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(SkillParseError::InvalidYaml)?;

    let name = sanitize_single_line(&parsed.name);
    let description = sanitize_single_line(&parsed.description);

    validate_field(&name, MAX_NAME_LEN, "name")?;
    validate_field(&description, MAX_DESCRIPTION_LEN, "description")?;

    let resolved_path = normalize_path(path).unwrap_or_else(|_| path.to_path_buf());

    Ok(SkillMetadata {
        name,
        description,
        path: resolved_path,
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
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    fn make_config(codex_home: &TempDir) -> Config {
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("defaults for test should always succeed");

        config.cwd = codex_home.path().to_path_buf();
        config
    }

    fn write_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) -> PathBuf {
        write_skill_at(codex_home.path(), dir, name, description)
    }

    fn write_skill_at(root: &Path, dir: &str, name: &str, description: &str) -> PathBuf {
        let skill_dir = root.join(format!("skills/{dir}"));
        fs::create_dir_all(&skill_dir).unwrap();
        let indented_description = description.replace('\n', "\n  ");
        let content = format!(
            "---\nname: {name}\ndescription: |-\n  {indented_description}\n---\n\n# Body\n"
        );
        let path = skill_dir.join(SKILLS_FILENAME);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn loads_valid_skill() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        write_skill(&codex_home, "demo", "demo-skill", "does things\ncarefully");
        let cfg = make_config(&codex_home);

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
        let path_str = skill.path.to_string_lossy().replace('\\', "/");
        assert!(
            path_str.ends_with("skills/demo/SKILL.md"),
            "unexpected path {path_str}"
        );
    }

    #[test]
    fn skips_hidden_and_invalid() {
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

        let cfg = make_config(&codex_home);
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

    #[test]
    fn enforces_length_limits() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let max_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN);
        write_skill(&codex_home, "max-len", "max-len", &max_desc);
        let cfg = make_config(&codex_home);

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

    #[test]
    fn loads_skills_from_repo_root() {
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
        let mut cfg = make_config(&codex_home);
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
}
