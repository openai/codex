//! Borrow the command and policy inputs observed by the sandbox integrity checks.

use crate::SandboxExecRequest;
use crate::SandboxType;
use codex_protocol::models::PermissionProfile;
use codex_protocol::sandbox::SandboxOverride;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::ffi::OsStr;

/// The actual launcher and authorization recorded for one execution.
pub struct SandboxIntegrityRequest<'a> {
    pub sandbox: SandboxType,
    pub sandbox_override: SandboxOverride,
    pub permission_profile: &'a PermissionProfile,
    pub sandbox_policy_cwd: &'a PathUri,
    pub cwd: &'a PathUri,
    pub launcher: Option<&'a OsStr>,
    pub env: &'a HashMap<String, String>,
}

impl<'a> From<&'a SandboxExecRequest> for SandboxIntegrityRequest<'a> {
    fn from(request: &'a SandboxExecRequest) -> Self {
        Self {
            sandbox: request.sandbox,
            sandbox_override: request.sandbox_override,
            permission_profile: &request.permission_profile,
            sandbox_policy_cwd: &request.sandbox_policy_cwd,
            cwd: &request.cwd,
            launcher: request.program_and_args().map(|(program, _args)| program),
            env: &request.env,
        }
    }
}
