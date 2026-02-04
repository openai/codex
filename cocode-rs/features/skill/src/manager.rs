//! Skill manager for loading and executing skills.
//!
//! The [`SkillManager`] provides a convenient interface for:
//! - Loading bundled skills
//! - Loading skills from configured directories
//! - Looking up skills by name
//! - Executing skill commands by injecting prompts

use crate::bundled::bundled_skills;
use crate::command::SkillPromptCommand;
use crate::dedup::dedup_skills;
use crate::loader::load_all_skills;
use crate::outcome::SkillLoadOutcome;

use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Result of loading skills from directories.
#[derive(Debug, Default)]
pub struct SkillLoadResult {
    /// Number of skills successfully loaded.
    pub loaded: i32,
    /// Number of skills that failed to load.
    pub failed: i32,
    /// Paths of skills that failed to load (for debugging).
    pub failures: Vec<PathBuf>,
}

impl SkillLoadResult {
    /// Check if all skills loaded successfully.
    pub fn is_complete(&self) -> bool {
        self.failed == 0
    }
}

/// Manages loaded skills and provides lookup/execution functionality.
///
/// The manager loads skills from configured directories and provides
/// efficient lookup by name. Skills are deduplicated by name, with
/// later-loaded skills taking precedence.
#[derive(Default)]
pub struct SkillManager {
    /// Loaded skills indexed by name.
    skills: HashMap<String, SkillPromptCommand>,
}

impl SkillManager {
    /// Create a new empty skill manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new skill manager with bundled skills pre-loaded.
    ///
    /// Bundled skills are compiled into the binary and provide essential
    /// system commands like `/output-style`.
    pub fn with_bundled() -> Self {
        let mut manager = Self::new();
        manager.register_bundled();
        manager
    }

    /// Register all bundled skills.
    ///
    /// Bundled skills have lowest priority and will be overridden by
    /// user-defined skills with the same name.
    pub fn register_bundled(&mut self) {
        for bundled in bundled_skills() {
            debug!(
                name = %bundled.name,
                fingerprint = %bundled.fingerprint,
                "Registering bundled skill"
            );
            // Only register if not already present (user skills take precedence)
            if !self.skills.contains_key(&bundled.name) {
                self.skills.insert(
                    bundled.name.clone(),
                    SkillPromptCommand {
                        name: bundled.name,
                        description: bundled.description,
                        prompt: bundled.prompt,
                        allowed_tools: None,
                        interface: None,
                    },
                );
            }
        }
    }

    /// Load skills from the given root directories.
    ///
    /// Skills are deduplicated by name, with later roots taking precedence.
    /// Returns a [`SkillLoadResult`] with counts and any failures.
    pub fn load_from_roots(&mut self, roots: &[PathBuf]) -> SkillLoadResult {
        let outcomes = load_all_skills(roots);
        let total = outcomes.len();

        // Collect failures for reporting
        let mut failures = Vec::new();
        for outcome in &outcomes {
            if let SkillLoadOutcome::Failed { path, error } = outcome {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "Failed to load skill"
                );
                failures.push(path.clone());
            }
        }

        let success_count = outcomes.iter().filter(|o| o.is_success()).count();

        // Deduplicate by name (keeps first occurrence)
        let deduped = dedup_skills(outcomes);

        // Index by name (only successful loads)
        for outcome in deduped {
            if let SkillLoadOutcome::Success { skill, source } = outcome {
                debug!(
                    name = %skill.name,
                    source = ?source,
                    "Loaded skill"
                );
                self.skills.insert(skill.name.clone(), skill);
            }
        }

        info!(
            total = total,
            success = success_count,
            failed = failures.len(),
            deduped = self.skills.len(),
            "Skill loading complete"
        );

        SkillLoadResult {
            loaded: self.skills.len() as i32,
            failed: failures.len() as i32,
            failures,
        }
    }

    /// Register a single skill.
    ///
    /// If a skill with the same name already exists, it is replaced.
    pub fn register(&mut self, skill: SkillPromptCommand) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&SkillPromptCommand> {
        self.skills.get(name)
    }

    /// Check if a skill exists.
    pub fn has(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }

    /// Get all skill names.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.skills.keys().map(String::as_str).collect();
        names.sort();
        names
    }

    /// Get all skills.
    pub fn all(&self) -> impl Iterator<Item = &SkillPromptCommand> {
        self.skills.values()
    }

    /// Get the number of loaded skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if the manager has no skills.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Clear all loaded skills.
    pub fn clear(&mut self) {
        self.skills.clear();
    }
}

/// Result of executing a skill command.
#[derive(Debug, Clone)]
pub struct SkillExecutionResult {
    /// The skill that was executed.
    pub skill_name: String,

    /// The prompt text to inject.
    pub prompt: String,

    /// Optional tools the skill is allowed to use.
    pub allowed_tools: Option<Vec<String>>,

    /// Arguments passed to the skill (from the command line).
    pub args: String,
}

/// Parse a skill command from user input.
///
/// Returns the skill name and any arguments.
///
/// # Examples
///
/// ```
/// use cocode_skill::manager::parse_skill_command;
///
/// let (name, args) = parse_skill_command("/commit").unwrap();
/// assert_eq!(name, "commit");
/// assert_eq!(args, "");
///
/// let (name, args) = parse_skill_command("/review src/main.rs").unwrap();
/// assert_eq!(name, "review");
/// assert_eq!(args, "src/main.rs");
/// ```
pub fn parse_skill_command(input: &str) -> Option<(&str, &str)> {
    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }

    let without_slash = &input[1..];
    let mut parts = without_slash.splitn(2, char::is_whitespace);

    let name = parts.next()?;
    let args = parts.next().unwrap_or("").trim();

    Some((name, args))
}

/// Execute a skill command.
///
/// Parses the input, looks up the skill, and returns the execution result
/// containing the prompt to inject.
///
/// # Arguments
///
/// * `manager` - The skill manager to look up skills from
/// * `input` - The user input (e.g., "/commit" or "/review file.rs")
///
/// # Returns
///
/// Returns `Some(SkillExecutionResult)` if the skill was found, `None` otherwise.
pub fn execute_skill(manager: &SkillManager, input: &str) -> Option<SkillExecutionResult> {
    let (name, args) = parse_skill_command(input)?;

    let skill = manager.get(name)?;

    // Build the prompt, potentially incorporating arguments
    // If prompt contains $ARGUMENTS placeholder, replace it; otherwise append args
    let prompt = if skill.prompt.contains("$ARGUMENTS") {
        skill.prompt.replace("$ARGUMENTS", args)
    } else if args.is_empty() {
        skill.prompt.clone()
    } else {
        // Append arguments to the prompt (fallback)
        format!("{}\n\nArguments: {}", skill.prompt, args)
    };

    Some(SkillExecutionResult {
        skill_name: skill.name.clone(),
        prompt,
        allowed_tools: skill.allowed_tools.clone(),
        args: args.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_skill(name: &str, prompt: &str) -> SkillPromptCommand {
        SkillPromptCommand {
            name: name.to_string(),
            description: format!("{name} description"),
            prompt: prompt.to_string(),
            allowed_tools: None,
            interface: None,
        }
    }

    #[test]
    fn test_parse_skill_command() {
        assert_eq!(parse_skill_command("/commit"), Some(("commit", "")));
        assert_eq!(
            parse_skill_command("/review file.rs"),
            Some(("review", "file.rs"))
        );
        assert_eq!(
            parse_skill_command("/test arg1 arg2"),
            Some(("test", "arg1 arg2"))
        );
        assert_eq!(parse_skill_command("not a command"), None);
        assert_eq!(parse_skill_command(""), None);
    }

    #[test]
    fn test_manager_register_and_get() {
        let mut manager = SkillManager::new();
        manager.register(make_skill("commit", "Generate commit message"));

        assert!(manager.has("commit"));
        assert!(!manager.has("review"));

        let skill = manager.get("commit").unwrap();
        assert_eq!(skill.name, "commit");
    }

    #[test]
    fn test_manager_names() {
        let mut manager = SkillManager::new();
        manager.register(make_skill("beta", "Beta"));
        manager.register(make_skill("alpha", "Alpha"));

        let names = manager.names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_execute_skill() {
        let mut manager = SkillManager::new();
        manager.register(make_skill("commit", "Generate a commit message"));

        let result = execute_skill(&manager, "/commit").unwrap();
        assert_eq!(result.skill_name, "commit");
        assert_eq!(result.prompt, "Generate a commit message");
        assert_eq!(result.args, "");

        // With arguments
        let result = execute_skill(&manager, "/commit --amend").unwrap();
        assert!(result.prompt.contains("--amend"));
        assert_eq!(result.args, "--amend");
    }

    #[test]
    fn test_execute_skill_not_found() {
        let manager = SkillManager::new();
        assert!(execute_skill(&manager, "/nonexistent").is_none());
    }

    #[test]
    fn test_execute_skill_with_arguments_placeholder() {
        let mut manager = SkillManager::new();
        manager.register(SkillPromptCommand {
            name: "review".to_string(),
            description: "Review PR".to_string(),
            prompt: "Review PR #$ARGUMENTS".to_string(),
            allowed_tools: None,
            interface: None,
        });

        // With placeholder and args
        let result = execute_skill(&manager, "/review 123").unwrap();
        assert_eq!(result.prompt, "Review PR #123");

        // With placeholder but no args (placeholder becomes empty)
        let result = execute_skill(&manager, "/review").unwrap();
        assert_eq!(result.prompt, "Review PR #");
    }

    #[test]
    fn test_with_bundled() {
        let manager = SkillManager::with_bundled();

        // Should have output-style skill
        assert!(manager.has("output-style"));
        let skill = manager.get("output-style").unwrap();
        assert!(skill.prompt.contains("/output-style"));
    }

    #[test]
    fn test_register_bundled_does_not_override_user_skills() {
        let mut manager = SkillManager::new();

        // Register a user skill with the same name as a bundled skill
        manager.register(make_skill("output-style", "User's custom output-style"));

        // Now register bundled skills
        manager.register_bundled();

        // User skill should still be there, not overridden
        let skill = manager.get("output-style").unwrap();
        assert_eq!(skill.prompt, "User's custom output-style");
    }
}
