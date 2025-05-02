//! Standard type to use with the `--approval-mode` CLI option.
//! Available when the `cli` feature is enabled for the crate.

use std::path::PathBuf;

use clap::ArgAction;
use clap::Parser;
use clap::ValueEnum;

use crate::protocol::AskForApproval;
use crate::protocol::SandboxPermission;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ApprovalModeCliArg {
    /// Run all commands without asking for user approval.
    /// Only asks for approval if a command fails to execute, in which case it
    /// will escalate to the user to ask for un-sandboxed execution.
    OnFailure,

    /// Only run "known safe" commands (e.g. ls, cat, sed) without
    /// asking for user approval. Will escalate to the user if the model
    /// proposes a command that is not allow-listed.
    UnlessAllowListed,

    /// Never ask for user approval
    /// Execution failures are immediately returned to the model.
    Never,
}

impl From<ApprovalModeCliArg> for AskForApproval {
    fn from(value: ApprovalModeCliArg) -> Self {
        match value {
            ApprovalModeCliArg::OnFailure => AskForApproval::OnFailure,
            ApprovalModeCliArg::UnlessAllowListed => AskForApproval::UnlessAllowListed,
            ApprovalModeCliArg::Never => AskForApproval::Never,
        }
    }
}

#[derive(Parser, Debug)]
pub struct SandboxPermissionOption {
    /// Specify this flag multiple times to specify the full set of permissions
    /// to grant to Codex.
    ///
    /// ```shell
    /// codex -s disk-full-read-access \
    ///       -s disk-write-cwd \
    ///       -s disk-write-platform-user-temp-folder \
    ///       -s disk-write-platform-global-temp-folder
    /// ```
    ///
    /// Note disk-write-folder takes a value:
    ///
    /// ```shell
    ///     -s disk-write-folder=$HOME/.pyenv/shims
    /// ```
    ///
    /// These permissions are quite broad and should be used with caution:
    ///
    /// ```shell
    ///     -s disk-full-write-access
    ///     -s network-full-access
    /// ```
    #[arg(long = "sandbox-permission", short = 's', action = ArgAction::Append, value_parser = parse_sandbox_permission)]
    pub permissions: Option<Vec<SandboxPermission>>,
}

/// Custom value-parser so we can keep the CLI surface small *and*
/// still handle the parameterised `disk-write-folder` case.
fn parse_sandbox_permission(raw: &str) -> std::io::Result<SandboxPermission> {
    let base_path = std::env::current_dir()?;
    parse_sandbox_permission_with_base_path(raw, base_path)
}

pub(crate) fn parse_sandbox_permission_with_base_path(
    raw: &str,
    base_path: PathBuf,
) -> std::io::Result<SandboxPermission> {
    use SandboxPermission::*;

    if let Some(path) = raw.strip_prefix("disk-write-folder=") {
        return if path.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--sandbox-permission disk-write-folder=<PATH> requires a non-empty PATH",
            ))
        } else {
            use path_absolutize::*;

            let file = PathBuf::from(path);
            let absolute_path = if file.is_relative() {
                file.absolutize_from(base_path)
            } else {
                file.absolutize()
            }
            .map(|path| path.into_owned())?;
            Ok(DiskWriteFolder {
                folder: absolute_path,
            })
        };
    }

    match raw {
        "disk-full-read-access" => Ok(DiskFullReadAccess),
        "disk-write-platform-user-temp-folder" => Ok(DiskWritePlatformUserTempFolder),
        "disk-write-platform-global-temp-folder" => Ok(DiskWritePlatformGlobalTempFolder),
        "disk-write-cwd" => Ok(DiskWriteCwd),
        "disk-full-write-access" => Ok(DiskFullWriteAccess),
        "network-full-access" => Ok(NetworkFullAccess),
        _ => Err(
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "`{raw}` is not a recognised permission.\nRun with `--help` to see the accepted values."
                ),
            )
        ),
    }
}
