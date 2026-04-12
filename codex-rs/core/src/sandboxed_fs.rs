use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallRawOutput;
use crate::exec::execute_exec_request_raw_output;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::merge_permission_profiles;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxablePreference;
use codex_fs_ops::CODEX_CORE_FS_OPS_ARG1;
use codex_fs_ops::READ_FILE_OPERATION_ARG2;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Reads the contents of the specified file subject to the sandbox constraints
/// imposed by the provided session and turn context.
///
/// Note that this function is comparable to `cat FILE`, though unlike `cat
/// FILE`, this function verifies that FILE is a regular file before reading,
/// which means that if you pass `/dev/zero` as the path, it will error (rather
/// than hang forever).
pub(crate) async fn read_file(
    path: &AbsolutePathBuf,
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
) -> Result<Vec<u8>, SandboxedFsError> {
    let output = perform_operation(
        SandboxedFsOperation::Read { path: path.clone() },
        session,
        turn,
    )
    .await?;
    Ok(output.stdout.text)
}

/// Operations supported by the [CODEX_CORE_FS_OPS_ARG1] sandbox helper.
enum SandboxedFsOperation {
    Read { path: AbsolutePathBuf },
}

async fn perform_operation(
    operation: SandboxedFsOperation,
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
) -> Result<ExecToolCallRawOutput, SandboxedFsError> {
    let exe = std::env::current_exe().map_err(|error| SandboxedFsError::ResolveExe {
        message: error.to_string(),
    })?;
    let additional_permissions = effective_granted_permissions(session).await;
    let sandbox_manager = crate::sandboxing::SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox: sandbox_manager.select_initial(
            &turn.file_system_sandbox_policy,
            turn.network_sandbox_policy,
            SandboxablePreference::Auto,
            turn.windows_sandbox_level,
            /*has_managed_network_requirements*/ false,
        ),
        policy: &turn.sandbox_policy,
        file_system_policy: &turn.file_system_sandbox_policy,
        network_policy: turn.network_sandbox_policy,
        enforce_managed_network: false,
        manager: &sandbox_manager,
        sandbox_cwd: &turn.cwd,
        codex_linux_sandbox_exe: turn.codex_linux_sandbox_exe.as_ref(),
        use_legacy_landlock: turn.features.use_legacy_landlock(),
        windows_sandbox_level: turn.windows_sandbox_level,
        windows_sandbox_private_desktop: turn.config.permissions.windows_sandbox_private_desktop,
    };

    let args = match operation {
        SandboxedFsOperation::Read { ref path } => vec![
            CODEX_CORE_FS_OPS_ARG1.to_string(),
            READ_FILE_OPERATION_ARG2.to_string(),
            path.to_string_lossy().to_string(),
        ],
    };

    // `FullBuffer` reads ignore exec expiration, but `ExecRequest` still requires
    // an `expiration` field, so keep a placeholder timeout here until that API
    // changes.
    let ignored_expiration = Duration::from_secs(30);
    let exec_request = attempt
        .env_for(
            CommandSpec {
                program: exe.to_string_lossy().to_string(),
                args,
                cwd: turn.cwd.clone(),
                env: HashMap::new(),
                expiration: ExecExpiration::Timeout(ignored_expiration),
                capture_policy: ExecCapturePolicy::FullBuffer,
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions,
                justification: None,
            },
            /*network*/ None,
        )
        .map_err(|error| SandboxedFsError::ProcessFailed {
            exit_code: -1,
            message: error.to_string(),
        })?;

    let effective_policy = exec_request.sandbox_policy.clone();
    let output = execute_exec_request_raw_output(
        exec_request,
        &effective_policy,
        /*stdout_stream*/ None,
        /*after_spawn*/ None,
    )
    .await
    .map_err(|error| SandboxedFsError::ProcessFailed {
        exit_code: 1,
        message: error.to_string(),
    })?;
    if output.exit_code == 0 {
        Ok(output)
    } else {
        Err(parse_helper_failure(
            output.exit_code,
            &output.stderr.text,
            &output.stdout.text,
        ))
    }
}

async fn effective_granted_permissions(session: &Session) -> Option<PermissionProfile> {
    let granted_session_permissions = session.granted_session_permissions().await;
    let granted_turn_permissions = session.granted_turn_permissions().await;
    merge_permission_profiles(
        granted_session_permissions.as_ref(),
        granted_turn_permissions.as_ref(),
    )
}

fn parse_helper_failure(exit_code: i32, stderr: &[u8], stdout: &[u8]) -> SandboxedFsError {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    let message = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else if !stdout.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        "no error details emitted".to_string()
    };

    SandboxedFsError::ProcessFailed { exit_code, message }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SandboxedFsError {
    #[error("failed to determine codex executable: {message}")]
    ResolveExe { message: String },
    #[error("sandboxed fs helper exited with code {exit_code}: {message}")]
    ProcessFailed { exit_code: i32, message: String },
}
