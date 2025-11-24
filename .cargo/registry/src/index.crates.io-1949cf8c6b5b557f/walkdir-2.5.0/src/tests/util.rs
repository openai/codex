use std::env;
use std::error;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::result;

use crate::{DirEntry, Error};

/// Create an error from a format!-like syntax.
#[macro_export]
macro_rules! err {
    ($($tt:tt)*) => {
        Box::<dyn error::Error + Send + Sync>::from(format!($($tt)*))
    }
}

/// A convenient result type alias.
pub type Result<T> = result::Result<T, Box<dyn error::Error + Send + Sync>>;

/// The result of running a recursive directory iterator on a single directory.
#[derive(Debug)]
pub struct RecursiveResults {
    ents: Vec<DirEntry>,
    errs: Vec<Error>,
}

impl RecursiveResults {
    /// Return all of the errors encountered during traversal.
    pub fn errs(&self) -> &[Error] {
        &self.errs
    }

    /// Assert that no errors have occurred.
    pub fn assert_no_errors(&self) {
        assert!(
            self.errs.is_empty(),
            "expected to find no errors, but found: {:?}",
            self.errs
        );
    }

    /// Return all the successfully retrieved directory entries in the order
    /// in which they were retrieved.
    pub fn ents(&self) -> &[DirEntry] {
        &self.ents
    }

    /// Return all paths from all successfully retrieved directory entries.
    ///
    /// This does not include paths that correspond to an error.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.ents.iter().map(|d| d.path().to_path_buf()).collect()
    }

    /// Return all the successfully retrieved directory entries, sorted
    /// lexicographically by their full file path.
    pub fn sorted_ents(&self) -> Vec<DirEntry> {
        let mut ents = self.ents.clone();
        ents.sort_by(|e1, e2| e1.path().cmp(e2.path()));
        ents
    }

    /// Return all paths from all successfully retrieved directory entries,
    /// sorted lexicographically.
    ///
    /// This does not include paths that correspond to an error.
    pub fn sorted_paths(&self) -> Vec<PathBuf> {
        self.sorted_ents().into_iter().map(|d| d.into_path()).collect()
    }
}

/// A helper for managing a directory in which to run tests.
///
/// When manipulating paths within this directory, paths are interpreted
/// relative to this directory.
#[derive(Debug)]
pub struct Dir {
    dir: TempDir,
}

impl Dir {
    /// Create a new empty temporary directory.
    pub fn tmp() -> Dir {
        let dir = TempDir::new().unwrap();
        Dir { dir }
    }

    /// Return the path to this directory.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Return a path joined to the path to this directory.
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path().join(path)
    }

    /// Run the given iterator and return the result as a distinct collection
    /// of directory entries and errors.
    pub fn run_recursive<I>(&self, it: I) -> RecursiveResults
    where
        I: IntoIterator<Item = result::Result<DirEntry, Error>>,
    {
        let mut results = RecursiveResults { ents: vec![], errs: vec![] };
        for result in it {
            match result {
                Ok(ent) => results.ents.push(ent),
                Err(err) => results.errs.push(err),
            }
        }
        results
    }

    /// Create a directory at the given path, while creating all intermediate
    /// directories as needed.
    pub fn mkdirp<P: AsRef<Path>>(&self, path: P) {
        let full = self.join(path);
        fs::create_dir_all(&full)
            .map_err(|e| {
                err!("failed to create directory {}: {}", full.display(), e)
            })
            .unwrap();
    }

    /// Create an empty file at the given path. All ancestor directories must
    /// already exists.
    pub fn touch<P: AsRef<Path>>(&self, path: P) {
        let full = self.join(path);
        File::create(&full)
            .map_err(|e| {
                err!("failed to create file {}: {}", full.display(), e)
            })
            .unwrap();
    }

    /// Create empty files at the given paths. All ancestor directories must
    /// already exists.
    pub fn touch_all<P: AsRef<Path>>(&self, paths: &[P]) {
        for p in paths {
            self.touch(p);
        }
    }

    /// Create a file symlink to the given src with the given link name.
    pub fn symlink_file<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        src: P1,
        link_name: P2,
    ) {
        #[cfg(windows)]
        fn imp(src: &Path, link_name: &Path) -> io::Result<()> {
            use std::os::windows::fs::symlink_file;
            symlink_file(src, link_name)
        }

        #[cfg(unix)]
        fn imp(src: &Path, link_name: &Path) -> io::Result<()> {
            use std::os::unix::fs::symlink;
            symlink(src, link_name)
        }

        let (src, link_name) = (self.join(src), self.join(link_name));
        imp(&src, &link_name)
            .map_err(|e| {
                err!(
                    "failed to symlink file {} with target {}: {}",
                    src.display(),
                    link_name.display(),
                    e
                )
            })
            .unwrap()
    }

    /// Create a directory symlink to the given src with the given link name.
    pub fn symlink_dir<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        src: P1,
        link_name: P2,
    ) {
        #[cfg(windows)]
        fn imp(src: &Path, link_name: &Path) -> io::Result<()> {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(src, link_name)
        }

        #[cfg(unix)]
        fn imp(src: &Path, link_name: &Path) -> io::Result<()> {
            use std::os::unix::fs::symlink;
            symlink(src, link_name)
        }

        let (src, link_name) = (self.join(src), self.join(link_name));
        imp(&src, &link_name)
            .map_err(|e| {
                err!(
                    "failed to symlink directory {} with target {}: {}",
                    src.display(),
                    link_name.display(),
                    e
                )
            })
            .unwrap()
    }
}

/// A simple wrapper for creating a temporary directory that is automatically
/// deleted when it's dropped.
///
/// We use this in lieu of tempfile because tempfile brings in too many
/// dependencies.
#[derive(Debug)]
pub struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

impl TempDir {
    /// Create a new empty temporary directory under the system's configured
    /// temporary directory.
    pub fn new() -> Result<TempDir> {
        #[allow(deprecated)]
        use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

        static TRIES: usize = 100;
        #[allow(deprecated)]
        static COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;

        let tmpdir = env::temp_dir();
        for _ in 0..TRIES {
            let count = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = tmpdir.join("rust-walkdir").join(count.to_string());
            if path.is_dir() {
                continue;
            }
            fs::create_dir_all(&path).map_err(|e| {
                err!("failed to create {}: {}", path.display(), e)
            })?;
            return Ok(TempDir(path));
        }
        Err(err!("failed to create temp dir after {} tries", TRIES))
    }

    /// Return the underlying path to this temporary directory.
    pub fn path(&self) -> &Path {
        &self.0
    }
}
