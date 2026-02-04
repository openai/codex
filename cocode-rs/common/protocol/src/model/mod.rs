//! Model configuration types.

mod capability;
mod model_info;
mod model_roles;
mod model_spec;
mod reasoning;
mod role_selection;
mod shell_type;
mod tool;
mod truncation;

pub use capability::Capability;
pub use model_info::ModelInfo;
pub use model_info::override_keys;
pub use model_roles::ModelRole;
pub use model_roles::ModelRoles;
pub use model_spec::ModelSpec;
pub use model_spec::ModelSpecParseError;
pub use model_spec::resolve_provider_type;
pub use reasoning::ReasoningEffort;
pub use reasoning::ReasoningSummary;
pub use reasoning::nearest_effort;
pub use role_selection::RoleSelection;
pub use role_selection::RoleSelections;
pub use shell_type::ConfigShellToolType;
pub use truncation::TruncationMode;
pub use truncation::TruncationPolicyConfig;
