#![cfg(target_os = "macos")]

pub(crate) use codex_sandbox::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
pub(crate) use codex_sandbox::create_seatbelt_command_args_for_policies_with_extensions;
pub(crate) use codex_sandbox::spawn_command_under_seatbelt;
