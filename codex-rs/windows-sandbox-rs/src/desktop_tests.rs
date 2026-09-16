use super::DesktopPolicy;
use super::LaunchDesktop;
use super::PRIVATE_DESKTOP_PREFIX;
use super::shared_private_desktop_for_user;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::setup::SandboxSetupRequest;
use crate::setup::SetupRootOverrides;
use crate::spawn_prep::legacy_session_capability_roots;
use crate::spawn_prep::prepare_legacy_session_security;
use crate::winutil::current_account_name;
use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use std::collections::HashMap;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn private_desktop_rejects_interactive_and_injected_names() {
    for name in [
        "Default",
        r"Winsta0\Default",
        "CodexSandboxDesktop-",
        r"CodexSandboxDesktop-abcd\Default",
        "CodexSandboxDesktop-abcd\0Default",
        "CodexSandboxDesktop-0123456789abcdef0123456789abcdef0",
    ] {
        assert!(LaunchDesktop::open_private(name).is_err(), "{name:?}");
    }
}

#[test]
fn opening_missing_private_desktop_does_not_create_or_fall_back() {
    let name = format!(
        "{PRIVATE_DESKTOP_PREFIX}{:032x}",
        SmallRng::from_entropy().r#gen::<u128>(),
    );
    assert!(LaunchDesktop::open_private(&name).is_err());
}

#[test]
fn shared_desktop_survives_launch_handles_and_concurrent_requests() -> Result<()> {
    let policy = DesktopPolicy {
        uses_write_capabilities: false,
        capability_sids: Default::default(),
        network_enabled: false,
        network_proxy_restricting_sid: None,
        read_roots: Some(Default::default()),
        write_roots: Default::default(),
        deny_read_paths: Default::default(),
        deny_write_paths: Default::default(),
    };
    let account = current_account_name()?;
    let names = std::thread::scope(|scope| {
        let workers = [(); 4].map(|_| {
            let account = &account;
            let policy = &policy;
            scope.spawn(move || {
                shared_private_desktop_for_user(account, policy, /*logs_base_dir*/ None)
            })
        });
        workers
            .into_iter()
            .map(|worker| worker.join().expect("desktop request panicked"))
            .collect::<Result<Vec<_>>>()
    })?;
    let name = names[0].clone();
    assert_eq!(names, vec![name.clone(); 4]);
    drop(LaunchDesktop::open_private(&name)?);
    assert_eq!(
        shared_private_desktop_for_user(
            &account.to_ascii_lowercase(),
            &policy,
            /*logs_base_dir*/ None
        )?,
        name,
    );
    drop(LaunchDesktop::open_private(&name)?);
    Ok(())
}

#[test]
fn shared_desktop_reuses_only_equivalent_permissions() -> Result<()> {
    let temp = TempDir::new()?;
    let workspace = temp.path().join("workspace");
    let readable = temp.path().join("readable");
    let writable = temp.path().join("writable");
    for path in [&workspace, &readable, &writable] {
        std::fs::create_dir(path)?;
    }
    let codex_home = temp.path().join("codex-home");
    let permissions = workspace_permissions(&workspace)?;
    let env = HashMap::new();
    let account = current_account_name()?;
    let sids = ["S-1-5-21-10-20-30-40".into(), "S-1-5-21-10-20-30-41".into()];
    let proxy_sid = "S-1-5-21-50-60-70-80";
    let overrides = || SetupRootOverrides {
        read_roots: Some(vec![readable.clone()]),
        write_roots: Some(vec![workspace.clone(), writable.clone()]),
        ..Default::default()
    };
    let desktop = |overrides, capability_sids: &[String], network_proxy_restricting_sid| {
        let policy = DesktopPolicy::elevated(
            SandboxSetupRequest {
                permissions: &permissions,
                command_cwd: &workspace,
                env_map: &env,
                codex_home: &codex_home,
                proxy_enforced: true,
            },
            overrides,
            capability_sids,
            Some(network_proxy_restricting_sid),
        )?;
        shared_private_desktop_for_user(&account, &policy, /*logs_base_dir*/ None)
    };
    let name = desktop(overrides(), &sids, proxy_sid)?;
    assert_eq!(
        desktop(
            SetupRootOverrides {
                read_roots: Some(vec![readable.clone(), readable.clone()]),
                write_roots: Some(vec![
                    writable.join("..").join("writable"),
                    workspace.clone(),
                    writable.clone(),
                ]),
                ..Default::default()
            },
            &[sids[1].clone(), sids[0].clone(), sids[1].clone()],
            proxy_sid,
        )?,
        name,
    );
    for (reason, changed) in [
        (
            "read roots",
            SetupRootOverrides {
                read_roots: Some(vec![workspace.clone()]),
                ..overrides()
            },
        ),
        (
            "write roots",
            SetupRootOverrides {
                write_roots: Some(vec![workspace.clone()]),
                ..overrides()
            },
        ),
        (
            "deny reads",
            SetupRootOverrides {
                deny_read_paths: Some(vec![readable.clone()]),
                ..overrides()
            },
        ),
        (
            "deny writes",
            SetupRootOverrides {
                deny_write_paths: Some(vec![writable.clone()]),
                ..overrides()
            },
        ),
    ] {
        let changed_name = desktop(changed, &sids, proxy_sid)?;
        assert_ne!(changed_name, name, "{reason}");
        drop(LaunchDesktop::open_private(&changed_name)?);
    }
    assert_ne!(desktop(overrides(), &sids[..1], proxy_sid)?, name);
    assert_ne!(desktop(overrides(), &sids, "S-1-5-21-50-60-70-81")?, name,);
    assert_eq!(desktop(overrides(), &sids, proxy_sid)?, name);
    Ok(())
}

#[test]
fn legacy_desktop_reuses_only_equivalent_permissions() -> Result<()> {
    let temp = TempDir::new()?;
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace)?;
    let codex_home = temp.path().join("codex-home");
    let permissions = workspace_permissions(&workspace)?;
    let env = HashMap::new();
    let security = prepare_legacy_session_security(
        /*uses_write_capabilities*/ true,
        &codex_home,
        &workspace,
        legacy_session_capability_roots(&permissions, &workspace, &env, &codex_home),
    )?;
    let _token = unsafe { OwnedHandle::from_raw_handle(security.h_token as *mut _) };
    let desktop = |deny_write_paths| {
        LaunchDesktop::prepare_legacy(
            /*use_private_desktop*/ true,
            &permissions,
            &workspace,
            &env,
            &security,
            deny_write_paths,
            /*logs_base_dir*/ None,
        )
    };
    let first = desktop(&[])?;
    let name = first.startup_name.clone();
    drop(first);
    assert_eq!(desktop(&[])?.startup_name, name);
    assert_ne!(
        desktop(std::slice::from_ref(&workspace))?.startup_name,
        name
    );
    assert_eq!(desktop(&[])?.startup_name, name);
    Ok(())
}

fn workspace_permissions(workspace: &Path) -> Result<ResolvedWindowsSandboxPermissions> {
    ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
        &PermissionProfile::workspace_write_with(
            &[],
            NetworkSandboxPolicy::Restricted,
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        ),
        &[AbsolutePathBuf::from_absolute_path(workspace)?],
    )
}
