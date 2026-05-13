use crate::acl::add_allow_ace;
use crate::acl::add_deny_write_ace;
use crate::acl::allow_null_device;
use crate::allow::AllowDenyPaths;
use crate::allow::compute_allow_paths;
use crate::cap::load_or_create_cap_sids;
use crate::cap::workspace_write_cap_sid_for_root;
use crate::cap::workspace_write_root_contains_path;
use crate::cap::workspace_write_root_overlaps_path;
use crate::cap::workspace_write_root_specificity;
use crate::deny_read_acl::apply_deny_read_acls;
use crate::deny_read_state::sync_persistent_deny_read_acls;
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
use anyhow::Context;
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
    pub(crate) readonly_sid_str: Option<String>,
    pub(crate) write_root_sids: Vec<RootCapabilitySid>,
}

pub(crate) struct RootCapabilitySid {
    pub(crate) root: PathBuf,
    pub(crate) sid: LocalSid,
    pub(crate) sid_str: String,
}

pub(crate) struct LegacyAclSids<'a> {
    pub(crate) readonly_sid: Option<&'a LocalSid>,
    pub(crate) readonly_sid_str: Option<&'a str>,
    pub(crate) write_root_sids: &'a [RootCapabilitySid],
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
    capability_roots: impl IntoIterator<Item = PathBuf>,
) -> Result<LegacySessionSecurity> {
    let caps = load_or_create_cap_sids(codex_home)?;
    let (h_token, readonly_sid, readonly_sid_str, write_root_sids) = unsafe {
        match policy {
            SandboxPolicy::ReadOnly { .. } => {
                let psid = LocalSid::from_string(&caps.readonly)?;
                let (h_token, _psid) = create_readonly_token_with_cap(psid.as_ptr())?;
                (h_token, Some(psid), Some(caps.readonly), Vec::new())
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                let write_root_sids = root_capability_sids(codex_home, cwd, capability_roots)?;
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
                (h_token, None, None, write_root_sids)
            }
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                unreachable!("dangerous policies rejected before legacy session prep")
            }
        }
    };

    Ok(LegacySessionSecurity {
        h_token,
        readonly_sid,
        readonly_sid_str,
        write_root_sids,
    })
}

pub(crate) fn legacy_session_capability_roots(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    current_dir: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> Vec<PathBuf> {
    let allow_paths = compute_allow_paths(policy, policy_cwd, current_dir, env_map)
        .allow
        .into_iter()
        .collect::<Vec<_>>();
    if matches!(policy, SandboxPolicy::WorkspaceWrite { .. }) {
        effective_write_roots_for_setup(
            policy,
            policy_cwd,
            current_dir,
            env_map,
            codex_home,
            Some(allow_paths.as_slice()),
        )
    } else {
        allow_paths
    }
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

fn deny_root_capabilities_for_path<'a>(
    path: &Path,
    root_sids: &'a [RootCapabilitySid],
) -> Vec<&'a RootCapabilitySid> {
    let matching_root_sids = root_sids
        .iter()
        .filter(|root_sid| workspace_write_root_overlaps_path(&root_sid.root, path))
        .collect::<Vec<_>>();
    if matching_root_sids.is_empty() {
        root_sids.iter().collect()
    } else {
        matching_root_sids
    }
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_legacy_session_acl_rules(
    policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    current_dir: &Path,
    env_map: &HashMap<String, String>,
    additional_deny_read_paths: &[PathBuf],
    additional_deny_write_paths: &[PathBuf],
    acl_sids: LegacyAclSids<'_>,
    persist_aces: bool,
) -> Result<Vec<(PathBuf, String)>> {
    let AllowDenyPaths { allow, mut deny } =
        compute_allow_paths(policy, sandbox_policy_cwd, current_dir, env_map);
    let mut guards: Vec<(PathBuf, String)> = Vec::new();
    unsafe {
        for path in additional_deny_write_paths {
            // Explicit carveouts must exist before the command starts so the
            // sandbox cannot create them under a writable parent first.
            if !path.exists() {
                std::fs::create_dir_all(path)
                    .with_context(|| format!("create deny-write path {}", path.display()))?;
            }
            deny.insert(path.clone());
        }
        if let Some(readonly_sid) = acl_sids.readonly_sid {
            for p in &allow {
                if matches!(add_allow_ace(p, readonly_sid.as_ptr()), Ok(true))
                    && !persist_aces
                    && let Some(readonly_sid_str) = acl_sids.readonly_sid_str
                {
                    guards.push((p.clone(), readonly_sid_str.to_string()));
                }
            }
        } else {
            for p in &allow {
                let Some(root_sid) = matching_root_capability(p, acl_sids.write_root_sids) else {
                    continue;
                };
                if matches!(add_allow_ace(p, root_sid.sid.as_ptr()), Ok(true)) && !persist_aces {
                    guards.push((p.clone(), root_sid.sid_str.clone()));
                }
            }
        }
        for p in &deny {
            for root_sid in deny_root_capabilities_for_path(p, acl_sids.write_root_sids) {
                if let Ok(added) = add_deny_write_ace(p, root_sid.sid.as_ptr())
                    && added
                    && !persist_aces
                {
                    guards.push((p.clone(), root_sid.sid_str.clone()));
                }
            }
        }
        if !additional_deny_read_paths.is_empty() {
            if let Some(readonly_sid) = acl_sids.readonly_sid {
                let Some(readonly_sid_str) = acl_sids.readonly_sid_str else {
                    anyhow::bail!("readonly capability SID string missing");
                };
                let applied_deny_read_paths = if persist_aces {
                    sync_persistent_deny_read_acls(
                        codex_home,
                        readonly_sid_str,
                        additional_deny_read_paths,
                        readonly_sid.as_ptr(),
                    )?
                } else {
                    apply_deny_read_acls(additional_deny_read_paths, readonly_sid.as_ptr())?
                };
                if !persist_aces {
                    guards.extend(
                        applied_deny_read_paths
                            .into_iter()
                            .map(|path| (path, readonly_sid_str.to_string())),
                    );
                }
            } else {
                for root_sid in acl_sids.write_root_sids {
                    let applied_deny_read_paths = if persist_aces {
                        sync_persistent_deny_read_acls(
                            codex_home,
                            &root_sid.sid_str,
                            additional_deny_read_paths,
                            root_sid.sid.as_ptr(),
                        )?
                    } else {
                        apply_deny_read_acls(additional_deny_read_paths, root_sid.sid.as_ptr())?
                    };
                    if !persist_aces {
                        guards.extend(
                            applied_deny_read_paths
                                .into_iter()
                                .map(|path| (path, root_sid.sid_str.clone())),
                        );
                    }
                }
            }
        }
        for root_sid in acl_sids.write_root_sids {
            allow_null_device(root_sid.sid.as_ptr());
        }
        if let Some(readonly_sid) = acl_sids.readonly_sid {
            allow_null_device(readonly_sid.as_ptr());
        }
        if persist_aces
            && matches!(policy, SandboxPolicy::WorkspaceWrite { .. })
            && let Some(workspace_sid) =
                matching_root_capability(current_dir, acl_sids.write_root_sids)
        {
            let canonical_cwd = canonicalize_path(current_dir);
            if is_command_cwd_root(&workspace_sid.root, &canonical_cwd) {
                let _ = protect_workspace_codex_dir(current_dir, workspace_sid.sid.as_ptr());
                let _ = protect_workspace_agents_dir(current_dir, workspace_sid.sid.as_ptr());
            }
        }
    }
    Ok(guards)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_elevated_spawn_context(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    cwd: &Path,
    env_map: &mut HashMap<String, String>,
    command: &[String],
    read_roots_override: Option<&[PathBuf]>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[PathBuf],
    deny_write_paths_override: &[PathBuf],
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
    let computed_write_roots_override = if common.is_workspace_write {
        Some(write_roots.as_slice())
    } else {
        None
    };
    let write_roots_for_setup = write_roots_override.or(computed_write_roots_override);
    let effective_write_roots = if common.is_workspace_write {
        effective_write_roots_for_setup(
            &common.policy,
            sandbox_policy_cwd,
            &common.current_dir,
            env_map,
            codex_home,
            write_roots_for_setup,
        )
    } else {
        Vec::new()
    };
    let setup_write_roots_override = if common.is_workspace_write {
        Some(effective_write_roots.as_slice())
    } else {
        write_roots_override
    };
    let sandbox_creds = require_logon_sandbox_creds(
        &common.policy,
        sandbox_policy_cwd,
        cwd,
        env_map,
        codex_home,
        read_roots_override,
        read_roots_include_platform_defaults,
        setup_write_roots_override,
        deny_read_paths_override,
        if deny_write_paths_override.is_empty() {
            &deny_write_paths
        } else {
            deny_write_paths_override
        },
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
    use super::deny_root_capabilities_for_path;
    use super::legacy_session_capability_roots;
    use super::prepare_legacy_spawn_context;
    use super::prepare_spawn_context_common;
    use super::root_capability_sids;
    use super::should_apply_network_block;
    use crate::cap::load_or_create_cap_sids;
    use crate::cap::workspace_write_cap_sid_for_root;
    use codex_utils_absolute_path::AbsolutePathBuf;
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
            vec![workspace.clone(), active_root],
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

    #[test]
    fn legacy_deny_path_includes_nested_active_root_sid() {
        let temp = TempDir::new().expect("tempdir");
        let codex_home = temp.path().join("codex-home");
        let workspace = temp.path().join("workspace");
        let protected_dir = workspace.join(".codex");
        let nested_root = protected_dir.join("nested-root");
        let unrelated_root = temp.path().join("unrelated-root");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        std::fs::create_dir_all(&nested_root).expect("create nested root");
        std::fs::create_dir_all(&unrelated_root).expect("create unrelated root");

        let workspace_sid = workspace_write_cap_sid_for_root(&codex_home, &workspace, &workspace)
            .expect("workspace sid");
        let nested_sid = workspace_write_cap_sid_for_root(&codex_home, &workspace, &nested_root)
            .expect("nested sid");
        let unrelated_sid =
            workspace_write_cap_sid_for_root(&codex_home, &workspace, &unrelated_root)
                .expect("unrelated sid");
        let root_sids = root_capability_sids(
            &codex_home,
            &workspace,
            vec![workspace.clone(), nested_root, unrelated_root],
        )
        .expect("root capabilities");

        let deny_sid_strs = deny_root_capabilities_for_path(&protected_dir, &root_sids)
            .into_iter()
            .map(|root_sid| root_sid.sid_str.clone())
            .collect::<Vec<_>>();

        assert_eq!(deny_sid_strs, vec![workspace_sid, nested_sid]);
        assert!(!deny_sid_strs.contains(&unrelated_sid));
    }

    #[test]
    fn legacy_capability_roots_use_effective_write_roots() {
        let temp = TempDir::new().expect("tempdir");
        let codex_home = temp.path().join("codex-home");
        let workspace = temp.path().join("workspace");
        let active_root = temp.path().join("active-root");
        let sandbox_root = codex_home.join(".sandbox");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        std::fs::create_dir_all(&active_root).expect("create active root");
        std::fs::create_dir_all(&sandbox_root).expect("create sandbox root");

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![
                AbsolutePathBuf::try_from(active_root.as_path()).expect("active root"),
                AbsolutePathBuf::try_from(codex_home.as_path()).expect("codex home"),
                AbsolutePathBuf::try_from(sandbox_root.as_path()).expect("sandbox root"),
            ],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let roots = legacy_session_capability_roots(
            &policy,
            &workspace,
            &workspace,
            &HashMap::new(),
            &codex_home,
        );

        assert!(roots.contains(&dunce::canonicalize(&workspace).expect("workspace")));
        assert!(roots.contains(&dunce::canonicalize(&active_root).expect("active root")));
        assert!(!roots.contains(&dunce::canonicalize(&codex_home).expect("codex home")));
        assert!(!roots.contains(&dunce::canonicalize(&sandbox_root).expect("sandbox root")));
    }
}
