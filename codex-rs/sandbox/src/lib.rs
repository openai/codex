#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod error;
pub mod exec;
pub mod landlock;
pub mod sandboxing;
pub mod seatbelt;
pub mod spawn;
pub mod windows_sandbox;
pub mod windows_sandbox_read_grants;

pub use exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
pub use exec::ExecCapturePolicy;
pub use exec::ExecExpiration;
pub use exec::ExecParams;
pub use exec::ExecToolCallOutput;
pub use exec::IO_DRAIN_TIMEOUT_MS;
pub use exec::MAX_EXEC_OUTPUT_DELTAS_PER_CALL;
pub use exec::SandboxType;
pub use exec::StdoutStream;
pub use exec::StreamOutput;
pub use exec::build_exec_request;
pub use exec::execute_exec_request;
pub use exec::is_likely_sandbox_denied;
pub use exec::process_exec_tool_call;
pub use landlock::allow_network_for_proxy;
pub use landlock::create_linux_sandbox_command_args_for_policies;
pub use landlock::spawn_command_under_linux_sandbox;
pub use sandboxing::CommandSpec;
pub use sandboxing::ExecRequest;
pub use sandboxing::SandboxManager;
pub use sandboxing::SandboxPermissions;
pub use sandboxing::SandboxPreference;
pub use sandboxing::SandboxTransformError;
pub use sandboxing::effective_file_system_sandbox_policy;
pub use sandboxing::execute_env;
pub use sandboxing::execute_exec_request_with_after_spawn;
pub use sandboxing::intersect_permission_profiles;
pub use sandboxing::merge_permission_profiles;
pub use sandboxing::normalize_additional_permissions;
pub use spawn::CODEX_SANDBOX_ENV_VAR;
pub use spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
pub use spawn::StdioPolicy;
pub use windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED;
pub use windows_sandbox::WindowsSandboxLevelExt;
pub use windows_sandbox::WindowsSandboxMode;
pub use windows_sandbox::WindowsSandboxSetupMode;
pub use windows_sandbox::WindowsSandboxSetupRequest;
pub use windows_sandbox::elevated_setup_failure_details;
pub use windows_sandbox::elevated_setup_failure_metric_name;
pub use windows_sandbox::legacy_windows_sandbox_mode;
pub use windows_sandbox::legacy_windows_sandbox_mode_from_entries;
pub use windows_sandbox::resolve_windows_sandbox_mode;
pub use windows_sandbox::resolve_windows_sandbox_private_desktop;
pub use windows_sandbox::run_elevated_setup;
pub use windows_sandbox::run_legacy_setup_preflight;
pub use windows_sandbox::run_setup_refresh_with_extra_read_roots;
pub use windows_sandbox::run_windows_sandbox_setup;
pub use windows_sandbox::sandbox_setup_is_complete;
pub use windows_sandbox::windows_sandbox_mode_tag;
pub use windows_sandbox_read_grants::grant_read_root_non_elevated;

#[cfg(target_os = "macos")]
pub use seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
#[cfg(target_os = "macos")]
pub use seatbelt::create_seatbelt_command_args_for_policies_with_extensions;
#[cfg(target_os = "macos")]
pub use seatbelt::spawn_command_under_seatbelt;

mod seatbelt_permissions;

pub fn get_platform_sandbox(windows_sandbox_enabled: bool) -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else if cfg!(target_os = "windows") {
        if windows_sandbox_enabled {
            Some(SandboxType::WindowsRestrictedToken)
        } else {
            None
        }
    } else {
        None
    }
}

pub(crate) use codex_protocol::protocol;

mod text_encoding;
