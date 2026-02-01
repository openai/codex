//! Skill deduplication.
//!
//! When skills are loaded from multiple sources, duplicate names may appear.
//! This module provides name-based deduplication that keeps the first
//! occurrence of each skill name (respecting source precedence order).

use crate::outcome::SkillLoadOutcome;
use std::collections::HashSet;

/// Tracks seen skill names for deduplication.
///
/// # Example
///
/// ```
/// # use cocode_skill::SkillDeduplicator;
/// let mut dedup = SkillDeduplicator::new();
/// assert!(!dedup.is_duplicate("commit"));
/// assert!(dedup.is_duplicate("commit")); // second time is duplicate
/// ```
pub struct SkillDeduplicator {
    seen: HashSet<String>,
}

impl SkillDeduplicator {
    /// Creates a new empty deduplicator.
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Returns `true` if the name has already been seen.
    ///
    /// If the name is new, records it and returns `false`.
    /// If the name was already recorded, returns `true`.
    pub fn is_duplicate(&mut self, name: &str) -> bool {
        !self.seen.insert(name.to_string())
    }

    /// Returns the number of unique names seen so far.
    pub fn len(&self) -> i32 {
        self.seen.len() as i32
    }

    /// Returns `true` if no names have been recorded.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl Default for SkillDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

/// Deduplicates a list of skill load outcomes by name.
///
/// Keeps the first successful occurrence of each skill name. Failed
/// outcomes are always kept (they have no name to dedup on). Later
/// duplicates are logged at debug level and dropped.
pub fn dedup_skills(skills: Vec<SkillLoadOutcome>) -> Vec<SkillLoadOutcome> {
    let mut dedup = SkillDeduplicator::new();
    let mut result = Vec::with_capacity(skills.len());

    for outcome in skills {
        match outcome.skill_name() {
            Some(name) => {
                if dedup.is_duplicate(name) {
                    tracing::debug!(name = name, "dropping duplicate skill");
                } else {
                    result.push(outcome);
                }
            }
            None => {
                // Failed outcomes are always kept for diagnostics
                result.push(outcome);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SkillPromptCommand;
    use crate::source::SkillSource;
    use std::path::PathBuf;

    fn make_success(name: &str) -> SkillLoadOutcome {
        SkillLoadOutcome::Success {
            skill: SkillPromptCommand {
                name: name.to_string(),
                description: "desc".to_string(),
                prompt: "prompt".to_string(),
                allowed_tools: None,
                interface: None,
            },
            source: SkillSource::Bundled,
        }
    }

    fn make_failed(path: &str) -> SkillLoadOutcome {
        SkillLoadOutcome::Failed {
            path: PathBuf::from(path),
            error: "error".to_string(),
        }
    }

    #[test]
    fn test_deduplicator_basic() {
        let mut dedup = SkillDeduplicator::new();
        assert!(dedup.is_empty());
        assert!(!dedup.is_duplicate("a"));
        assert_eq!(dedup.len(), 1);
        assert!(dedup.is_duplicate("a"));
        assert_eq!(dedup.len(), 1);
        assert!(!dedup.is_duplicate("b"));
        assert_eq!(dedup.len(), 2);
    }

    #[test]
    fn test_dedup_skills_no_duplicates() {
        let skills = vec![make_success("a"), make_success("b"), make_success("c")];
        let result = dedup_skills(skills);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_dedup_skills_removes_duplicates() {
        let skills = vec![
            make_success("a"),
            make_success("b"),
            make_success("a"), // duplicate
            make_success("c"),
            make_success("b"), // duplicate
        ];
        let result = dedup_skills(skills);
        assert_eq!(result.len(), 3);

        let names: Vec<_> = result.iter().filter_map(|o| o.skill_name()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_dedup_skills_keeps_first_occurrence() {
        let skills = vec![
            SkillLoadOutcome::Success {
                skill: SkillPromptCommand {
                    name: "x".to_string(),
                    description: "first".to_string(),
                    prompt: "first prompt".to_string(),
                    allowed_tools: None,
                    interface: None,
                },
                source: SkillSource::ProjectLocal {
                    path: PathBuf::from("/first"),
                },
            },
            SkillLoadOutcome::Success {
                skill: SkillPromptCommand {
                    name: "x".to_string(),
                    description: "second".to_string(),
                    prompt: "second prompt".to_string(),
                    allowed_tools: None,
                    interface: None,
                },
                source: SkillSource::UserGlobal {
                    path: PathBuf::from("/second"),
                },
            },
        ];
        let result = dedup_skills(skills);
        assert_eq!(result.len(), 1);

        if let SkillLoadOutcome::Success { skill, .. } = &result[0] {
            assert_eq!(skill.description, "first");
            assert_eq!(skill.prompt, "first prompt");
        } else {
            panic!("expected Success outcome");
        }
    }

    #[test]
    fn test_dedup_skills_keeps_failures() {
        let skills = vec![
            make_success("a"),
            make_failed("/bad1"),
            make_success("a"), // duplicate
            make_failed("/bad2"),
        ];
        let result = dedup_skills(skills);
        assert_eq!(result.len(), 3); // 1 success + 2 failures

        let successes = result.iter().filter(|o| o.is_success()).count();
        let failures = result.iter().filter(|o| !o.is_success()).count();
        assert_eq!(successes, 1);
        assert_eq!(failures, 2);
    }

    #[test]
    fn test_dedup_skills_empty() {
        let result = dedup_skills(Vec::new());
        assert!(result.is_empty());
    }
}
