use crate::acl::add_allow_ace;
use crate::acl::add_deny_write_ace;
use crate::acl::allow_null_device;
use crate::allow::AllowDenyPaths;
use crate::allow::compute_allow_paths;
use crate::cap::load_or_create_cap_sids;
use crate::cap::workspace_cap_sid_for_cwd;
use crate::env::apply_no_network_to_env;
use crate::env::ensure_non_interactive_pager;
use crate::env::inherit_path_env;
use crate::env::normalize_null_device_env;
use crate::identity::SandboxCreds;
use crate::identity::require_logon_sandbox_creds;
use crate::logging::log_start;
use crate::path_normalization::canonicalize_path;
use crate::policy::SandboxPolicy;
use crate::policy::parse_policy;
use crate::sandbox_utils::ensure_codex_home_exists;
use crate::sandbox_utils::inject_git_safe_directory;
use crate::token::convert_string_sid_to_sid;
use crate::token::create_readonly_token_with_cap;
use crate::token::create_workspace_write_token_with_caps_from;
use crate::token::get_current_token_for_restriction;
use crate::token::get_logon_sid_bytes;
use crate::workspace_acl::is_command_cwd_root;
use crate::workspace_acl::protect_workspace_agents_dir;
use crate::workspace_acl::protect_workspace_codex_dir;
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;

pub(crate) struct SpawnContext {
    pub(crate) policy: SandboxPolicy,
    pub(crate) current_dir: PathBuf,
    pub(crate) sandbox_base: PathBuf,
    pub(crate) logs_base_dir: Option<PathBuf>,
    pub(crate) is_workspace_write: bool,
}

pub(crate) struct ElevatedSpawnContext {
    pub(crate) common: SpawnContext,
    pub(crate) sandbox_creds: SandboxCreds,
    pub(crate) cap_sids: Vec<String>,
}

pub(crate) struct LegacySessionSecurity {
    pub(crate) h_token: HANDLE,
    pub(crate) psid_generic: LocalSid,
    pub(crate) psid_workspace: Option<LocalSid>,
    pub(crate) cap_sid_str: String,
}

pub(crate) struct LocalSid {
    psid: *mut c_void,
}

impl LocalSid {
    pub(crate) fn from_string(sid: &str) -> Result<Self> {
        let psid = unsafe { convert_string_sid_to_sid(sid) }
            .ok_or_else(|| anyhow::anyhow!("invalid SID string: {sid}"))?;
        Ok(Self { psid })
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.psid
    }
}

impl Drop for LocalSid {
    fn drop(&mut self) {
        if !self.psid.is_null() {
            unsafe {
                LocalFree(self.psid as HLOCAL);
            }
        }
    }
}

pub(crate) fn should_apply_network_block(policy: &SandboxPolicy) -> bool {
    !policy.has_full_network_access()
}

fn prepare_spawn_context_common(
    policy_json_or_preset: &str,
    codex_home: &Path,
    cwd: &Path,
    env_map: &mut HashMap<String, String>,
    command: &[String],
    inherit_path: bool,
    add_git_safe_directory: bool,
) -> Result<SpawnContext> {
    let policy = parse_policy(policy_json_or_preset)?;
    if matches!(
        &policy,
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
    ) {
        anyhow::bail!("DangerFullAccess and ExternalSandbox are not supported for sandboxing")
    }

    normalize_null_device_env(env_map);
    ensure_non_interactive_pager(env_map);
    if inherit_path {
        inherit_path_env(env_map);
    }
    if add_git_safe_directory {
        inject_git_safe_directory(env_map, cwd);
    }

    ensure_codex_home_exists(codex_home)?;
    let sandbox_base = codex_home.join(".sandbox");
    std::fs::create_dir_all(&sandbox_base)?;
    let logs_base_dir = Some(sandbox_base.clone());
    log_start(command, logs_base_dir.as_deref());

    let is_workspace_write = matches!(&policy, SandboxPolicy::WorkspaceWrite { .. });

    Ok(SpawnContext {
        policy,
        current_dir: cwd.to_path_buf(),
        sandbox_base,
        logs_base_dir,
        is_workspace_write,
    })
}

pub(crate) fn prepare_legacy_spawn_context(
    policy_json_or_preset: &str,
    codex_home: &Path,
    cwd: &Path,
    env_map: &mut HashMap<String, String>,
    command: &[String],
    inherit_path: bool,
    add_git_safe_directory: bool,
) -> Result<SpawnContext> {
    let common = prepare_spawn_context_common(
        policy_json_or_preset,
        codex_home,
        cwd,
        env_map,
        command,
        inherit_path,
        add_git_safe_directory,
    )?;
    if should_apply_network_block(&common.policy) {
        apply_no_network_to_env(env_map)?;
    }
    Ok(common)
}

pub(crate) fn prepare_legacy_session_security(
    policy: &SandboxPolicy,
    codex_home: &Path,
    cwd: &Path,
) -> Result<LegacySessionSecurity> {
    let caps = load_or_create_cap_sids(codex_home)?;
    let (h_token, psid_generic, psid_workspace, cap_sid_str) = unsafe {
        match policy {
            SandboxPolicy::ReadOnly { .. } => {
                let psid = LocalSid::from_string(&caps.readonly)?;
                let (h_token, _psid) = create_readonly_token_with_cap(psid.as_ptr())?;
                (h_token, psid, None, caps.readonly)
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                let psid_generic = LocalSid::from_string(&caps.workspace)?;
                let workspace_sid = workspace_cap_sid_for_cwd(codex_home, cwd)?;
                let psid_workspace = LocalSid::from_string(&workspace_sid)?;
                let base = get_current_token_for_restriction()?;
                let h_token = create_workspace_write_token_with_caps_from(
                    base,
                    &[psid_generic.as_ptr(), psid_workspace.as_ptr()],
                );
                CloseHandle(base);
                let h_token = h_token?;
                (h_token, psid_generic, Some(psid_workspace), caps.workspace)
            }
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                unreachable!("dangerous policies rejected before legacy session prep")
            }
        }
    };

    Ok(LegacySessionSecurity {
        h_token,
        psid_generic,
        psid_workspace,
        cap_sid_str,
    })
}

pub(crate) fn allow_null_device_for_workspace_write(is_workspace_write: bool) {
    if !is_workspace_write {
        return;
    }

    unsafe {
        if let Ok(base) = get_current_token_for_restriction() {
            if let Ok(bytes) = get_logon_sid_bytes(base) {
                let mut tmp = bytes;
                let psid = tmp.as_mut_ptr() as *mut c_void;
                allow_null_device(psid);
            }
            CloseHandle(base);
        }
    }
}

pub(crate) fn apply_legacy_session_acl_rules(
    policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    current_dir: &Path,
    env_map: &HashMap<String, String>,
    psid_generic: &LocalSid,
    psid_workspace: Option<&LocalSid>,
    persist_aces: bool,
) -> Vec<PathBuf> {
    let AllowDenyPaths { allow, deny } =
        compute_allow_paths(policy, sandbox_policy_cwd, current_dir, env_map);
    let mut guards: Vec<PathBuf> = Vec::new();
    let canonical_cwd = canonicalize_path(current_dir);
    unsafe {
        for p in &allow {
            let psid = if matches!(policy, SandboxPolicy::WorkspaceWrite { .. })
                && is_command_cwd_root(p, &canonical_cwd)
            {
                psid_workspace.unwrap_or(psid_generic).as_ptr()
            } else {
                psid_generic.as_ptr()
            };
            if matches!(add_allow_ace(p, psid), Ok(true)) && !persist_aces {
                guards.push(p.clone());
            }
        }
        for p in &deny {
            if let Ok(added) = add_deny_write_ace(p, psid_generic.as_ptr())
                && added
                && !persist_aces
            {
                guards.push(p.clone());
            }
        }
        allow_null_device(psid_generic.as_ptr());
        if let Some(psid_workspace) = psid_workspace {
            allow_null_device(psid_workspace.as_ptr());
            if persist_aces && matches!(policy, SandboxPolicy::WorkspaceWrite { .. }) {
                let _ = protect_workspace_codex_dir(current_dir, psid_workspace.as_ptr());
                let _ = protect_workspace_agents_dir(current_dir, psid_workspace.as_ptr());
            }
        }
    }
    guards
}

pub(crate) fn prepare_elevated_spawn_context(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    cwd: &Path,
    env_map: &mut HashMap<String, String>,
    command: &[String],
) -> Result<ElevatedSpawnContext> {
    let common = prepare_spawn_context_common(
        policy_json_or_preset,
        codex_home,
        cwd,
        env_map,
        command,
        /*inherit_path*/ true,
        /*add_git_safe_directory*/ true,
    )?;

    let AllowDenyPaths { allow, deny } = compute_allow_paths(
        &common.policy,
        sandbox_policy_cwd,
        &common.current_dir,
        env_map,
    );
    let write_roots: Vec<PathBuf> = allow.into_iter().collect();
    let deny_write_paths: Vec<PathBuf> = deny.into_iter().collect();
    let write_roots_override = if common.is_workspace_write {
        Some(write_roots.as_slice())
    } else {
        None
    };
    let sandbox_creds = require_logon_sandbox_creds(
        &common.policy,
        sandbox_policy_cwd,
        cwd,
        env_map,
        codex_home,
        /*read_roots_override*/ None,
        /*read_roots_include_platform_defaults*/ false,
        write_roots_override,
        &deny_write_paths,
        /*proxy_enforced*/ false,
    )?;
    let caps = load_or_create_cap_sids(codex_home)?;
    let (psid_to_use, cap_sids) = match &common.policy {
        SandboxPolicy::ReadOnly { .. } => (
            LocalSid::from_string(&caps.readonly)?,
            vec![caps.readonly.clone()],
        ),
        SandboxPolicy::WorkspaceWrite { .. } => {
            let cap_sid = workspace_cap_sid_for_cwd(codex_home, cwd)?;
            (
                LocalSid::from_string(&caps.workspace)?,
                vec![caps.workspace.clone(), cap_sid],
            )
        }
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
            unreachable!("dangerous policies rejected before elevated session prep")
        }
    };

    unsafe {
        allow_null_device(psid_to_use.as_ptr());
    }

    Ok(ElevatedSpawnContext {
        common,
        sandbox_creds,
        cap_sids,
    })
}

#[cfg(test)]
mod tests {
    use super::SandboxPolicy;
    use super::prepare_legacy_spawn_context;
    use super::prepare_spawn_context_common;
    use super::should_apply_network_block;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn no_network_env_rewrite_applies_for_workspace_write() {
        assert!(should_apply_network_block(
            &SandboxPolicy::new_workspace_write_policy(),
        ));
    }

    #[test]
    fn no_network_env_rewrite_skips_when_network_access_is_allowed() {
        assert!(!should_apply_network_block(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        ));
    }

    #[test]
    fn legacy_spawn_env_applies_offline_network_rewrite() {
        let codex_home = TempDir::new().expect("tempdir");
        let cwd = TempDir::new().expect("tempdir");
        let mut env_map = HashMap::new();

        let _context = prepare_legacy_spawn_context(
            "workspace-write",
            codex_home.path(),
            cwd.path(),
            &mut env_map,
            &["cmd.exe".to_string()],
            /*inherit_path*/ true,
            /*add_git_safe_directory*/ false,
        )
        .expect("legacy env prep");

        assert_eq!(env_map.get("SBX_NONET_ACTIVE"), Some(&"1".to_string()));
        assert_eq!(
            env_map.get("HTTP_PROXY"),
            Some(&"http://127.0.0.1:9".to_string())
        );
    }

    #[test]
    fn common_spawn_env_keeps_network_env_unchanged() {
        let codex_home = TempDir::new().expect("tempdir");
        let cwd = TempDir::new().expect("tempdir");
        let mut env_map = HashMap::from([(
            "HTTP_PROXY".to_string(),
            "http://user.proxy:8080".to_string(),
        )]);

        let context = prepare_spawn_context_common(
            "workspace-write",
            codex_home.path(),
            cwd.path(),
            &mut env_map,
            &["cmd.exe".to_string()],
            /*inherit_path*/ true,
            /*add_git_safe_directory*/ true,
        )
        .expect("preserve existing env prep");
        assert_eq!(context.policy, SandboxPolicy::new_workspace_write_policy());

        assert_eq!(env_map.get("SBX_NONET_ACTIVE"), None);
        assert_eq!(
            env_map.get("HTTP_PROXY"),
            Some(&"http://user.proxy:8080".to_string())
        );
    }
}
