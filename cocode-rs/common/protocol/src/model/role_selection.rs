//! Runtime role selection types.
//!
//! Defines `RoleSelection` (model + thinking level for a single role) and
//! `RoleSelections` (all roles' runtime selections).

use super::ModelRole;
use super::ModelSpec;
use crate::ThinkingLevel;
use serde::Deserialize;
use serde::Serialize;

/// Runtime selection for a single role: current model + current thinking level.
///
/// `RoleSelection` is used for runtime state (in-memory switching), distinct from
/// `ModelRoles` which is used for JSON configuration (persisted to disk).
///
/// # Example
///
/// ```
/// use cocode_protocol::model::{RoleSelection, ModelSpec};
/// use cocode_protocol::ThinkingLevel;
///
/// // Create with just model
/// let selection = RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4"));
///
/// // Create with model + thinking level
/// let selection = RoleSelection::with_thinking(
///     ModelSpec::new("anthropic", "claude-opus-4"),
///     ThinkingLevel::high(),
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleSelection {
    /// Current model (provider/model).
    pub model: ModelSpec,

    /// Current thinking level (overrides ModelInfo.default_thinking_level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<ThinkingLevel>,
}

impl RoleSelection {
    /// Create a new role selection with only the model.
    pub fn new(model: ModelSpec) -> Self {
        Self {
            model,
            thinking_level: None,
        }
    }

    /// Create a role selection with model and thinking level.
    pub fn with_thinking(model: ModelSpec, level: ThinkingLevel) -> Self {
        Self {
            model,
            thinking_level: Some(level),
        }
    }

    /// Create from a ModelSpec (uses model's default thinking level).
    pub fn from_spec(spec: ModelSpec) -> Self {
        Self::new(spec)
    }

    /// Update the thinking level.
    pub fn set_thinking_level(&mut self, level: ThinkingLevel) {
        self.thinking_level = Some(level);
    }

    /// Clear the thinking level override.
    pub fn clear_thinking_level(&mut self) {
        self.thinking_level = None;
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.model.provider
    }

    /// Get the model name.
    pub fn model_name(&self) -> &str {
        &self.model.model
    }
}

/// All roles' runtime selections.
///
/// Unlike `ModelRoles` which uses `ModelSpec` for JSON config, `RoleSelections`
/// uses `RoleSelection` which includes the current thinking level override.
///
/// # Example
///
/// ```
/// use cocode_protocol::model::{RoleSelections, RoleSelection, ModelRole, ModelSpec};
/// use cocode_protocol::ThinkingLevel;
///
/// let mut selections = RoleSelections::default();
///
/// // Set main role
/// selections.set(
///     ModelRole::Main,
///     RoleSelection::with_thinking(
///         ModelSpec::new("anthropic", "claude-opus-4"),
///         ThinkingLevel::high(),
///     ),
/// );
///
/// // Set fast role
/// selections.set(
///     ModelRole::Fast,
///     RoleSelection::new(ModelSpec::new("anthropic", "claude-haiku")),
/// );
///
/// // Get selection for a role
/// if let Some(main) = selections.get(ModelRole::Main) {
///     println!("Main: {}", main.model);
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoleSelections {
    /// Main role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<RoleSelection>,

    /// Fast role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<RoleSelection>,

    /// Vision role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<RoleSelection>,

    /// Review role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<RoleSelection>,

    /// Plan role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<RoleSelection>,

    /// Explore role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explore: Option<RoleSelection>,

    /// Compact role selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<RoleSelection>,
}

impl RoleSelections {
    /// Create empty selections.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with only the main role set.
    pub fn with_main(selection: RoleSelection) -> Self {
        Self {
            main: Some(selection),
            ..Default::default()
        }
    }

    /// Get selection for a specific role.
    pub fn get(&self, role: ModelRole) -> Option<&RoleSelection> {
        match role {
            ModelRole::Main => self.main.as_ref(),
            ModelRole::Fast => self.fast.as_ref(),
            ModelRole::Vision => self.vision.as_ref(),
            ModelRole::Review => self.review.as_ref(),
            ModelRole::Plan => self.plan.as_ref(),
            ModelRole::Explore => self.explore.as_ref(),
            ModelRole::Compact => self.compact.as_ref(),
        }
    }

    /// Get mutable selection for a specific role.
    pub fn get_mut(&mut self, role: ModelRole) -> Option<&mut RoleSelection> {
        match role {
            ModelRole::Main => self.main.as_mut(),
            ModelRole::Fast => self.fast.as_mut(),
            ModelRole::Vision => self.vision.as_mut(),
            ModelRole::Review => self.review.as_mut(),
            ModelRole::Plan => self.plan.as_mut(),
            ModelRole::Explore => self.explore.as_mut(),
            ModelRole::Compact => self.compact.as_mut(),
        }
    }

    /// Set selection for a specific role.
    pub fn set(&mut self, role: ModelRole, selection: RoleSelection) {
        match role {
            ModelRole::Main => self.main = Some(selection),
            ModelRole::Fast => self.fast = Some(selection),
            ModelRole::Vision => self.vision = Some(selection),
            ModelRole::Review => self.review = Some(selection),
            ModelRole::Plan => self.plan = Some(selection),
            ModelRole::Explore => self.explore = Some(selection),
            ModelRole::Compact => self.compact = Some(selection),
        }
    }

    /// Clear selection for a specific role.
    pub fn clear(&mut self, role: ModelRole) {
        match role {
            ModelRole::Main => self.main = None,
            ModelRole::Fast => self.fast = None,
            ModelRole::Vision => self.vision = None,
            ModelRole::Review => self.review = None,
            ModelRole::Plan => self.plan = None,
            ModelRole::Explore => self.explore = None,
            ModelRole::Compact => self.compact = None,
        }
    }

    /// Check if any role is selected.
    pub fn is_empty(&self) -> bool {
        self.main.is_none()
            && self.fast.is_none()
            && self.vision.is_none()
            && self.review.is_none()
            && self.plan.is_none()
            && self.explore.is_none()
            && self.compact.is_none()
    }

    /// Get selection for a role, falling back to main if not set.
    pub fn get_or_main(&self, role: ModelRole) -> Option<&RoleSelection> {
        self.get(role).or(self.main.as_ref())
    }

    /// Update thinking level for a specific role.
    ///
    /// Returns `true` if the role selection exists and was updated.
    pub fn set_thinking_level(&mut self, role: ModelRole, level: ThinkingLevel) -> bool {
        if let Some(selection) = self.get_mut(role) {
            selection.set_thinking_level(level);
            true
        } else {
            false
        }
    }

    /// Merge another RoleSelections into this one.
    ///
    /// Values from `other` take precedence where set.
    pub fn merge(&mut self, other: &RoleSelections) {
        if other.main.is_some() {
            self.main = other.main.clone();
        }
        if other.fast.is_some() {
            self.fast = other.fast.clone();
        }
        if other.vision.is_some() {
            self.vision = other.vision.clone();
        }
        if other.review.is_some() {
            self.review = other.review.clone();
        }
        if other.plan.is_some() {
            self.plan = other.plan.clone();
        }
        if other.explore.is_some() {
            self.explore = other.explore.clone();
        }
        if other.compact.is_some() {
            self.compact = other.compact.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_selection_new() {
        let selection = RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4"));
        assert_eq!(selection.model.provider, "anthropic");
        assert_eq!(selection.model.model, "claude-opus-4");
        assert!(selection.thinking_level.is_none());
    }

    #[test]
    fn test_role_selection_with_thinking() {
        let selection = RoleSelection::with_thinking(
            ModelSpec::new("anthropic", "claude-opus-4"),
            ThinkingLevel::high(),
        );
        assert_eq!(selection.model.provider, "anthropic");
        assert!(selection.thinking_level.is_some());
        assert_eq!(
            selection.thinking_level.as_ref().unwrap().effort,
            crate::model::ReasoningEffort::High
        );
    }

    #[test]
    fn test_role_selection_set_thinking_level() {
        let mut selection = RoleSelection::new(ModelSpec::new("openai", "gpt-5"));
        assert!(selection.thinking_level.is_none());

        selection.set_thinking_level(ThinkingLevel::medium());
        assert!(selection.thinking_level.is_some());

        selection.clear_thinking_level();
        assert!(selection.thinking_level.is_none());
    }

    #[test]
    fn test_role_selections_default() {
        let selections = RoleSelections::default();
        assert!(selections.is_empty());
        assert!(selections.get(ModelRole::Main).is_none());
    }

    #[test]
    fn test_role_selections_with_main() {
        let selection = RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4"));
        let selections = RoleSelections::with_main(selection.clone());

        assert!(!selections.is_empty());
        assert_eq!(selections.get(ModelRole::Main), Some(&selection));
        assert!(selections.get(ModelRole::Fast).is_none());
    }

    #[test]
    fn test_role_selections_set_and_get() {
        let mut selections = RoleSelections::default();

        selections.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );
        selections.set(
            ModelRole::Fast,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-haiku")),
        );

        assert_eq!(
            selections.get(ModelRole::Main).unwrap().model.model,
            "claude-opus-4"
        );
        assert_eq!(
            selections.get(ModelRole::Fast).unwrap().model.model,
            "claude-haiku"
        );
    }

    #[test]
    fn test_role_selections_get_or_main() {
        let mut selections = RoleSelections::default();

        selections.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );

        // Fast not set, falls back to main
        let fast = selections.get_or_main(ModelRole::Fast);
        assert!(fast.is_some());
        assert_eq!(fast.unwrap().model.model, "claude-opus-4");

        // Set fast explicitly
        selections.set(
            ModelRole::Fast,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-haiku")),
        );

        let fast = selections.get_or_main(ModelRole::Fast);
        assert_eq!(fast.unwrap().model.model, "claude-haiku");
    }

    #[test]
    fn test_role_selections_set_thinking_level() {
        let mut selections = RoleSelections::default();

        // No selection yet, should return false
        assert!(!selections.set_thinking_level(ModelRole::Main, ThinkingLevel::high()));

        // Add selection
        selections.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );

        // Now should succeed
        assert!(selections.set_thinking_level(ModelRole::Main, ThinkingLevel::high()));
        assert!(
            selections
                .get(ModelRole::Main)
                .unwrap()
                .thinking_level
                .is_some()
        );
    }

    #[test]
    fn test_role_selections_clear() {
        let mut selections = RoleSelections::default();
        selections.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );

        assert!(!selections.is_empty());

        selections.clear(ModelRole::Main);
        assert!(selections.is_empty());
    }

    #[test]
    fn test_role_selections_merge() {
        let mut base = RoleSelections::default();
        base.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );
        base.set(
            ModelRole::Fast,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-haiku")),
        );

        let mut other = RoleSelections::default();
        other.set(
            ModelRole::Fast,
            RoleSelection::new(ModelSpec::new("openai", "gpt-4o-mini")),
        );
        other.set(
            ModelRole::Vision,
            RoleSelection::new(ModelSpec::new("openai", "gpt-4o")),
        );

        base.merge(&other);

        // main unchanged
        assert_eq!(
            base.get(ModelRole::Main).unwrap().model.model,
            "claude-opus-4"
        );
        // fast overridden
        assert_eq!(
            base.get(ModelRole::Fast).unwrap().model.model,
            "gpt-4o-mini"
        );
        // vision added
        assert_eq!(base.get(ModelRole::Vision).unwrap().model.model, "gpt-4o");
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut selections = RoleSelections::default();
        selections.set(
            ModelRole::Main,
            RoleSelection::with_thinking(
                ModelSpec::new("anthropic", "claude-opus-4"),
                ThinkingLevel::high().set_budget(32000),
            ),
        );
        selections.set(
            ModelRole::Fast,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-haiku")),
        );

        let json = serde_json::to_string(&selections).unwrap();
        let parsed: RoleSelections = serde_json::from_str(&json).unwrap();

        assert_eq!(selections, parsed);
    }

    #[test]
    fn test_serde_skip_none_fields() {
        let selection = RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4"));
        let json = serde_json::to_string(&selection).unwrap();

        // thinking_level should be skipped when None
        assert!(!json.contains("thinking_level"));

        // Empty selections should serialize to {}
        let selections = RoleSelections::default();
        let json = serde_json::to_string(&selections).unwrap();
        assert_eq!(json, "{}");
    }
}
