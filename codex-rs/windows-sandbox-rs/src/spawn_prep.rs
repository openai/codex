use crate::acl::add_allow_ace;
use crate::acl::add_deny_write_ace;
use crate::acl::allow_null_device;
use crate::allow::AllowDenyPaths;
use crate::allow::compute_allow_paths;
use crate::cap::load_or_create_cap_sids;
use crate::cap::workspace_write_cap_sid_for_root;
use crate::cap::workspace_write_root_contains_path;
use crate::cap::workspace_write_root_specificity;
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
use crate::setup::effective_write_roots_for_setup;
use crate::token::LocalSid;
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
    pub(crate) readonly_sid: Option<LocalSid>,
    pub(crate) write_root_sids: Vec<RootCapabilitySid>,
}

pub(crate) struct RootCapabilitySid {
    pub(crate) root: PathBuf,
    pub(crate) sid: LocalSid,
    pub(crate) sid_str: String,
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
    allow_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<LegacySessionSecurity> {
    let caps = load_or_create_cap_sids(codex_home)?;
    let (h_token, readonly_sid, write_root_sids) = unsafe {
        match policy {
            SandboxPolicy::ReadOnly { .. } => {
                let psid = LocalSid::from_string(&caps.readonly)?;
                let (h_token, _psid) = create_readonly_token_with_cap(psid.as_ptr())?;
                (h_token, Some(psid), Vec::new())
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                let write_root_sids = root_capability_sids(codex_home, cwd, allow_paths)?;
                if write_root_sids.is_empty() {
                    anyhow::bail!("workspace-write sandbox has no writable root capability SIDs");
                }
                let base = get_current_token_for_restriction()?;
                let cap_ptrs: Vec<*mut c_void> = write_root_sids
                    .iter()
                    .map(|root| root.sid.as_ptr())
                    .collect();
                let h_token =
                    create_workspace_write_token_with_caps_from(base, cap_ptrs.as_slice());
                CloseHandle(base);
                let h_token = h_token?;
                (h_token, None, write_root_sids)
            }
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                unreachable!("dangerous policies rejected before legacy session prep")
            }
        }
    };

    Ok(LegacySessionSecurity {
        h_token,
        readonly_sid,
        write_root_sids,
    })
}

pub(crate) fn root_capability_sids(
    codex_home: &Path,
    cwd: &Path,
    allow_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<RootCapabilitySid>> {
    let mut roots: Vec<PathBuf> = allow_paths.into_iter().collect();
    roots.sort_by_key(|root| canonicalize_path(root.as_path()));
    roots.dedup_by(|a, b| canonicalize_path(a.as_path()) == canonicalize_path(b.as_path()));

    let mut out = Vec::with_capacity(roots.len());
    for root in roots {
        let sid_str = workspace_write_cap_sid_for_root(codex_home, cwd, &root)?;
        let sid = LocalSid::from_string(&sid_str)?;
        out.push(RootCapabilitySid { root, sid, sid_str });
    }
    Ok(out)
}

fn matching_root_capability<'a>(
    path: &Path,
    root_sids: &'a [RootCapabilitySid],
) -> Option<&'a RootCapabilitySid> {
    root_sids
        .iter()
        .filter(|root_sid| workspace_write_root_contains_path(&root_sid.root, path))
        .max_by_key(|root_sid| workspace_write_root_specificity(&root_sid.root))
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
    additional_deny_write_paths: &[PathBuf],
    readonly_sid: Option<&LocalSid>,
    write_root_sids: &[RootCapabilitySid],
    persist_aces: bool,
) -> Vec<(PathBuf, String)> {
    let AllowDenyPaths { allow, mut deny } =
        compute_allow_paths(policy, sandbox_policy_cwd, current_dir, env_map);
    for path in additional_deny_write_paths {
        if path.exists() {
            deny.insert(path.clone());
        }
    }
    let mut guards: Vec<(PathBuf, String)> = Vec::new();
    unsafe {
        for p in &allow {
            let Some(root_sid) = matching_root_capability(p, write_root_sids) else {
                continue;
            };
            if matches!(add_allow_ace(p, root_sid.sid.as_ptr()), Ok(true)) && !persist_aces {
                guards.push((p.clone(), root_sid.sid_str.clone()));
            }
        }
        for p in &deny {
            let mut matched_any_root = false;
            for root_sid in write_root_sids {
                if !workspace_write_root_contains_path(&root_sid.root, p) {
                    continue;
                }
                matched_any_root = true;
                if let Ok(added) = add_deny_write_ace(p, root_sid.sid.as_ptr())
                    && added
                    && !persist_aces
                {
                    guards.push((p.clone(), root_sid.sid_str.clone()));
                }
            }
            if !matched_any_root {
                for root_sid in write_root_sids {
                    if let Ok(added) = add_deny_write_ace(p, root_sid.sid.as_ptr())
                        && added
                        && !persist_aces
                    {
                        guards.push((p.clone(), root_sid.sid_str.clone()));
                    }
                }
            }
        }
        for root_sid in write_root_sids {
            allow_null_device(root_sid.sid.as_ptr());
        }
        if let Some(readonly_sid) = readonly_sid {
            allow_null_device(readonly_sid.as_ptr());
        }
        if persist_aces
            && matches!(policy, SandboxPolicy::WorkspaceWrite { .. })
            && let Some(workspace_sid) = matching_root_capability(current_dir, write_root_sids)
        {
            let canonical_cwd = canonicalize_path(current_dir);
            if is_command_cwd_root(&workspace_sid.root, &canonical_cwd) {
                let _ = protect_workspace_codex_dir(current_dir, workspace_sid.sid.as_ptr());
                let _ = protect_workspace_agents_dir(current_dir, workspace_sid.sid.as_ptr());
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
    let effective_write_roots = if common.is_workspace_write {
        effective_write_roots_for_setup(
            &common.policy,
            sandbox_policy_cwd,
            &common.current_dir,
            env_map,
            codex_home,
            Some(write_roots.as_slice()),
        )
    } else {
        Vec::new()
    };
    let write_roots_override = if common.is_workspace_write {
        Some(effective_write_roots.as_slice())
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
            let cap_sids = root_capability_sids(codex_home, cwd, effective_write_roots)?
                .into_iter()
                .map(|root_sid| root_sid.sid_str)
                .collect::<Vec<_>>();
            if cap_sids.is_empty() {
                anyhow::bail!("workspace-write sandbox has no writable root capability SIDs");
            }
            (LocalSid::from_string(&cap_sids[0])?, cap_sids)
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
    use super::root_capability_sids;
    use super::should_apply_network_block;
    use crate::cap::load_or_create_cap_sids;
    use crate::cap::workspace_write_cap_sid_for_root;
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

    #[test]
    fn root_capability_sids_only_include_active_roots() {
        let temp = TempDir::new().expect("tempdir");
        let codex_home = temp.path().join("codex-home");
        let workspace = temp.path().join("workspace");
        let active_root = temp.path().join("active-root");
        let stale_root = temp.path().join("stale-root");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        std::fs::create_dir_all(&active_root).expect("create active root");
        std::fs::create_dir_all(&stale_root).expect("create stale root");

        let stale_sid = workspace_write_cap_sid_for_root(&codex_home, &workspace, &stale_root)
            .expect("stale sid");
        let active_sid = workspace_write_cap_sid_for_root(&codex_home, &workspace, &active_root)
            .expect("active sid");
        let workspace_sid = workspace_write_cap_sid_for_root(&codex_home, &workspace, &workspace)
            .expect("workspace sid");
        let caps = load_or_create_cap_sids(&codex_home).expect("load caps");

        let sid_strs = root_capability_sids(
            &codex_home,
            &workspace,
            vec![workspace.clone(), active_root.clone()],
        )
        .expect("root capabilities")
        .into_iter()
        .map(|root_sid| root_sid.sid_str)
        .collect::<Vec<_>>();

        assert_eq!(sid_strs.len(), 2);
        assert!(sid_strs.contains(&workspace_sid));
        assert!(sid_strs.contains(&active_sid));
        assert!(!sid_strs.contains(&stale_sid));
        assert!(!sid_strs.contains(&caps.workspace));
    }
}
