use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::Context;
use anyhow::Result;
use ignore::WalkBuilder;
use sha2::Digest;
use sha2::Sha256;
use tokio::task;
use tracing::warn;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CodebaseSnapshot {
    root: PathBuf,
    entries: BTreeMap<String, EntryFingerprint>,
    root_digest: DigestBytes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryFingerprint {
    pub kind: EntryKind,
    pub digest: DigestBytes,
    pub size: u64,
    pub modified_millis: Option<u128>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum EntryKind {
    File,
    Symlink,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct SnapshotDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

impl SnapshotDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

pub(crate) type DigestBytes = [u8; 32];

impl CodebaseSnapshot {
    pub(crate) async fn capture(root: PathBuf) -> Result<Self> {
        task::spawn_blocking(move || Self::from_disk(&root))
            .await
            .map_err(|e| anyhow::anyhow!("codebase snapshot task failed: {e}"))?
    }

    pub(crate) fn from_disk(root: &Path) -> Result<Self> {
        if !root.exists() {
            return Ok(Self::empty(root));
        }

        let mut entries: BTreeMap<String, EntryFingerprint> = BTreeMap::new();

        let mut walker = WalkBuilder::new(root);
        walker
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .parents(true)
            .ignore(true)
            .follow_links(false);

        for result in walker.build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(err) => {
                    warn!("codebase snapshot failed to read entry: {err}");
                    continue;
                }
            };

            let path = entry.path();
            if entry.depth() == 0 {
                continue;
            }

            let relative = match path.strip_prefix(root) {
                Ok(rel) => rel,
                Err(_) => continue,
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let rel_string = normalize_rel_path(relative);

            let file_type = match entry.file_type() {
                Some(file_type) => file_type,
                None => continue,
            };

            if file_type.is_dir() {
                continue;
            }

            if file_type.is_file() {
                match fingerprint_file(path) {
                    Ok(fp) => {
                        entries.insert(rel_string, fp);
                    }
                    Err(err) => {
                        warn!(
                            "codebase snapshot failed to hash file {}: {err}",
                            path.display()
                        );
                    }
                }
                continue;
            }

            if file_type.is_symlink() {
                match fingerprint_symlink(path) {
                    Ok(fp) => {
                        entries.insert(rel_string, fp);
                    }
                    Err(err) => {
                        warn!(
                            "codebase snapshot failed to hash symlink {}: {err}",
                            path.display()
                        );
                    }
                }
                continue;
            }
        }

        let root_digest = compute_root_digest(&entries);

        Ok(Self {
            root: root.to_path_buf(),
            entries,
            root_digest,
        })
    }

    pub(crate) fn diff(&self, newer: &CodebaseSnapshot) -> SnapshotDiff {
        let mut diff = SnapshotDiff::default();

        for (path, fingerprint) in &newer.entries {
            match self.entries.get(path) {
                None => diff.added.push(path.clone()),
                Some(existing) if existing != fingerprint => diff.modified.push(path.clone()),
                _ => {}
            }
        }

        for path in self.entries.keys() {
            if !newer.entries.contains_key(path) {
                diff.removed.push(path.clone());
            }
        }

        diff
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn empty(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            entries: BTreeMap::new(),
            root_digest: Sha256::digest(b"").into(),
        }
    }
}

fn fingerprint_file(path: &Path) -> Result<EntryFingerprint> {
    let metadata = path
        .metadata()
        .with_context(|| format!("metadata {}", path.display()))?;
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(EntryFingerprint {
        kind: EntryKind::File,
        digest: hasher.finalize().into(),
        size: metadata.len(),
        modified_millis: metadata.modified().ok().and_then(system_time_to_millis),
    })
}

fn fingerprint_symlink(path: &Path) -> Result<EntryFingerprint> {
    let target =
        std::fs::read_link(path).with_context(|| format!("read_link {}", path.display()))?;
    let mut hasher = Sha256::new();
    let target_str = normalize_rel_path(&target);
    hasher.update(target_str.as_bytes());
    Ok(EntryFingerprint {
        kind: EntryKind::Symlink,
        digest: hasher.finalize().into(),
        size: 0,
        modified_millis: None,
    })
}

fn compute_root_digest(entries: &BTreeMap<String, EntryFingerprint>) -> DigestBytes {
    let mut hasher = Sha256::new();
    for (path, fingerprint) in entries {
        hasher.update(path.as_bytes());
        hasher.update(fingerprint.digest);
        hasher.update([fingerprint.kind as u8]);
        hasher.update(fingerprint.size.to_le_bytes());
        if let Some(modified) = fingerprint.modified_millis {
            hasher.update(modified.to_le_bytes());
        }
    }
    hasher.finalize().into()
}

fn normalize_rel_path(path: &Path) -> String {
    let s = path_to_cow(path);
    if s.is_empty() {
        String::new()
    } else {
        s.replace('\\', "/")
    }
}

fn path_to_cow(path: &Path) -> Cow<'_, str> {
    path.to_string_lossy()
}

fn system_time_to_millis(ts: SystemTime) -> Option<u128> {
    ts.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn diff_tracks_added_modified_removed() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        std::fs::write(root.join("file_a.txt"), "alpha").unwrap();
        std::fs::write(root.join("file_b.txt"), "bravo").unwrap();
        let snapshot_one = CodebaseSnapshot::from_disk(root).unwrap();

        std::fs::write(root.join("file_a.txt"), "alpha-updated").unwrap();
        std::fs::remove_file(root.join("file_b.txt")).unwrap();
        std::fs::write(root.join("file_c.txt"), "charlie").unwrap();
        let snapshot_two = CodebaseSnapshot::from_disk(root).unwrap();

        let diff = snapshot_one.diff(&snapshot_two);
        assert_eq!(diff.added, vec!["file_c.txt".to_string()]);
        assert_eq!(diff.modified, vec!["file_a.txt".to_string()]);
        assert_eq!(diff.removed, vec!["file_b.txt".to_string()]);
    }
}
