//! Prepare policy grants and glob matching shared by containment integrity checks.
//! Each checker owns further filesystem resolution of these roots and carveouts.

use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;

pub(super) struct PreparedPolicy {
    pub roots: Vec<WritableRoot>,
    pub glob_denials: Option<ReadDenyMatcher>,
}

pub(super) fn prepare_policy(
    policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> io::Result<PreparedPolicy> {
    // Writable roots already resolve precedence for exact-path rules.
    let mut glob_policy = policy.clone();
    glob_policy
        .entries
        .retain(|entry| matches!(entry.path, FileSystemPath::GlobPattern { .. }));
    let glob_denials = ReadDenyMatcher::try_new_for_local_paths(&glob_policy, cwd.as_path())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(PreparedPolicy {
        roots: policy.get_writable_roots_with_cwd(cwd.as_path()),
        glob_denials,
    })
}
