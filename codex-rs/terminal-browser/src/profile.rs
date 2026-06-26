use std::fmt::Write;
#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_utils_absolute_path::AbsolutePathBuf;
use sha2::Digest;
use sha2::Sha256;

use crate::sandbox::BrowserLaunchContext;

const MAX_PROFILE_NAME_BYTES: usize = 64;
const MAX_LISTED_PROFILES: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BrowserProfileName(String);

impl BrowserProfileName {
    pub(crate) fn parse(name: &str) -> Result<Self> {
        let mut bytes = name.bytes();
        let first = bytes.next().context("profile name must not be empty")?;
        let valid_first = first.is_ascii_alphanumeric();
        let valid_rest =
            bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        anyhow::ensure!(
            name.len() <= MAX_PROFILE_NAME_BYTES && valid_first && valid_rest,
            "profile names must start with a letter or number and contain at most 64 ASCII letters, numbers, dots, underscores, or dashes"
        );
        Ok(Self(name.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BrowserProfileStore {
    root: AbsolutePathBuf,
}

pub(crate) struct BrowserProfileListing {
    pub(crate) profiles: Vec<String>,
    pub(crate) total: usize,
    pub(crate) truncated: bool,
}

pub(crate) struct BrowserProfileLock {
    #[cfg(unix)]
    file: File,
}

impl BrowserProfileStore {
    pub(crate) fn from_context(context: &BrowserLaunchContext) -> Result<Option<Self>> {
        let (Some(codex_home), Some(workspace_root)) =
            (&context.codex_home, &context.workspace_root)
        else {
            return Ok(None);
        };
        let mut workspace_hash = String::with_capacity(/*capacity*/ 16);
        let digest = Sha256::digest(workspace_root.as_os_str().as_encoded_bytes());
        for byte in &digest[..8] {
            write!(&mut workspace_hash, "{byte:02x}").context("format workspace profile hash")?;
        }
        let root = codex_home
            .join("browser-profiles")
            .join("terminal-browser")
            .join(workspace_hash);
        Ok(Some(Self { root }))
    }

    pub(crate) fn list(&self) -> Result<BrowserProfileListing> {
        let entries = match std::fs::read_dir(self.root.as_path()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BrowserProfileListing {
                    profiles: Vec::new(),
                    total: 0,
                    truncated: false,
                });
            }
            Err(error) => return Err(error).context("list terminal-browser profiles"),
        };
        let mut profiles = Vec::new();
        let mut total = 0;
        for entry in entries {
            let entry = entry.context("read terminal-browser profile entry")?;
            let file_type = entry
                .file_type()
                .context("read terminal-browser profile type")?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if file_type.is_dir() && BrowserProfileName::parse(&name).is_ok() {
                total += 1;
                if profiles.len() < MAX_LISTED_PROFILES {
                    profiles.push(name);
                }
            }
        }
        profiles.sort_unstable();
        Ok(BrowserProfileListing {
            profiles,
            total,
            truncated: total > MAX_LISTED_PROFILES,
        })
    }

    pub(crate) fn create(&self, name: &BrowserProfileName) -> Result<AbsolutePathBuf> {
        create_private_directory(self.root.as_path())?;
        let path = self.root.join(name.as_str());
        anyhow::ensure!(
            !std::fs::symlink_metadata(path.as_path())
                .is_ok_and(|metadata| metadata.file_type().is_symlink()),
            "profile path must not be a symbolic link"
        );
        create_private_directory(path.as_path())?;
        Ok(path)
    }

    pub(crate) fn existing_path(&self, name: &BrowserProfileName) -> Result<AbsolutePathBuf> {
        let path = self.root.join(name.as_str());
        let metadata = std::fs::symlink_metadata(path.as_path())
            .with_context(|| format!("browser profile `{}` does not exist", name.as_str()))?;
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "browser profile path is not a safe directory"
        );
        Ok(path)
    }

    pub(crate) fn lock_existing(
        &self,
        name: &BrowserProfileName,
    ) -> Result<(AbsolutePathBuf, BrowserProfileLock)> {
        self.existing_path(name)?;
        let lock = self.acquire_lock(name)?;
        // A cooperating process cannot delete the profile after the lock is acquired, but
        // rechecking keeps us fail-closed if an older Codex process raced with this one.
        let path = self.existing_path(name)?;
        Ok((path, lock))
    }

    pub(crate) fn forget(&self, name: &BrowserProfileName) -> Result<()> {
        let _lock = self.acquire_lock(name)?;
        let path = self.existing_path(name)?;
        std::fs::remove_dir_all(path.as_path())
            .with_context(|| format!("delete browser profile `{}`", name.as_str()))
    }

    #[cfg(unix)]
    fn acquire_lock(&self, name: &BrowserProfileName) -> Result<BrowserProfileLock> {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::OpenOptionsExt;

        create_private_directory(self.root.as_path())?;
        let lock_path = self.root.join(format!(".{}.lock", name.as_str()));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(/*mode*/ 0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(lock_path.as_path())
            .with_context(|| format!("open browser profile lock for `{}`", name.as_str()))?;
        anyhow::ensure!(
            file.metadata()
                .context("read browser profile lock metadata")?
                .is_file(),
            "browser profile lock path is not a regular file"
        );
        // SAFETY: `file` owns a valid descriptor for the duration of this call and remains owned
        // by `BrowserProfileLock` for as long as the advisory lock must be held.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(BrowserProfileLock { file });
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::WouldBlock {
            anyhow::bail!("browser profile `{}` is already in use", name.as_str());
        }
        Err(error).with_context(|| format!("lock browser profile `{}`", name.as_str()))
    }

    #[cfg(not(unix))]
    fn acquire_lock(&self, _name: &BrowserProfileName) -> Result<BrowserProfileLock> {
        anyhow::bail!("named terminal-browser profiles are unsupported on this platform")
    }
}

#[cfg(unix)]
impl Drop for BrowserProfileLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;

        // SAFETY: `self.file` remains open for the duration of the call. Closing the file would
        // also release the lock, but an explicit unlock makes the lifetime boundary unambiguous.
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

fn create_private_directory(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("create browser profile directory {}", path.display()))?;
    set_private_permissions(path)
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(/*mode*/ 0o700))
        .with_context(|| format!("restrict browser profile directory {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
