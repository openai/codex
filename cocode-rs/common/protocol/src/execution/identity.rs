//! Model addressing modes for inference requests.

use crate::model::ModelRole;
use crate::model::ModelSpec;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// Model addressing mode - describes "how to find a model".
///
/// `ExecutionIdentity` replaces the scattered `model: Option<String>` pattern
/// throughout the codebase with an explicit, type-safe way to express model
/// selection intent.
///
/// # Variants
///
/// - `Role(ModelRole)`: Dynamic selection via configured role mapping (e.g., Plan, Explore)
/// - `Spec(ModelSpec)`: Static selection with explicit provider/model
/// - `Inherit`: Use the parent context's model (for subagents)
///
/// # Example
///
/// ```
/// use cocode_protocol::execution::ExecutionIdentity;
/// use cocode_protocol::model::{ModelRole, ModelSpec};
///
/// // Dynamic: resolve via role configuration
/// let plan_identity = ExecutionIdentity::Role(ModelRole::Plan);
///
/// // Static: force specific model
/// let specific = ExecutionIdentity::Spec(ModelSpec::new("anthropic", "claude-haiku"));
///
/// // Inherit: use parent's model
/// let inherit = ExecutionIdentity::Inherit;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ExecutionIdentity {
    /// Dynamic: resolve via configured role mapping.
    ///
    /// The actual model is looked up from `RoleSelections` at runtime,
    /// with fallback to Main role if the specific role is not configured.
    Role(ModelRole),

    /// Static: explicit provider/model specification.
    ///
    /// Use this when you need to force a specific model regardless of
    /// role configuration (e.g., user explicitly requested a model).
    Spec(ModelSpec),

    /// Inherit: use the parent context's model.
    ///
    /// For subagents, this means using the same model as the spawning agent.
    /// This makes the inheritance explicit rather than implicit via `None`.
    Inherit,
}

impl ExecutionIdentity {
    /// Create a role-based identity.
    pub fn role(role: ModelRole) -> Self {
        Self::Role(role)
    }

    /// Create a spec-based identity.
    pub fn spec(spec: ModelSpec) -> Self {
        Self::Spec(spec)
    }

    /// Create an inheriting identity.
    pub fn inherit() -> Self {
        Self::Inherit
    }

    /// Convenience: main role identity.
    pub fn main() -> Self {
        Self::Role(ModelRole::Main)
    }

    /// Convenience: fast role identity.
    pub fn fast() -> Self {
        Self::Role(ModelRole::Fast)
    }

    /// Convenience: plan role identity.
    pub fn plan() -> Self {
        Self::Role(ModelRole::Plan)
    }

    /// Convenience: explore role identity.
    pub fn explore() -> Self {
        Self::Role(ModelRole::Explore)
    }

    /// Convenience: compact role identity.
    pub fn compact() -> Self {
        Self::Role(ModelRole::Compact)
    }

    /// Check if this identity requires a parent context to resolve.
    pub fn requires_parent(&self) -> bool {
        matches!(self, Self::Inherit)
    }

    /// Check if this is a role-based identity.
    pub fn is_role(&self) -> bool {
        matches!(self, Self::Role(_))
    }

    /// Check if this is a spec-based identity.
    pub fn is_spec(&self) -> bool {
        matches!(self, Self::Spec(_))
    }

    /// Get the role if this is a role-based identity.
    pub fn as_role(&self) -> Option<ModelRole> {
        match self {
            Self::Role(role) => Some(*role),
            _ => None,
        }
    }

    /// Get the spec if this is a spec-based identity.
    pub fn as_spec(&self) -> Option<&ModelSpec> {
        match self {
            Self::Spec(spec) => Some(spec),
            _ => None,
        }
    }
}

impl Default for ExecutionIdentity {
    /// Default to main role.
    fn default() -> Self {
        Self::Role(ModelRole::Main)
    }
}

impl fmt::Display for ExecutionIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Role(role) => write!(f, "role:{}", role),
            Self::Spec(spec) => write!(f, "spec:{}", spec),
            Self::Inherit => write!(f, "inherit"),
        }
    }
}

impl From<ModelRole> for ExecutionIdentity {
    fn from(role: ModelRole) -> Self {
        Self::Role(role)
    }
}

impl From<ModelSpec> for ExecutionIdentity {
    fn from(spec: ModelSpec) -> Self {
        Self::Spec(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_identity() {
        let identity = ExecutionIdentity::role(ModelRole::Plan);
        assert!(identity.is_role());
        assert!(!identity.is_spec());
        assert!(!identity.requires_parent());
        assert_eq!(identity.as_role(), Some(ModelRole::Plan));
        assert_eq!(identity.to_string(), "role:plan");
    }

    #[test]
    fn test_spec_identity() {
        let spec = ModelSpec::new("anthropic", "claude-haiku");
        let identity = ExecutionIdentity::spec(spec.clone());
        assert!(identity.is_spec());
        assert!(!identity.is_role());
        assert!(!identity.requires_parent());
        assert_eq!(identity.as_spec(), Some(&spec));
        assert_eq!(identity.to_string(), "spec:anthropic/claude-haiku");
    }

    #[test]
    fn test_inherit_identity() {
        let identity = ExecutionIdentity::inherit();
        assert!(!identity.is_role());
        assert!(!identity.is_spec());
        assert!(identity.requires_parent());
        assert_eq!(identity.to_string(), "inherit");
    }

    #[test]
    fn test_convenience_constructors() {
        assert_eq!(
            ExecutionIdentity::main(),
            ExecutionIdentity::Role(ModelRole::Main)
        );
        assert_eq!(
            ExecutionIdentity::fast(),
            ExecutionIdentity::Role(ModelRole::Fast)
        );
        assert_eq!(
            ExecutionIdentity::plan(),
            ExecutionIdentity::Role(ModelRole::Plan)
        );
        assert_eq!(
            ExecutionIdentity::explore(),
            ExecutionIdentity::Role(ModelRole::Explore)
        );
        assert_eq!(
            ExecutionIdentity::compact(),
            ExecutionIdentity::Role(ModelRole::Compact)
        );
    }

    #[test]
    fn test_default() {
        assert_eq!(
            ExecutionIdentity::default(),
            ExecutionIdentity::Role(ModelRole::Main)
        );
    }

    #[test]
    fn test_from_role() {
        let identity: ExecutionIdentity = ModelRole::Plan.into();
        assert_eq!(identity, ExecutionIdentity::Role(ModelRole::Plan));
    }

    #[test]
    fn test_from_spec() {
        let spec = ModelSpec::new("openai", "gpt-5");
        let identity: ExecutionIdentity = spec.clone().into();
        assert_eq!(identity, ExecutionIdentity::Spec(spec));
    }

    #[test]
    fn test_serde_role() {
        let identity = ExecutionIdentity::role(ModelRole::Plan);
        let json = serde_json::to_string(&identity).unwrap();
        assert!(json.contains("Role"));
        let parsed: ExecutionIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity, parsed);
    }

    #[test]
    fn test_serde_spec() {
        let identity = ExecutionIdentity::spec(ModelSpec::new("anthropic", "claude-haiku"));
        let json = serde_json::to_string(&identity).unwrap();
        assert!(json.contains("Spec"));
        let parsed: ExecutionIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity, parsed);
    }

    #[test]
    fn test_serde_inherit() {
        let identity = ExecutionIdentity::inherit();
        let json = serde_json::to_string(&identity).unwrap();
        assert!(json.contains("Inherit"));
        let parsed: ExecutionIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity, parsed);
    }
}
