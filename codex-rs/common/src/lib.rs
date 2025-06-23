#[cfg(feature = "cli")]
mod approval_mode_cli_arg;

#[cfg(feature = "elapsed")]
pub mod elapsed;

#[cfg(feature = "cli")]
pub use approval_mode_cli_arg::ApprovalModeCliArg;
#[cfg(feature = "cli")]
pub use approval_mode_cli_arg::SandboxPermissionOption;

#[cfg(any(feature = "cli", test))]
mod config_override;

#[cfg(feature = "cli")]
pub use config_override::CliConfigOverrides;

// -------------------------------------------------------------------------
//  Optional helpers for querying the list of available models.
// -------------------------------------------------------------------------

#[cfg(feature = "model-list")]
mod model_list;

#[cfg(feature = "model-list")]
pub use model_list::fetch_available_models;
