//! Skill loading from directories.
//!
//! Reads `SKILL.toml` files, resolves prompt content (from external files
//! or inline), validates the result, and produces [`SkillLoadOutcome`] values.

use crate::command::SkillPromptCommand;
use crate::interface::SkillInterface;
use crate::outcome::SkillLoadOutcome;
use crate::scanner::SkillScanner;
use crate::source::SkillSource;
use crate::validator;

use std::fs;
use std::path::{Path, PathBuf};

/// The expected metadata file name.
const SKILL_TOML: &str = "SKILL.toml";

/// Loads all skills from a single directory.
///
/// The directory itself is expected to be a skills root (e.g.,
/// `.cocode/skills/`). Each immediate subdirectory (or nested directory
/// found by the scanner) that contains `SKILL.toml` is treated as a
/// skill directory.
///
/// Returns one [`SkillLoadOutcome`] per discovered skill directory.
/// Failed skills produce [`SkillLoadOutcome::Failed`] but do not
/// prevent other skills from loading.
pub fn load_skills_from_dir(dir: &Path) -> Vec<SkillLoadOutcome> {
    let scanner = SkillScanner::new();
    let skill_dirs = scanner.scan(dir);

    skill_dirs
        .into_iter()
        .map(|skill_dir| load_single_skill(&skill_dir, dir))
        .collect()
}

/// Loads skills from multiple root directories.
///
/// Scans each root for skill directories and loads them. All results
/// are concatenated.
pub fn load_all_skills(roots: &[PathBuf]) -> Vec<SkillLoadOutcome> {
    let mut outcomes = Vec::new();
    for root in roots {
        if root.is_dir() {
            let loaded = load_skills_from_dir(root);
            tracing::debug!(
                root = %root.display(),
                loaded = loaded.len(),
                success = loaded.iter().filter(|o| o.is_success()).count(),
                "loaded skills from root"
            );
            outcomes.extend(loaded);
        } else {
            tracing::debug!(
                root = %root.display(),
                "skill root does not exist or is not a directory"
            );
        }
    }
    outcomes
}

/// Loads a single skill from its directory.
fn load_single_skill(skill_dir: &Path, root: &Path) -> SkillLoadOutcome {
    let toml_path = skill_dir.join(SKILL_TOML);

    // Read SKILL.toml
    let toml_content = match fs::read_to_string(&toml_path) {
        Ok(content) => content,
        Err(err) => {
            return SkillLoadOutcome::Failed {
                path: skill_dir.to_path_buf(),
                error: format!("failed to read {SKILL_TOML}: {err}"),
            };
        }
    };

    // Parse SKILL.toml
    let interface: SkillInterface = match toml::from_str(&toml_content) {
        Ok(iface) => iface,
        Err(err) => {
            return SkillLoadOutcome::Failed {
                path: skill_dir.to_path_buf(),
                error: format!("failed to parse {SKILL_TOML}: {err}"),
            };
        }
    };

    // Validate
    if let Err(errors) = validator::validate_skill(&interface) {
        return SkillLoadOutcome::Failed {
            path: skill_dir.to_path_buf(),
            error: format!("validation failed: {}", errors.join("; ")),
        };
    }

    // Resolve prompt content
    let prompt = match resolve_prompt(skill_dir, &interface) {
        Ok(p) => p,
        Err(err) => {
            return SkillLoadOutcome::Failed {
                path: skill_dir.to_path_buf(),
                error: format!("failed to resolve prompt: {err}"),
            };
        }
    };

    // Determine source based on relationship to root
    let source = determine_source(skill_dir, root);

    SkillLoadOutcome::Success {
        skill: SkillPromptCommand {
            name: interface.name,
            description: interface.description,
            prompt,
            allowed_tools: interface.allowed_tools,
        },
        source,
    }
}

/// Resolves the prompt text for a skill.
///
/// If `prompt_file` is set, reads the file relative to the skill directory.
/// Otherwise, falls back to `prompt_inline`.
fn resolve_prompt(skill_dir: &Path, interface: &SkillInterface) -> Result<String, String> {
    // prompt_file takes precedence over prompt_inline
    if let Some(ref file) = interface.prompt_file {
        if !file.is_empty() {
            let prompt_path = skill_dir.join(file);
            return fs::read_to_string(&prompt_path).map_err(|err| {
                format!(
                    "failed to read prompt file '{}': {err}",
                    prompt_path.display()
                )
            });
        }
    }

    if let Some(ref inline) = interface.prompt_inline {
        if !inline.is_empty() {
            return Ok(inline.clone());
        }
    }

    Err("no prompt content available".to_string())
}

/// Determines the [`SkillSource`] based on the skill directory and its root.
fn determine_source(skill_dir: &Path, root: &Path) -> SkillSource {
    // Use the root path as a heuristic:
    // - If root contains ".cocode/skills" it is project-local
    // - If root contains home dir patterns, it is user-global
    // - Otherwise default to project-local
    let root_str = root.to_string_lossy();
    if root_str.contains(".cocode/skills") || root_str.contains(".cocode\\skills") {
        SkillSource::ProjectLocal {
            path: skill_dir.to_path_buf(),
        }
    } else {
        SkillSource::UserGlobal {
            path: skill_dir.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_skills_from_dir_success() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        let skill_dir = root.join("commit");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        fs::write(
            skill_dir.join("SKILL.toml"),
            r#"
name = "commit"
description = "Generate a commit message"
prompt_inline = "Look at staged changes and generate a commit message."
allowed_tools = ["Bash"]
"#,
        )
        .expect("write SKILL.toml");

        let outcomes = load_skills_from_dir(root);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].is_success());
        assert_eq!(outcomes[0].skill_name(), Some("commit"));
    }

    #[test]
    fn test_load_skills_from_dir_with_prompt_file() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        let skill_dir = root.join("review");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        fs::write(
            skill_dir.join("SKILL.toml"),
            r#"
name = "review"
description = "Review code"
prompt_file = "prompt.md"
"#,
        )
        .expect("write SKILL.toml");
        fs::write(
            skill_dir.join("prompt.md"),
            "Please review the following code changes carefully.",
        )
        .expect("write prompt.md");

        let outcomes = load_skills_from_dir(root);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].is_success());

        if let SkillLoadOutcome::Success { skill, .. } = &outcomes[0] {
            assert_eq!(skill.name, "review");
            assert_eq!(
                skill.prompt,
                "Please review the following code changes carefully."
            );
        }
    }

    #[test]
    fn test_load_skills_from_dir_missing_prompt_file() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        let skill_dir = root.join("bad");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        fs::write(
            skill_dir.join("SKILL.toml"),
            r#"
name = "bad"
description = "Bad skill"
prompt_file = "nonexistent.md"
"#,
        )
        .expect("write SKILL.toml");

        let outcomes = load_skills_from_dir(root);
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].is_success());
    }

    #[test]
    fn test_load_skills_from_dir_invalid_toml() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        let skill_dir = root.join("broken");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        fs::write(skill_dir.join("SKILL.toml"), "this is not valid toml {{{}}")
            .expect("write SKILL.toml");

        let outcomes = load_skills_from_dir(root);
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].is_success());
    }

    #[test]
    fn test_load_skills_from_dir_validation_failure() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        let skill_dir = root.join("invalid");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        // Empty name should fail validation
        fs::write(
            skill_dir.join("SKILL.toml"),
            r#"
name = ""
description = "Invalid"
prompt_inline = "text"
"#,
        )
        .expect("write SKILL.toml");

        let outcomes = load_skills_from_dir(root);
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].is_success());
    }

    #[test]
    fn test_load_skills_fail_open() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        // Good skill
        let good = root.join("good");
        fs::create_dir_all(&good).expect("mkdir");
        fs::write(
            good.join("SKILL.toml"),
            "name = \"good\"\ndescription = \"Works\"\nprompt_inline = \"do it\"",
        )
        .expect("write");

        // Bad skill
        let bad = root.join("bad");
        fs::create_dir_all(&bad).expect("mkdir");
        fs::write(bad.join("SKILL.toml"), "garbage {{{}}").expect("write");

        let outcomes = load_skills_from_dir(root);
        assert_eq!(outcomes.len(), 2);

        let successes: Vec<_> = outcomes.iter().filter(|o| o.is_success()).collect();
        let failures: Vec<_> = outcomes.iter().filter(|o| !o.is_success()).collect();
        assert_eq!(successes.len(), 1);
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn test_load_all_skills_multiple_roots() {
        let tmp1 = tempfile::tempdir().expect("create temp dir");
        let tmp2 = tempfile::tempdir().expect("create temp dir");

        let skill1 = tmp1.path().join("s1");
        fs::create_dir_all(&skill1).expect("mkdir");
        fs::write(
            skill1.join("SKILL.toml"),
            "name = \"s1\"\ndescription = \"d\"\nprompt_inline = \"p\"",
        )
        .expect("write");

        let skill2 = tmp2.path().join("s2");
        fs::create_dir_all(&skill2).expect("mkdir");
        fs::write(
            skill2.join("SKILL.toml"),
            "name = \"s2\"\ndescription = \"d\"\nprompt_inline = \"p\"",
        )
        .expect("write");

        let roots = vec![tmp1.path().to_path_buf(), tmp2.path().to_path_buf()];
        let outcomes = load_all_skills(&roots);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.is_success()));
    }

    #[test]
    fn test_load_all_skills_nonexistent_root() {
        let roots = vec![PathBuf::from("/nonexistent/xyz")];
        let outcomes = load_all_skills(&roots);
        assert!(outcomes.is_empty());
    }

    #[test]
    fn test_determine_source_project_local() {
        let source = determine_source(
            Path::new("/project/.cocode/skills/commit"),
            Path::new("/project/.cocode/skills"),
        );
        assert!(matches!(source, SkillSource::ProjectLocal { .. }));
    }

    #[test]
    fn test_determine_source_user_global() {
        let source = determine_source(
            Path::new("/home/user/.config/cocode/skills/review"),
            Path::new("/home/user/.config/cocode/skills"),
        );
        assert!(matches!(source, SkillSource::UserGlobal { .. }));
    }
}
