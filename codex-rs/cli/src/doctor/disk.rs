use std::io;
use std::path::Path;

use codex_core::config::Config;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorIssue;
use super::find_codex_home;

const GIB: u64 = 1024 * 1024 * 1024;
const FAIL_THRESHOLD: u64 = GIB;
const WARNING_THRESHOLD: u64 = 5 * GIB;

pub(super) fn check(config: Option<&Config>, cwd: &Path) -> DoctorCheck {
    let home = config
        .map(|config| config.codex_home.as_path().to_path_buf())
        .or_else(|| find_codex_home().ok().map(Into::into))
        .or_else(|| std::env::var_os("CODEX_HOME").map(Into::into));

    check_with_paths(home.as_deref(), cwd, available_space)
}

fn check_with_paths(
    home: Option<&Path>,
    cwd: &Path,
    measure: impl Fn(&Path) -> io::Result<u64>,
) -> DoctorCheck {
    let home = home.and_then(|path| std::path::absolute(path).ok());
    let mut check = DoctorCheck::new(
        "system.disk",
        "disk",
        CheckStatus::Ok,
        "sufficient free disk space",
    )
    .detail(format!(
        "warning threshold: {}",
        format_capacity(WARNING_THRESHOLD)
    ))
    .detail(format!(
        "failure threshold: {}",
        format_capacity(FAIL_THRESHOLD)
    ));
    let mut lowest = None;

    for (label, path) in [("CODEX_HOME", home.as_deref()), ("worktree", Some(cwd))] {
        let field = format!("{label} available");
        let available = path
            .and_then(|path| path.ancestors().find(|ancestor| ancestor.is_dir()))
            .map(&measure)
            .unwrap_or_else(|| Err(io::ErrorKind::NotFound.into()));
        match available {
            Ok(available) => {
                let measured = format_capacity(available);
                check = check.detail(format!("{field}: {measured}"));
                lowest = Some(lowest.map_or(available, |lowest: u64| lowest.min(available)));

                let status = match available {
                    0..FAIL_THRESHOLD => CheckStatus::Fail,
                    FAIL_THRESHOLD..WARNING_THRESHOLD => CheckStatus::Warning,
                    WARNING_THRESHOLD.. => CheckStatus::Ok,
                };
                if status != CheckStatus::Ok {
                    check.status = check.status.max(status);
                    check = check.issue(
                        DoctorIssue::new(status, format!("{label} has insufficient disk space"))
                            .measured(measured)
                            .expected(format!("at least {}", format_capacity(WARNING_THRESHOLD)))
                            .remedy("Free disk space or move the worktree to a larger volume.")
                            .field(field),
                    );
                }
            }
            Err(error) => {
                check.status = check.status.max(CheckStatus::Warning);
                check = check
                    .detail(format!("{field}: unavailable ({:?})", error.kind()))
                    .issue(
                        DoctorIssue::new(
                            CheckStatus::Warning,
                            format!("disk space for {label} could not be checked"),
                        )
                        .measured(format!("{:?}", error.kind()))
                        .expected("readable filesystem capacity")
                        .remedy("Check filesystem access and available disk space.")
                        .field(field),
                    );
            }
        }
    }

    check.summary = match (check.status, lowest) {
        (CheckStatus::Fail, Some(available)) => {
            format!("critically low disk space ({})", format_capacity(available))
        }
        (CheckStatus::Warning, Some(available)) if available < WARNING_THRESHOLD => {
            format!("low disk space ({})", format_capacity(available))
        }
        (CheckStatus::Warning, _) => "disk capacity could not be fully verified".to_string(),
        (CheckStatus::Ok, Some(available)) => {
            format!(
                "sufficient free disk space ({})",
                format_capacity(available)
            )
        }
        (CheckStatus::Ok | CheckStatus::Fail, None) => {
            "disk capacity could not be verified".to_string()
        }
    };

    check
}

fn format_capacity(bytes: u64) -> String {
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024 * 1024) as f64)
    }
}

#[cfg(unix)]
fn available_space(path: &Path) -> io::Result<u64> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let mut stats = MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stats = unsafe { stats.assume_init() };
    if stats.f_files > 0 && stats.f_favail == 0 {
        return Ok(0);
    }
    u64::try_from(u128::from(stats.f_bavail) * u128::from(stats.f_frsize))
        .map_err(|_| io::Error::other("available disk space exceeds u64"))
}

#[cfg(windows)]
fn available_space(path: &Path) -> io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut path = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if path.last() != Some(&u16::from(b'\\')) {
        path.push(u16::from(b'\\'));
    }
    path.push(0);
    let mut available = 0;
    if unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    Ok(available)
}

#[cfg(not(any(unix, windows)))]
fn available_space(_path: &Path) -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "filesystem capacity is unavailable on this platform",
    ))
}

#[cfg(test)]
#[path = "disk_tests.rs"]
mod tests;
