//! Skill search manager for slash command autocomplete.
//!
//! This module provides skill search functionality for the /command autocomplete
//! feature, using fuzzy matching to find skills by name.

use cocode_skill::SkillPromptCommand;
use cocode_utils_common::fuzzy_match;

use crate::state::SkillSuggestionItem;

/// Maximum number of suggestions to return.
const MAX_SUGGESTIONS: i32 = 10;

/// Information about a skill for searching.
#[derive(Debug, Clone)]
pub struct SkillInfo {
    /// Skill name (e.g., "commit").
    pub name: String,
    /// Short description.
    pub description: String,
}

impl From<&SkillPromptCommand> for SkillInfo {
    fn from(skill: &SkillPromptCommand) -> Self {
        Self {
            name: skill.name.clone(),
            description: skill.description.clone(),
        }
    }
}

/// Manages skill search with fuzzy matching.
///
/// This struct handles:
/// - Loading skills from a SkillManager
/// - Fuzzy search by skill name
/// - Dual-target matching (name and description)
#[derive(Debug, Default)]
pub struct SkillSearchManager {
    /// Loaded skill info for searching.
    skills: Vec<SkillInfo>,
}

impl SkillSearchManager {
    /// Create a new empty skill search manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a skill search manager with pre-loaded skills.
    pub fn with_skills(skills: Vec<SkillInfo>) -> Self {
        Self { skills }
    }

    /// Load skills from an iterator of skill commands.
    pub fn load_skills<'a>(&mut self, skills: impl Iterator<Item = &'a SkillPromptCommand>) {
        self.skills = skills.map(SkillInfo::from).collect();
    }

    /// Add a single skill.
    pub fn add_skill(&mut self, skill: SkillInfo) {
        self.skills.push(skill);
    }

    /// Clear all loaded skills.
    pub fn clear(&mut self) {
        self.skills.clear();
    }

    /// Get the number of loaded skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if no skills are loaded.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Search for skills matching the query.
    ///
    /// Uses fuzzy matching on skill names. Returns suggestions sorted by
    /// match score (lower score = better match).
    pub fn search(&self, query: &str) -> Vec<SkillSuggestionItem> {
        if query.is_empty() {
            // Return all skills sorted by name
            let mut suggestions: Vec<_> = self
                .skills
                .iter()
                .map(|skill| SkillSuggestionItem {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    score: i32::MAX,
                    match_indices: vec![],
                })
                .collect();
            suggestions.sort_by(|a, b| a.name.cmp(&b.name));
            suggestions.truncate(MAX_SUGGESTIONS as usize);
            return suggestions;
        }

        let mut results = Vec::new();

        for skill in &self.skills {
            // Try matching against name
            if let Some((indices, score)) = fuzzy_match(&skill.name, query) {
                results.push(SkillSuggestionItem {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    score,
                    match_indices: indices,
                });
            }
        }

        // Sort by score (ascending = better)
        results.sort_by_key(|r| r.score);

        // Limit results
        results.truncate(MAX_SUGGESTIONS as usize);

        results
    }

    /// Get all skills (for display when showing all available commands).
    pub fn all_skills(&self) -> &[SkillInfo] {
        &self.skills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_skills() -> Vec<SkillInfo> {
        vec![
            SkillInfo {
                name: "commit".to_string(),
                description: "Generate a commit message".to_string(),
            },
            SkillInfo {
                name: "review".to_string(),
                description: "Review code changes".to_string(),
            },
            SkillInfo {
                name: "test".to_string(),
                description: "Run tests".to_string(),
            },
            SkillInfo {
                name: "config".to_string(),
                description: "Configure settings".to_string(),
            },
        ]
    }

    #[test]
    fn test_search_empty_query() {
        let manager = SkillSearchManager::with_skills(create_test_skills());
        let results = manager.search("");

        // Should return all skills
        assert_eq!(results.len(), 4);
        // Should be sorted by name
        assert_eq!(results[0].name, "commit");
        assert_eq!(results[1].name, "config");
        assert_eq!(results[2].name, "review");
        assert_eq!(results[3].name, "test");
    }

    #[test]
    fn test_search_exact_match() {
        let manager = SkillSearchManager::with_skills(create_test_skills());
        let results = manager.search("commit");

        assert!(!results.is_empty());
        assert_eq!(results[0].name, "commit");
    }

    #[test]
    fn test_search_prefix_match() {
        let manager = SkillSearchManager::with_skills(create_test_skills());
        let results = manager.search("com");

        assert!(!results.is_empty());
        assert_eq!(results[0].name, "commit");
    }

    #[test]
    fn test_search_fuzzy_match() {
        let manager = SkillSearchManager::with_skills(create_test_skills());
        let results = manager.search("cmit");

        assert!(!results.is_empty());
        assert_eq!(results[0].name, "commit");
    }

    #[test]
    fn test_search_no_match() {
        let manager = SkillSearchManager::with_skills(create_test_skills());
        let results = manager.search("xyz");

        assert!(results.is_empty());
    }

    #[test]
    fn test_search_case_insensitive() {
        let manager = SkillSearchManager::with_skills(create_test_skills());
        let results = manager.search("COMMIT");

        assert!(!results.is_empty());
        assert_eq!(results[0].name, "commit");
    }

    #[test]
    fn test_add_skill() {
        let mut manager = SkillSearchManager::new();
        assert!(manager.is_empty());

        manager.add_skill(SkillInfo {
            name: "test".to_string(),
            description: "Test skill".to_string(),
        });

        assert_eq!(manager.len(), 1);
        let results = manager.search("test");
        assert_eq!(results[0].name, "test");
    }

    #[test]
    fn test_clear_skills() {
        let mut manager = SkillSearchManager::with_skills(create_test_skills());
        assert!(!manager.is_empty());

        manager.clear();
        assert!(manager.is_empty());
    }
}
