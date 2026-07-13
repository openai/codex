use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

/// Private environment key used to point the packaged shim at an
/// exec-server-owned cache generation.
pub const CACHE_ROOT_ENV: &str = "CODEX_INTERNAL_RG_CACHE_ROOT";

const FILES_FILENAME: &str = "files";
const READY_FILENAME: &str = "ready";
const ROOT_FILENAME: &str = "root";

#[derive(Clone, Debug, Eq, PartialEq)]
struct InventoryCache {
    directory: PathBuf,
    files: PathBuf,
    ready: PathBuf,
    root: PathBuf,
    root_text: String,
}

impl InventoryCache {
    fn new(cache_root: &Path, repository_root: &Path) -> Option<Self> {
        let root_text = repository_root.to_str()?.to_string();
        let key = format!("{:x}", Sha256::digest(root_text.as_bytes()));
        let directory = cache_root.join(key);
        Some(Self {
            files: directory.join(FILES_FILENAME),
            ready: directory.join(READY_FILENAME),
            root: directory.join(ROOT_FILENAME),
            directory,
            root_text,
        })
    }

    fn open(&self) -> Option<File> {
        let generation_before = fs::read(&self.ready).ok()?;
        if generation_before.is_empty() || fs::read_to_string(&self.root).ok()? != self.root_text {
            return None;
        }
        let files = File::open(&self.files).ok()?;
        let generation_after = fs::read(&self.ready).ok()?;
        (generation_before == generation_after).then_some(files)
    }
}

/// Opens an exact cached `rg --files` result for `cwd` when the executor has
/// published a live generation for the containing Git worktree.
pub fn open_file_inventory(cache_root: &Path, cwd: &Path) -> Option<File> {
    let repository_root = find_repository_root(cwd)?;
    if fs::canonicalize(cwd).ok()? != repository_root {
        return None;
    }
    InventoryCache::new(cache_root, &repository_root)?.open()
}

pub(crate) fn find_repository_root(cwd: &Path) -> Option<PathBuf> {
    let cwd = fs::canonicalize(cwd).ok()?;
    cwd.ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

mod manager;
pub use manager::RgCacheManager;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
