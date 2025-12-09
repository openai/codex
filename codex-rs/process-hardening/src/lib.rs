#[cfg(unix)]
use std::ffi::OsString;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

/// This is designed to be called pre-main() (using `#[ctor::ctor]`) to perform
/// various process hardening steps, such as
/// - disabling core dumps
/// - disabling ptrace attach on Linux and macOS.
/// - removing dangerous environment variables such as LD_PRELOAD and DYLD_*
pub fn pre_main_hardening() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pre_main_hardening_linux();

    #[cfg(target_os = "macos")]
    pre_main_hardening_macos();

    // On FreeBSD and OpenBSD, apply similar hardening to Linux/macOS:
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    pre_main_hardening_bsd();

    #[cfg(windows)]
    pre_main_hardening_windows();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const PRCTL_FAILED_EXIT_CODE: i32 = 5;

#[cfg(target_os = "macos")]
const PTRACE_DENY_ATTACH_FAILED_EXIT_CODE: i32 = 6;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
const SET_RLIMIT_CORE_FAILED_EXIT_CODE: i32 = 7;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn pre_main_hardening_linux() {
    // Disable ptrace attach / mark process non-dumpable.
    let ret_code = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
    if ret_code != 0 {
        eprintln!(
            "ERROR: prctl(PR_SET_DUMPABLE, 0) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(PRCTL_FAILED_EXIT_CODE);
    }

    // For "defense in depth," set the core file size limit to 0.
    set_core_file_size_limit_to_zero();

    // Official Codex releases are MUSL-linked, which means that variables such
    // as LD_PRELOAD are ignored anyway, but just to be sure, clear them here.
    let ld_keys = env_keys_with_prefix(std::env::vars_os(), b"LD_");

    for key in ld_keys {
        unsafe {
            std::env::remove_var(key);
        }
    }
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
pub(crate) fn pre_main_hardening_bsd() {
    // FreeBSD/OpenBSD: set RLIMIT_CORE to 0 and clear LD_* env vars
    set_core_file_size_limit_to_zero();

    let ld_keys = env_keys_with_prefix(std::env::vars_os(), b"LD_");
    for key in ld_keys {
        unsafe {
            std::env::remove_var(key);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn pre_main_hardening_macos() {
    // Prevent debuggers from attaching to this process.
    let ret_code = unsafe { libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0) };
    if ret_code == -1 {
        eprintln!(
            "ERROR: ptrace(PT_DENY_ATTACH) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(PTRACE_DENY_ATTACH_FAILED_EXIT_CODE);
    }

    // Set the core file size limit to 0 to prevent core dumps.
    set_core_file_size_limit_to_zero();

    // Remove all DYLD_ environment variables, which can be used to subvert
    // library loading.
    let dyld_keys = env_keys_with_prefix(std::env::vars_os(), b"DYLD_");

    for key in dyld_keys {
        unsafe {
            std::env::remove_var(key);
        }
    }
}

#[cfg(unix)]
fn set_core_file_size_limit_to_zero() {
    let rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    let ret_code = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &rlim) };
    if ret_code != 0 {
        eprintln!(
            "ERROR: setrlimit(RLIMIT_CORE) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(SET_RLIMIT_CORE_FAILED_EXIT_CODE);
    }
}

#[cfg(windows)]
pub(crate) fn pre_main_hardening_windows() {
    // TODO(mbolin): Perform the appropriate configuration for Windows.
}

#[cfg(unix)]
fn env_keys_with_prefix<I>(vars: I, prefix: &[u8]) -> Vec<OsString>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    vars.into_iter()
        .filter_map(|(key, _)| {
            key.as_os_str()
                .as_bytes()
                .starts_with(prefix)
                .then_some(key)
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn env_keys_with_prefix_handles_non_utf8_entries() {
        // RÖDBURK
        let non_utf8_key1 = OsStr::from_bytes(b"R\xD6DBURK").to_os_string();
        assert!(non_utf8_key1.clone().into_string().is_err());
        let non_utf8_key2 = OsString::from_vec(vec![b'L', b'D', b'_', 0xF0]);
        assert!(non_utf8_key2.clone().into_string().is_err());

        let non_utf8_value = OsString::from_vec(vec![0xF0, 0x9F, 0x92, 0xA9]);

        let keys = env_keys_with_prefix(
            vec![
                (non_utf8_key1, non_utf8_value.clone()),
                (non_utf8_key2.clone(), non_utf8_value),
            ],
            b"LD_",
        );
        assert_eq!(
            keys,
            vec![non_utf8_key2],
            "non-UTF-8 env entries with LD_ prefix should be retained"
        );
    }

    #[test]
    fn env_keys_with_prefix_filters_only_matching_keys() {
        let ld_test_var = OsStr::from_bytes(b"LD_TEST");
        let vars = vec![
            (OsString::from("PATH"), OsString::from("/usr/bin")),
            (ld_test_var.to_os_string(), OsString::from("1")),
            (OsString::from("DYLD_FOO"), OsString::from("bar")),
        ];

        let keys = env_keys_with_prefix(vars, b"LD_");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_os_str(), ld_test_var);
    }
}
