use std::path::Path;
use std::path::PathBuf;

use crate::env;

pub fn normalize_for_path_comparison(path: &Path) -> std::io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    Ok(normalize_for_wsl(canonical))
}

fn normalize_for_wsl(path: PathBuf) -> PathBuf {
    if !env::is_wsl() {
        return path;
    }

    if !is_wsl_case_insensitive_path(&path) {
        return path;
    }

    lower_ascii_path(path)
}

fn is_wsl_case_insensitive_path(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        use std::path::Component;

        let mut components = path.components();
        let Some(Component::RootDir) = components.next() else {
            return false;
        };
        let Some(Component::Normal(mnt)) = components.next() else {
            return false;
        };
        if !ascii_eq_ignore_case(mnt.as_bytes(), b"mnt") {
            return false;
        }
        let Some(Component::Normal(drive)) = components.next() else {
            return false;
        };
        let drive_bytes = drive.as_bytes();
        drive_bytes.len() == 1 && drive_bytes[0].is_ascii_alphabetic()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
}

#[cfg(target_os = "linux")]
fn ascii_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(lhs, rhs)| lhs.to_ascii_lowercase() == *rhs)
}

#[cfg(target_os = "linux")]
fn lower_ascii_path(path: PathBuf) -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::ffi::OsStringExt;

    // WSL mounts Windows drives under /mnt/<drive>, which are case-insensitive.
    let bytes = path.as_os_str().as_bytes();
    let mut lowered = Vec::with_capacity(bytes.len());
    for byte in bytes {
        lowered.push(byte.to_ascii_lowercase());
    }
    PathBuf::from(OsString::from_vec(lowered))
}

#[cfg(not(target_os = "linux"))]
fn lower_ascii_path(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    mod wsl {
        use super::normalize_for_wsl;
        use pretty_assertions::assert_eq;
        use std::ffi::OsStr;
        use std::path::PathBuf;
        use std::sync::Mutex;
        use std::sync::OnceLock;

        struct EnvVarGuard {
            key: &'static str,
            original: Option<std::ffi::OsString>,
        }

        impl EnvVarGuard {
            fn set(key: &'static str, value: &OsStr) -> Self {
                let original = std::env::var_os(key);
                unsafe {
                    std::env::set_var(key, value);
                }
                Self { key, original }
            }
        }

        impl Drop for EnvVarGuard {
            fn drop(&mut self) {
                unsafe {
                    match &self.original {
                        Some(value) => std::env::set_var(self.key, value),
                        None => std::env::remove_var(self.key),
                    }
                }
            }
        }

        static WSL_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

        fn lock_wsl_env() -> std::sync::MutexGuard<'static, ()> {
            WSL_ENV_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("wsl env lock poisoned")
        }

        #[test]
        fn wsl_mnt_drive_paths_lowercase() {
            let _lock = lock_wsl_env();
            let _guard = EnvVarGuard::set("WSL_DISTRO_NAME", OsStr::new("Ubuntu"));

            let normalized = normalize_for_wsl(PathBuf::from("/mnt/C/Users/Dev"));

            assert_eq!(normalized, PathBuf::from("/mnt/c/users/dev"));
        }

        #[test]
        fn wsl_non_drive_paths_unchanged() {
            let _lock = lock_wsl_env();
            let _guard = EnvVarGuard::set("WSL_DISTRO_NAME", OsStr::new("Ubuntu"));

            let path = PathBuf::from("/mnt/cc/Users/Dev");
            let normalized = normalize_for_wsl(path.clone());

            assert_eq!(normalized, path);
        }

        #[test]
        fn wsl_non_mnt_paths_unchanged() {
            let _lock = lock_wsl_env();
            let _guard = EnvVarGuard::set("WSL_DISTRO_NAME", OsStr::new("Ubuntu"));

            let path = PathBuf::from("/home/Dev");
            let normalized = normalize_for_wsl(path.clone());

            assert_eq!(normalized, path);
        }
    }
}
