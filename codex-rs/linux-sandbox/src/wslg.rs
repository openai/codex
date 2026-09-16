//! Identify WSLg's duplicate root without relying on filtered environment variables
//! or kernel branding. An unrelated directory at the same path is left untouched.
//! Explicit alias paths are rejected before building the restricted filesystem.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(crate) fn is_duplicate_root(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let root = fs::metadata("/")?;
            Ok(metadata.is_dir() && (metadata.dev(), metadata.ino()) == (root.dev(), root.ino()))
        }
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(false)
        }
        Err(_) => {
            // Mount metadata remains readable when an ancestor is not searchable.
            // If neither observation is available, fail rather than guess that
            // an inaccessible duplicate can safely remain in the sandbox.
            let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;
            let root = mount_identity(&mountinfo, "/")?
                .ok_or_else(|| io::Error::other("root mount identity is unavailable"))?;
            Ok(Some(root) == mount_identity(&mountinfo, &path.to_string_lossy())?)
        }
    }
}

pub(crate) fn ensure_supported_path(path: &Path) -> codex_protocol::error::Result<()> {
    let alias = crate::bwrap::WSLG_DISTRO_ROOT;
    if path.starts_with(alias)
        || fs::canonicalize(path).is_ok_and(|resolved| resolved.starts_with(alias))
    {
        return Err(codex_protocol::error::CodexErr::Fatal(format!(
            "restricted sandboxes do not support paths under {alias}: {}; use the corresponding path under the primary filesystem root instead",
            path.display()
        )));
    }
    Ok(())
}

fn mount_identity<'a>(mountinfo: &'a str, path: &str) -> io::Result<Option<(&'a str, &'a str)>> {
    let mut identities = mountinfo.lines().filter_map(|line| {
        let mut fields = line.split_ascii_whitespace();
        let device = fields.nth(2)?;
        let root = fields.next()?;
        let mountpoint = fields.next()?;
        (mountpoint == path).then_some((device, root))
    });
    let identity = identities.next();
    if identities.any(|other| Some(other) != identity) {
        return Err(io::Error::other("mount identity is ambiguous"));
    }
    Ok(identity)
}

#[cfg(test)]
#[path = "wslg_tests.rs"]
mod tests;
