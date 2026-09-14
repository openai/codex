//! Prepare exact managed logons before native cleanup; authorize removal only after success.
//! The child owns loaded profiles and waits for both COMMIT and service-process EOF.

use std::io::Read;
use std::io::Write;
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::AsHandle;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use base64::Engine;
use windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS;
use windows_sys::Win32::Foundation::DuplicateHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Pipes::PeekNamedPipe;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading as threading;

use crate::installation_record::InstallationRecord;
use crate::package_lifecycle::with_owner_impersonation;

pub(crate) struct PreparedRemoval {
    child: Child,
    tokens: Vec<OwnedHandle>,
}

impl PreparedRemoval {
    pub(crate) fn tokens(&self) -> Vec<BorrowedHandle<'_>> {
        self.tokens.iter().map(AsHandle::as_handle).collect()
    }

    /// Called only after the native cleanup guard has successfully finished.
    pub(crate) fn commit(mut self) -> Result<()> {
        self.child
            .stdin
            .as_mut()
            .context("cleanup input closed")?
            .write_all(b"COMMIT\n")
            .context("commit Windows package cleanup")?;
        // The only pipe writer remains open until this own-process service exits.
        // Dropping an uncommitted preparation instead sends EOF, which only aborts.
        std::mem::forget(self.child);
        Ok(())
    }
}

// The cleanup entry checked the service family; prepare_cleanup validated this record
// under the same setup lock. Fresh logons below still require exact account-SID checks.
pub(crate) fn prepare(owner_token: HANDLE, record: &InstallationRecord) -> Result<PreparedRemoval> {
    ensure!(
        record.runtime()?.retiring.is_some(),
        "runtime is not retiring"
    );
    let mut tokens = Vec::new();
    let mut targets = Vec::new();
    for account in &record.runtime()?.accounts {
        if codex_windows_sandbox::local_user_flags(account.account.username())?.is_none() {
            ensure!(
                super::registered_packages(&account.user_sid, &record.runtime()?.package_family)?
                    .is_empty(),
                "runtime account is missing while its package remains registered"
            );
            continue;
        }
        let token = with_owner_impersonation(owner_token, || {
            let mut pins = Vec::new();
            crate::ipc::pin_existing_ancestors(
                &codex_windows_sandbox::sandbox_secrets_dir(&record.codex_home),
                &mut pins,
            )?;
            codex_windows_sandbox::logon_existing_sandbox_account(
                &record.codex_home,
                account.account,
            )
        })?;
        super::validate_target(
            token.as_raw_handle() as _,
            account.account,
            Some(&account.user_sid),
        )?;
        targets.push(serde_json::json!({
            "username": account.account.username(),
            "sid": account.user_sid,
        }));
        tokens.push(token);
    }
    let mut system = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    ensure!(
        length > 0 && length < system.len(),
        "Windows system directory is unavailable"
    );
    let system = PathBuf::from(String::from_utf16(&system[..length])?);
    let executable = system.join(r"WindowsPowerShell\v1.0\powershell.exe");
    let metadata = executable
        .symlink_metadata()
        .context("inspect Windows PowerShell")?;
    ensure!(
        metadata.is_file()
            && metadata.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                == 0,
        "invalid Windows PowerShell executable"
    );
    let bytes = include_str!("removal.ps1")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    ensure!(
        executable.as_os_str().len() + encoded.len() + 64 < 32767,
        "cleanup command exceeds the Windows command-line limit"
    );
    let mut child = Command::new(executable)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .current_dir(system)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(threading::CREATE_NO_WINDOW)
        .spawn()
        .context("start Windows package cleanup")?;
    // Do not relax process handle inheritance. Only these validated token handles
    // are duplicated into the exact child process, with inheritance disabled.
    for (target, token) in targets.iter_mut().zip(&tokens) {
        let mut remote = 0;
        let duplicated = unsafe {
            DuplicateHandle(
                threading::GetCurrentProcess(),
                token.as_raw_handle() as _,
                child.as_raw_handle() as _,
                &mut remote,
                /*dwdesiredaccess*/ 0,
                /*binherithandle*/ 0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        ensure!(
            duplicated != 0,
            "duplicate managed cleanup token: {}",
            std::io::Error::last_os_error()
        );
        target["handle"] = serde_json::json!(remote as usize);
    }
    let plan = serde_json::json!({
        "key": crate::installation_record::CORE_INSTALLATION_KEY,
        "legacy_key": crate::installation_record::INSTALLATION_KEY,
        "value": crate::installation_record::INSTALLATION_VALUE,
        "record": record,
        "targets": targets,
        "service_name": codex_windows_sandbox::windows_sandbox_service_name()?,
    });
    let input = child.stdin.as_mut().context("capture cleanup input")?;
    serde_json::to_writer(&mut *input, &plan)?;
    input.write_all(b"\n")?;
    let output = child.stdout.as_mut().context("capture cleanup readiness")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut ready = Vec::new();
    while !ready.ends_with(b"\n") {
        ensure!(
            Instant::now() < deadline,
            "timed out preparing Windows package cleanup"
        );
        let mut available = 0;
        let result = unsafe {
            PeekNamedPipe(
                output.as_raw_handle() as _,
                std::ptr::null_mut(),
                /*nbuffersize*/ 0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        ensure!(
            result != 0,
            "cleanup exited before readiness: {}",
            std::io::Error::last_os_error()
        );
        if available == 0 {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let mut byte = [0];
        output.read_exact(&mut byte)?;
        ready.push(byte[0]);
        ensure!(ready.len() <= 16, "invalid cleanup readiness response");
    }
    ensure!(
        ready == b"READY\r\n" || ready == b"READY\n",
        "cleanup profiles were not prepared"
    );
    Ok(PreparedRemoval { child, tokens })
}

#[cfg(test)]
#[path = "removal_tests.rs"]
mod tests;
