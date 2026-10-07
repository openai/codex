//! Check declared policy access to an existing containment dependency's contents.
//! Callers own the dependency inventory, reporting, and enforcement. Filesystem
//! inspection resolves current objects; it does not assess ownership or OS ACLs.

use super::finding::IntegrityFinding;
use super::finding::IntegrityFindingDetails;
use super::policy::PreparedPolicy;
use super::policy::prepare_policy;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;

/// Rebuild for each effective policy and executor filesystem snapshot.
pub struct FileContentsChecker {
    policy: PreparedPolicy,
}

impl FileContentsChecker {
    /// Takes the prepared executor policy: symbolic roots must be materialized,
    /// and backend snapshot expansion of globs (including scan limits) must
    /// already be reflected in its entries. Remaining globs use policy matching.
    /// Callers must handle full-disk write access separately.
    pub fn new(policy: &FileSystemSandboxPolicy, cwd: &AbsolutePathBuf) -> io::Result<Self> {
        let mut policy = prepare_policy(policy, cwd)?;
        let mut roots = Vec::new();
        for mut root in policy.roots {
            let Some(resolved_root) = resolve_existing(&root.root)? else {
                continue;
            };
            if !fs::metadata(&resolved_root)?.is_dir() {
                root.protected_metadata_names.clear();
            }
            // Resolve existing carveouts and protected metadata aliases while
            // retaining missing paths unchanged.
            let mut exclusions = Vec::new();
            for path in root.read_only_subpaths.iter().cloned().chain(
                root.protected_metadata_names
                    .iter()
                    .map(|name| root.root.join(name)),
            ) {
                exclusions.push(resolve_existing(&path)?.unwrap_or(path));
            }
            root.root = resolved_root;
            root.read_only_subpaths = exclusions;
            roots.push(root);
        }
        policy.roots = roots;
        Ok(Self { policy })
    }

    pub fn check(&self, target: &AbsolutePathBuf) -> io::Result<Option<IntegrityFinding>> {
        match fs::metadata(target) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "containment dependency is not a file",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
        let resolved_target = target.canonicalize()?;
        // Denying an alias does not establish protection of the resolved contents.
        if self
            .policy
            .glob_denials
            .as_ref()
            .is_some_and(|matcher| matcher.is_local_path_read_denied(resolved_target.as_path()))
        {
            return Ok(None);
        }
        let write_roots: Vec<_> = self
            .policy
            .roots
            .iter()
            .filter(|root| root.is_path_writable(resolved_target.as_path()))
            .map(|root| root.root.clone())
            .collect();
        Ok((!write_roots.is_empty()).then(|| IntegrityFinding {
            target: target.clone(),
            write_roots,
            details: IntegrityFindingDetails::Contents { resolved_target },
        }))
    }
}

fn resolve_existing(path: &AbsolutePathBuf) -> io::Result<Option<AbsolutePathBuf>> {
    match path.canonicalize() {
        Ok(path) => Ok(Some(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "contents_tests.rs"]
mod tests;
