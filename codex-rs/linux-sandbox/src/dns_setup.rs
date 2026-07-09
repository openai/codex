use crate::bwrap::BwrapArgs;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use rustix::thread::CapabilitySet;
use rustix::thread::CapabilitySets;
use rustix::thread::capabilities;
use rustix::thread::capability_is_in_ambient_set;
use rustix::thread::capability_is_in_bounding_set;
use rustix::thread::clear_ambient_capability_set;
use rustix::thread::remove_capability_from_bounding_set;
use rustix::thread::set_capabilities;
use std::fs::File;
use std::io;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::path::PathBuf;

const RESOLV_CONF_PATH: &str = "/etc/resolv.conf";
const LOADER_ENV_KEYS: &[&str] = &["LD_AUDIT", "LD_LIBRARY_PATH", "LD_PRELOAD"];

pub(crate) type LoaderEnvironment = Vec<(String, String)>;

#[derive(Debug)]
pub(crate) struct ResolvConfMount {
    pub(crate) file: File,
    pub(crate) path: PathBuf,
}

pub(crate) fn create_resolv_conf_file() -> io::Result<File> {
    let fd = unsafe { libc::memfd_create(c"codex-resolv-conf".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(b"nameserver 127.0.0.1\n")?;
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}

pub(crate) fn resolv_conf_mount_path(
    policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> io::Result<PathBuf> {
    let logical_path = Path::new(RESOLV_CONF_PATH);
    let target_path = logical_path.canonicalize()?;
    ensure_resolver_paths_allowed(policy, cwd, logical_path, &target_path)?;
    Ok(target_path)
}

fn ensure_resolver_paths_allowed(
    policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    logical_path: &Path,
    target_path: &Path,
) -> io::Result<()> {
    let deny_matcher = ReadDenyMatcher::new(policy, cwd);
    if [logical_path, target_path].into_iter().any(|path| {
        !policy.can_read_path_with_cwd(path, cwd)
            || deny_matcher
                .as_ref()
                .is_some_and(|deny| deny.is_read_denied(path))
    }) {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    Ok(())
}

pub(crate) fn capture_loader_environment() -> LoaderEnvironment {
    LOADER_ENV_KEYS
        .iter()
        .filter_map(|&key| {
            std::env::var_os(key).map(|value| {
                (
                    key.to_string(),
                    value
                        .into_string()
                        .unwrap_or_else(|_| panic!("{key} must contain valid UTF-8")),
                )
            })
        })
        .collect()
}

pub(crate) fn restore_loader_environment(environment: LoaderEnvironment) {
    for (key, value) in environment {
        // SAFETY: the setup process is single-threaded and has dropped all capabilities.
        unsafe { std::env::set_var(key, value) };
    }
}

pub(crate) fn append_bwrap_args(bwrap_args: &mut BwrapArgs, mount: ResolvConfMount) {
    let separator = bwrap_args
        .args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or_else(|| panic!("bubblewrap argv is missing command separator '--'"));
    let Some(parent) = mount.path.parent() else {
        panic!("resolver configuration path has no parent");
    };
    let fd = mount.file.as_raw_fd().to_string();
    bwrap_args.args.splice(
        separator..separator,
        LOADER_ENV_KEYS
            .iter()
            .flat_map(|key| ["--unsetenv".to_string(), (*key).to_string()])
            .chain([
                "--cap-drop".to_string(),
                "ALL".to_string(),
                "--cap-add".to_string(),
                "CAP_NET_BIND_SERVICE".to_string(),
                "--cap-add".to_string(),
                "CAP_SETPCAP".to_string(),
                "--dir".to_string(),
                parent.to_string_lossy().into_owned(),
                "--perms".to_string(),
                "444".to_string(),
                "--ro-bind-data".to_string(),
                fd,
                mount.path.to_string_lossy().into_owned(),
            ]),
    );
    bwrap_args.preserved_files.push(mount.file);
}

pub(crate) fn drop_and_verify_capabilities() -> io::Result<()> {
    let ignore_unknown = |result: rustix::io::Result<()>| match result {
        Ok(()) | Err(rustix::io::Errno::INVAL) => Ok(()),
        Err(err) => Err(io::Error::from_raw_os_error(err.raw_os_error())),
    };
    ignore_unknown(clear_ambient_capability_set())?;
    for bit in 0..u64::BITS {
        let capability = CapabilitySet::from_bits_retain(1_u64 << bit);
        match capability_is_in_ambient_set(capability) {
            Ok(false) | Err(rustix::io::Errno::INVAL) => {}
            Ok(true) => return Err(io::Error::other("capability remained in ambient set")),
            Err(err) => return Err(io::Error::from_raw_os_error(err.raw_os_error())),
        }
        ignore_unknown(remove_capability_from_bounding_set(capability))?;
        match capability_is_in_bounding_set(capability) {
            Ok(false) | Err(rustix::io::Errno::INVAL) => {}
            Ok(true) => return Err(io::Error::other("capability remained in bounding set")),
            Err(err) => return Err(io::Error::from_raw_os_error(err.raw_os_error())),
        }
    }
    let empty = CapabilitySets {
        effective: CapabilitySet::empty(),
        permitted: CapabilitySet::empty(),
        inheritable: CapabilitySet::empty(),
    };
    set_capabilities(None, empty).map_err(io::Error::from)?;
    if capabilities(None).map_err(io::Error::from)? != empty {
        return Err(io::Error::other("capabilities remained after DNS setup"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "dns_setup_tests.rs"]
mod tests;
