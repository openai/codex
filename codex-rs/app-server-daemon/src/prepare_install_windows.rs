//! Publishes and selects Windows managed releases despite transient filesystem contention.

use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

const PUBLISH_RELEASE_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const PUBLISH_RELEASE_RETRY_LIMIT: usize = 100;

pub(super) async fn publish_release(stage: &Path, release: &Path) -> Result<()> {
    let mut retries = 0;
    loop {
        match std::fs::rename(stage, release) {
            Ok(()) => return Ok(()),
            // Executable scanners can briefly hold newly staged files without delete sharing.
            Err(error)
                if retries < PUBLISH_RELEASE_RETRY_LIMIT
                    && (error.kind() == std::io::ErrorKind::PermissionDenied
                        || error.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32)) =>
            {
                retries += 1;
                tokio::time::sleep(PUBLISH_RELEASE_RETRY_INTERVAL).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to publish managed daemon release from {} to {}",
                        stage.display(),
                        release.display()
                    )
                });
            }
        }
    }
}

pub(super) fn select_release(root: &Path, release: &Path) -> Result<()> {
    select_release_with(root, release, retarget_junction)
}

fn select_release_with(
    root: &Path,
    release: &Path,
    retarget: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let release = release.canonicalize()?;
    let current = root.join("current");
    if current.symlink_metadata().is_err() {
        let temporary = tempfile::TempDir::new_in(root)?;
        let junction = temporary.path().join("current");
        std::fs::create_dir(&junction)?;
        return match retarget(&junction, &release) {
            Ok(()) => Ok(std::fs::rename(junction, current)?),
            Err(error) if is_permission_denied(&error) => install_junction(root, &release)
                .with_context(|| format!("failed to create managed daemon junction after native creation was denied: {error:#}")),
            Err(error) => Err(error),
        };
    }
    validate_selection(root)?;
    match retarget(&current, &release) {
        Ok(()) => Ok(()),
        Err(error) if is_permission_denied(&error) => {
            install_junction(root, &release).with_context(|| format!("failed to replace managed daemon junction after retargeting was denied: {error:#}"))
        }
        Err(error) => Err(error),
    }
}

fn is_permission_denied(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied)
}

fn install_junction(root: &Path, release: &Path) -> Result<()> {
    let temporary = tempfile::TempDir::new_in(root)?;
    let junction = temporary.path().join("current");
    let current = root.join("current");
    let replacing = current.symlink_metadata().is_ok();
    if replacing {
        validate_selection(root)?;
    }
    // Some Windows policies deny in-process reparse-point mutation while allowing the system
    // junction creator. Pass paths through the environment so cmd metacharacters stay quoted.
    let system_root = std::env::var_os("SystemRoot").context("SystemRoot is not set")?;
    let command_shell = Path::new(&system_root).join("System32").join("cmd.exe");
    anyhow::ensure!(
        command_shell.is_absolute(),
        "SystemRoot must be an absolute path"
    );
    let output = Command::new(command_shell)
        .env("CODEX_DAEMON_JUNCTION_LINK", &junction)
        .env("CODEX_DAEMON_JUNCTION_TARGET", release)
        .args(["/d", "/s", "/e:on", "/v:off", "/c"])
        .raw_arg(r#""mklink /J "%CODEX_DAEMON_JUNCTION_LINK%" "%CODEX_DAEMON_JUNCTION_TARGET%"""#)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("failed to run cmd.exe to create managed daemon junction")?;
    anyhow::ensure!(
        output.status.success(),
        "cmd.exe could not create managed daemon junction (status {}): stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    if !replacing {
        std::fs::rename(junction, current)?;
        return Ok(());
    }

    let previous = temporary.path().join("previous");
    std::fs::rename(&current, &previous)?;
    if let Err(error) = std::fs::rename(&junction, &current) {
        if let Err(restore_error) = std::fs::rename(&previous, &current) {
            let preserved = temporary.keep();
            anyhow::bail!(
                "failed to replace managed daemon junction: {error}; also failed to restore the previous junction from {}: {restore_error}",
                preserved.display()
            );
        }
        return Err(error).context("failed to replace managed daemon junction");
    }
    if let Err(error) = std::fs::remove_dir(previous) {
        tracing::warn!(
            %error,
            "failed to remove previous managed daemon junction"
        );
    }
    Ok(())
}

fn retarget_junction(current: &Path, release: &Path) -> Result<()> {
    // Use the same mount-point reparse operation as the standalone installer.
    // Retargeting in place keeps current available to concurrent readers.
    const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00a4;
    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa000_0003;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    let release: Vec<u16> = release.as_os_str().encode_wide().collect();
    let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    let release = release.strip_prefix(prefix.as_slice()).unwrap_or(&release);
    let substitute: Vec<u8> = r"\??\"
        .encode_utf16()
        .chain(release.iter().copied())
        .flat_map(u16::to_le_bytes)
        .collect();
    u16::try_from(substitute.len() + 20).context("managed release path is too long")?;
    let length = substitute.len() as u16;
    // REPARSE_DATA_BUFFER: an 8-byte header, then four u16 byte offsets/lengths
    // for substitute and print names. The UTF-16 path buffer starts at byte 16;
    // both names have a trailing NUL, and the print name is empty.
    let mut data = vec![0; substitute.len() + 20];
    data[0..4].copy_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
    data[4..6].copy_from_slice(&(length + 12).to_le_bytes());
    data[10..12].copy_from_slice(&length.to_le_bytes());
    data[12..14].copy_from_slice(&(length + 2).to_le_bytes());
    data[16..16 + substitute.len()].copy_from_slice(&substitute);
    let handle = std::fs::OpenOptions::new()
        .access_mode(GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(current)?;
    let mut returned = 0;
    if unsafe {
        DeviceIoControl(
            handle.as_raw_handle(),
            FSCTL_SET_REPARSE_POINT,
            data.as_ptr().cast(),
            data.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .context("failed to retarget managed daemon junction");
    }
    Ok(())
}

pub(super) fn validate_selection(root: &Path) -> Result<()> {
    let current = root.join("current");
    if matches!(current.symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(());
    }
    anyhow::ensure!(
        current.canonicalize()?.parent() == Some(root.join("releases").canonicalize()?.as_path()),
        "refusing to replace a daemon selection outside its releases directory"
    );
    Ok(())
}

#[cfg(test)]
#[path = "prepare_install_windows_tests.rs"]
mod tests;
