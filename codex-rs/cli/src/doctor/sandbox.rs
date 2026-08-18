#[cfg(target_os = "windows")]
use std::fs;
#[cfg(target_os = "windows")]
use std::io;

use codex_arg0::Arg0DispatchPaths;
use codex_core::config::Config;
#[cfg(target_os = "windows")]
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
#[cfg(target_os = "windows")]
use codex_protocol::config_types::WindowsSandboxLevel;
#[cfg(target_os = "windows")]
use codex_sandboxing::windows_sandbox_uses_elevated_backend;
#[cfg(target_os = "windows")]
use codex_windows_sandbox::SetupErrorCode;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::NERR_Success as NERR_SUCCESS;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::NERR_UserNotFound as NERR_USER_NOT_FOUND;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::NetApiBufferFree;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserGetInfo;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::UF_ACCOUNTDISABLE;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::UF_LOCKOUT;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::UF_PASSWORD_EXPIRED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::NetManagement::USER_INFO_23;

use super::CheckStatus;
use super::DoctorCheck;
#[cfg(target_os = "windows")]
use super::DoctorIssue;
use super::push_path_detail;

#[cfg(target_os = "windows")]
const WINDOWS_SETUP_REMEDIATION: &str = "run codex sandbox setup --elevated --user <end-user> --codex-home <authoritative-home> from an elevated shell";

pub(super) fn sandbox_check(config: &Config, arg0_paths: &Arg0DispatchPaths) -> DoctorCheck {
    let mut details = Vec::new();
    details.push(format!(
        "approval policy: {:?}",
        config.permissions.approval_policy.value()
    ));
    let file_system_sandbox = config.permissions.file_system_sandbox_policy();
    details.push(format!("filesystem sandbox: {}", file_system_sandbox.kind));
    details.push(format!(
        "network sandbox: {}",
        config.permissions.network_sandbox_policy()
    ));
    push_path_detail(
        &mut details,
        "codex-linux-sandbox helper",
        arg0_paths.codex_linux_sandbox_exe.as_deref(),
    );
    push_path_detail(
        &mut details,
        "execve wrapper helper",
        arg0_paths.main_execve_wrapper_exe.as_deref(),
    );

    let mut status = CheckStatus::Ok;
    let mut summary = "sandbox configuration is readable".to_string();
    if let Some(helper) = arg0_paths.codex_linux_sandbox_exe.as_deref()
        && !helper.exists()
    {
        status = CheckStatus::Warning;
        summary = "Linux sandbox helper path does not exist".to_string();
    }

    let check = DoctorCheck::new("sandbox.helpers", "sandbox", status, summary).details(details);

    #[cfg(target_os = "windows")]
    let mut check = check;

    #[cfg(target_os = "windows")]
    {
        let configured_backend = WindowsSandboxLevel::from_config(config);
        let elevated = configured_backend != WindowsSandboxLevel::Disabled
            && windows_sandbox_uses_elevated_backend(configured_backend);
        let backend = if elevated {
            WindowsSandboxLevel::Elevated
        } else {
            configured_backend
        };
        let denied = !config
            .permissions
            .file_system_sandbox_policy()
            .has_full_disk_read_access();

        check.details.push(format!("sandbox backend: {backend}"));
        check
            .details
            .push(format!("denied-read restrictions: {denied}"));

        if denied && !elevated {
            check.issues.push(
                DoctorIssue::new(
                    CheckStatus::Fail,
                    "managed denied-read requirements need the elevated Windows sandbox backend",
                )
                .measured(format!("{backend} backend"))
                .expected("elevated backend")
                .remedy("enable the elevated Windows sandbox backend permitted by managed policy")
                .field("sandbox backend")
                .field("denied-read restrictions"),
            );
        }

        if elevated {
            let home = config.codex_home.as_path();
            let path = codex_windows_sandbox::setup_error_path(home);
            let report = match fs::metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Ok(metadata) if metadata.is_file() && metadata.len() <= 64 * 1024 => {
                    codex_windows_sandbox::read_setup_error_report(home)
                        .map_err(|error| error.to_string())
                }
                Ok(metadata) if !metadata.is_file() => Err("not a regular file".to_string()),
                Ok(_) => Err("larger than 64 KiB".to_string()),
                Err(error) => Err(error.to_string()),
            };
            let report = match report {
                Ok(report) => report,
                Err(error) => {
                    check
                        .details
                        .push("setup failure report: unreadable".to_string());
                    check.issues.push(
                        DoctorIssue::new(
                            CheckStatus::Warning,
                            "Windows sandbox setup failure report could not be read",
                        )
                        .measured(error)
                        .expected("readable setup failure report no larger than 64 KiB")
                        .remedy(WINDOWS_SETUP_REMEDIATION)
                        .field("setup failure report"),
                    );
                    None
                }
            };

            if report.is_none() && codex_windows_sandbox::sandbox_setup_is_complete(home) {
                check
                    .details
                    .push("sandbox provisioning: complete".to_string());

                for username in [
                    codex_windows_sandbox::OFFLINE_USERNAME,
                    codex_windows_sandbox::ONLINE_USERNAME,
                ] {
                    let username_wide = username.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                    let mut buffer = std::ptr::null_mut();
                    let status = unsafe {
                        NetUserGetInfo(
                            std::ptr::null(),
                            username_wide.as_ptr(),
                            /*level*/ 23,
                            &mut buffer,
                        )
                    };
                    let flags = if status == NERR_SUCCESS && !buffer.is_null() {
                        Some(unsafe { (*(buffer as *const USER_INFO_23)).usri23_flags })
                    } else {
                        None
                    };
                    if !buffer.is_null() {
                        unsafe { NetApiBufferFree(buffer.cast()) };
                    }

                    let (severity, cause, remediation) = match (status, flags) {
                        (NERR_USER_NOT_FOUND, _) => (
                            CheckStatus::Fail,
                            "a Windows sandbox account is missing",
                            WINDOWS_SETUP_REMEDIATION,
                        ),
                        (NERR_SUCCESS, Some(flags)) if flags & UF_LOCKOUT != 0 => (
                            CheckStatus::Fail,
                            "a Windows sandbox account is locked",
                            "stop sandbox retries and ask IT to unlock the account",
                        ),
                        (NERR_SUCCESS, Some(flags)) if flags & UF_ACCOUNTDISABLE != 0 => (
                            CheckStatus::Fail,
                            "a Windows sandbox account is disabled",
                            "ask IT to enable the sandbox account before rerunning setup",
                        ),
                        (NERR_SUCCESS, Some(flags)) if flags & UF_PASSWORD_EXPIRED != 0 => (
                            CheckStatus::Fail,
                            "a Windows sandbox account password has expired",
                            WINDOWS_SETUP_REMEDIATION,
                        ),
                        (NERR_SUCCESS, Some(_)) => continue,
                        (ERROR_ACCESS_DENIED, _) => (
                            CheckStatus::Warning,
                            "Windows sandbox account status could not be inspected",
                            "ask IT to inspect access to the local sandbox accounts",
                        ),
                        _ => (
                            CheckStatus::Warning,
                            "Windows sandbox account status could not be inspected",
                            "ask IT to inspect the local sandbox accounts",
                        ),
                    };
                    check.details.push(format!("sandbox account: {username}"));
                    if !matches!(
                        status,
                        NERR_SUCCESS | NERR_USER_NOT_FOUND | ERROR_ACCESS_DENIED
                    ) {
                        check
                            .details
                            .push(format!("account query status: {status}"));
                    }
                    check.issues.push(
                        DoctorIssue::new(severity, cause)
                            .measured(username)
                            .expected("enabled, unlocked local sandbox account")
                            .remedy(remediation)
                            .field("sandbox account"),
                    );
                }
            } else {
                check.details.push(if report.is_some() {
                    "sandbox provisioning: failed".to_string()
                } else {
                    "sandbox provisioning: incomplete".to_string()
                });
                if let Some(report) = report {
                    let remediation = match report.code {
                        SetupErrorCode::OrchestratorElevationCheckFailed
                        | SetupErrorCode::OrchestratorElevationRequired
                        | SetupErrorCode::OrchestratorHelperLaunchCanceled => {
                            "use an elevated shell or your organization's approved elevation workflow"
                        }
                        SetupErrorCode::OrchestratorHelperLaunchFailed
                        | SetupErrorCode::OrchestratorHelperExitNonzero
                        | SetupErrorCode::OrchestratorHelperReportReadFailed
                        | SetupErrorCode::OrchestratorHelperIncomplete
                        | SetupErrorCode::HelperReadAclHelperSpawnFailed => {
                            "repair the installed Codex helpers or ask IT to allow their execution"
                        }
                        SetupErrorCode::HelperUserProvisionFailed
                        | SetupErrorCode::HelperUsersGroupCreateFailed
                        | SetupErrorCode::HelperUserCreateOrUpdateFailed
                        | SetupErrorCode::HelperSidResolveFailed
                        | SetupErrorCode::HelperCapabilitySidFailed => {
                            "ask IT to allow the managed local sandbox accounts and group"
                        }
                        SetupErrorCode::OrchestratorSandboxDirCreateFailed
                        | SetupErrorCode::HelperSandboxDirCreateFailed
                        | SetupErrorCode::HelperLogFailed
                        | SetupErrorCode::HelperDpapiProtectFailed
                        | SetupErrorCode::HelperUsersFileWriteFailed
                        | SetupErrorCode::HelperSetupMarkerWriteFailed
                        | SetupErrorCode::HelperSandboxLockFailed => {
                            "rerun elevated setup for the authoritative Codex home or ask IT"
                        }
                        SetupErrorCode::HelperFirewallComInitFailed
                        | SetupErrorCode::HelperFirewallPolicyAccessFailed
                        | SetupErrorCode::HelperFirewallPolicyIneffective
                        | SetupErrorCode::HelperFirewallRuleCreateOrAddFailed
                        | SetupErrorCode::HelperFirewallRuleVerifyFailed => {
                            "ask IT to allow Codex sandbox rules in managed Windows Firewall policy"
                        }
                        SetupErrorCode::OrchestratorPayloadSerializeFailed
                        | SetupErrorCode::HelperRequestArgsFailed
                        | SetupErrorCode::HelperUnknownError => {
                            "repair or reinstall the Codex CLI from an approved distribution"
                        }
                    };
                    check
                        .details
                        .push(format!("error code: {}", report.code.as_str()));
                    check.issues.push(
                        DoctorIssue::new(
                            CheckStatus::Fail,
                            "elevated Windows sandbox provisioning recorded a structured failure",
                        )
                        .measured(report.code.as_str())
                        .remedy(remediation)
                        .field("error code"),
                    );
                } else {
                    check.issues.push(
                        DoctorIssue::new(
                            CheckStatus::Warning,
                            "elevated Windows sandbox provisioning is incomplete or outdated",
                        )
                        .remedy(WINDOWS_SETUP_REMEDIATION)
                        .field("sandbox provisioning"),
                    );
                }
            }
        }

        if let Some(severity) = check.issues.iter().map(|issue| issue.severity).max() {
            check.status = check.status.max(severity);
            check.summary = if check.issues.len() == 1 {
                check.issues[0].cause.clone()
            } else {
                "Windows sandbox has multiple configuration or provisioning issues".to_string()
            };
            if check.remediation.is_none() {
                check.remediation = check.issues.iter().find_map(|issue| issue.remedy.clone());
            }
        }
    }

    check
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
