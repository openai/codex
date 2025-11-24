// Copyright 2015 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ffi::OsStr;
use std::fs::remove_dir_all;
use std::mem;
use std::path::{self, Path, PathBuf};
use std::{fmt, io};

use crate::error::IoResultExt;
use crate::Builder;

#[cfg(doc)]
use crate::env;

/// Create a new temporary directory. Also see [`tempdir_in`].
///
/// The `tempdir` function creates a directory in the file system and returns a
/// [`TempDir`]. The directory will be automatically deleted when the `TempDir`'s
/// destructor is run.
///
/// # Resource Leaking
///
/// See [the resource leaking][resource-leaking] docs on `TempDir`.
///
/// # Security
///
/// Temporary directories are created with the default permissions unless otherwise
/// specified via [`Builder::permissions`]. Depending on your platform, this may make
/// them world-readable.
///
/// # Errors
///
/// If the directory can not be created, `Err` is returned.
///
/// # Examples
///
/// ```
/// use tempfile::tempdir;
/// use std::fs::File;
/// use std::io::Write;
///
/// // Create a directory inside of `env::temp_dir()`
/// let tmp_dir = tempdir()?;
///
/// let file_path = tmp_dir.path().join("my-temporary-note.txt");
/// let mut tmp_file = File::create(file_path)?;
/// writeln!(tmp_file, "Brian was here. Briefly.")?;
///
/// // `tmp_dir` goes out of scope, the directory as well as
/// // `tmp_file` will be deleted here.
/// drop(tmp_file);
/// tmp_dir.close()?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// [`TempDir`]: struct.TempDir.html
/// [resource-leaking]: struct.TempDir.html#resource-leaking
pub fn tempdir() -> io::Result<TempDir> {
    TempDir::new()
}

/// Create a new temporary directory in a specific directory. Also see [`tempdir`].
///
/// The `tempdir_in` function creates a directory in the specified directory
/// and returns a [`TempDir`].
/// The directory will be automatically deleted when the `TempDir`s
/// destructor is run.
///
/// # Resource Leaking
///
/// See [the resource leaking][resource-leaking] docs on `TempDir`.
///
/// # Errors
///
/// If the directory can not be created, `Err` is returned.
///
/// # Examples
///
/// ```
/// use tempfile::tempdir_in;
/// use std::fs::File;
/// use std::io::Write;
///
/// // Create a directory inside of the current directory.
/// let tmp_dir = tempdir_in(".")?;
///
/// let file_path = tmp_dir.path().join("my-temporary-note.txt");
/// let mut tmp_file = File::create(file_path)?;
/// writeln!(tmp_file, "Brian was here. Briefly.")?;
///
/// // `tmp_dir` goes out of scope, the directory as well as
/// // `tmp_file` will be deleted here.
/// drop(tmp_file);
/// tmp_dir.close()?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// [`TempDir`]: struct.TempDir.html
/// [resource-leaking]: struct.TempDir.html#resource-leaking
pub fn tempdir_in<P: AsRef<Path>>(dir: P) -> io::Result<TempDir> {
    TempDir::new_in(dir)
}

/// A directory in the filesystem that is automatically deleted when
/// it goes out of scope.
///
/// The [`TempDir`] type creates a directory on the file system that
/// is deleted once it goes out of scope. At construction, the
/// `TempDir` creates a new directory with a randomly generated name.
///
/// The default constructor, [`TempDir::new()`], creates directories in
/// the location returned by [`env::temp_dir()`], but `TempDir`
/// can be configured to manage a temporary directory in any location
/// by constructing with a [`Builder`].
///
/// After creating a `TempDir`, work with the file system by doing
/// standard [`std::fs`] file system operations on its [`Path`],
/// which can be retrieved with [`TempDir::path()`]. Once the `TempDir`
/// value is dropped, the directory at the path will be deleted, along
/// with any files and directories it contains. It is your responsibility
/// to ensure that no further file system operations are attempted
/// inside the temporary directory once it has been deleted.
///
/// # Resource Leaking
///
/// Various platform-specific conditions may cause `TempDir` to fail
/// to delete the underlying directory. It's important to ensure that
/// handles (like [`File`] and [`ReadDir`]) to files inside the
/// directory are dropped before the `TempDir` goes out of scope. The
/// `TempDir` destructor will silently ignore any errors in deleting
/// the directory; to instead handle errors call [`TempDir::close()`].
///
/// Note that if the program exits before the `TempDir` destructor is
/// run, such as via [`std::process::exit()`], by segfaulting, or by
/// receiving a signal like `SIGINT`, then the temporary directory
/// will not be deleted.
///
/// # Examples
///
/// Create a temporary directory with a generated name:
///
/// ```
/// use std::fs::File;
/// use std::io::Write;
/// use tempfile::TempDir;
///
/// // Create a directory inside of `env::temp_dir()`
/// let tmp_dir = TempDir::new()?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// Create a temporary directory with a prefix in its name:
///
/// ```
/// use std::fs::File;
/// use std::io::Write;
/// use tempfile::Builder;
///
/// // Create a directory inside of `env::temp_dir()`,
/// // whose name will begin with 'example'.
/// let tmp_dir = Builder::new().prefix("example").tempdir()?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// [`File`]: http://doc.rust-lang.org/std/fs/struct.File.html
/// [`Path`]: http://doc.rust-lang.org/std/path/struct.Path.html
/// [`ReadDir`]: http://doc.rust-lang.org/std/fs/struct.ReadDir.html
/// [`Builder`]: struct.Builder.html
/// [`TempDir::close()`]: struct.TempDir.html#method.close
/// [`TempDir::new()`]: struct.TempDir.html#method.new
/// [`TempDir::path()`]: struct.TempDir.html#method.path
/// [`TempDir`]: struct.TempDir.html
/// [`std::fs`]: http://doc.rust-lang.org/std/fs/index.html
/// [`std::process::exit()`]: http://doc.rust-lang.org/std/process/fn.exit.html
pub struct TempDir {
    path: Box<Path>,
    disable_cleanup: bool,
}

impl TempDir {
    /// Attempts to make a temporary directory inside of `env::temp_dir()`.
    ///
    /// See [`Builder`] for more configuration.
    ///
    /// The directory and everything inside it will be automatically deleted
    /// once the returned `TempDir` is destroyed.
    ///
    /// # Errors
    ///
    /// If the directory can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::File;
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of `env::temp_dir()`
    /// let tmp_dir = TempDir::new()?;
    ///
    /// let file_path = tmp_dir.path().join("my-temporary-note.txt");
    /// let mut tmp_file = File::create(file_path)?;
    /// writeln!(tmp_file, "Brian was here. Briefly.")?;
    ///
    /// // `tmp_dir` goes out of scope, the directory as well as
    /// // `tmp_file` will be deleted here.
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`Builder`]: struct.Builder.html
    pub fn new() -> io::Result<TempDir> {
        Builder::new().tempdir()
    }

    /// Attempts to make a temporary directory inside of `dir`.
    /// The directory and everything inside it will be automatically
    /// deleted once the returned `TempDir` is destroyed.
    ///
    /// # Errors
    ///
    /// If the directory can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::{self, File};
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of the current directory
    /// let tmp_dir = TempDir::new_in(".")?;
    /// let file_path = tmp_dir.path().join("my-temporary-note.txt");
    /// let mut tmp_file = File::create(file_path)?;
    /// writeln!(tmp_file, "Brian was here. Briefly.")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn new_in<P: AsRef<Path>>(dir: P) -> io::Result<TempDir> {
        Builder::new().tempdir_in(dir)
    }

    /// Attempts to make a temporary directory with the specified prefix inside of
    /// `env::temp_dir()`. The directory and everything inside it will be automatically
    /// deleted once the returned `TempDir` is destroyed.
    ///
    /// # Errors
    ///
    /// If the directory can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::{self, File};
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of the current directory
    /// let tmp_dir = TempDir::with_prefix("foo-")?;
    /// let tmp_name = tmp_dir.path().file_name().unwrap().to_str().unwrap();
    /// assert!(tmp_name.starts_with("foo-"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn with_prefix<S: AsRef<OsStr>>(prefix: S) -> io::Result<TempDir> {
        Builder::new().prefix(&prefix).tempdir()
    }

    /// Attempts to make a temporary directory with the specified suffix inside of
    /// `env::temp_dir()`. The directory and everything inside it will be automatically
    /// deleted once the returned `TempDir` is destroyed.
    ///
    /// # Errors
    ///
    /// If the directory can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::{self, File};
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of the current directory
    /// let tmp_dir = TempDir::with_suffix("-foo")?;
    /// let tmp_name = tmp_dir.path().file_name().unwrap().to_str().unwrap();
    /// assert!(tmp_name.ends_with("-foo"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn with_suffix<S: AsRef<OsStr>>(suffix: S) -> io::Result<TempDir> {
        Builder::new().suffix(&suffix).tempdir()
    }
    /// Attempts to make a temporary directory with the specified prefix inside
    /// the specified directory. The directory and everything inside it will be
    /// automatically deleted once the returned `TempDir` is destroyed.
    ///
    /// # Errors
    ///
    /// If the directory can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::{self, File};
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of the current directory
    /// let tmp_dir = TempDir::with_suffix_in("-foo", ".")?;
    /// let tmp_name = tmp_dir.path().file_name().unwrap().to_str().unwrap();
    /// assert!(tmp_name.ends_with("-foo"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn with_suffix_in<S: AsRef<OsStr>, P: AsRef<Path>>(
        suffix: S,
        dir: P,
    ) -> io::Result<TempDir> {
        Builder::new().suffix(&suffix).tempdir_in(dir)
    }

    /// Attempts to make a temporary directory with the specified prefix inside
    /// the specified directory. The directory and everything inside it will be
    /// automatically deleted once the returned `TempDir` is destroyed.
    ///
    /// # Errors
    ///
    /// If the directory can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::{self, File};
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of the current directory
    /// let tmp_dir = TempDir::with_prefix_in("foo-", ".")?;
    /// let tmp_name = tmp_dir.path().file_name().unwrap().to_str().unwrap();
    /// assert!(tmp_name.starts_with("foo-"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn with_prefix_in<S: AsRef<OsStr>, P: AsRef<Path>>(
        prefix: S,
        dir: P,
    ) -> io::Result<TempDir> {
        Builder::new().prefix(&prefix).tempdir_in(dir)
    }

    /// Accesses the [`Path`] to the temporary directory.
    ///
    /// [`Path`]: http://doc.rust-lang.org/std/path/struct.Path.html
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::TempDir;
    ///
    /// let tmp_path;
    ///
    /// {
    ///    let tmp_dir = TempDir::new()?;
    ///    tmp_path = tmp_dir.path().to_owned();
    ///
    ///    // Check that the temp directory actually exists.
    ///    assert!(tmp_path.exists());
    ///
    ///    // End of `tmp_dir` scope, directory will be deleted
    /// }
    ///
    /// // Temp directory should be deleted by now
    /// assert_eq!(tmp_path.exists(), false);
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[must_use]
    pub fn path(&self) -> &path::Path {
        self.path.as_ref()
    }

    /// Deprecated alias for [`TempDir::keep`].
    #[must_use]
    #[deprecated = "use TempDir::keep()"]
    pub fn into_path(self) -> PathBuf {
        self.keep()
    }

    /// Persist the temporary directory to disk, returning the [`PathBuf`] where it is located.
    ///
    /// This consumes the [`TempDir`] without deleting directory on the filesystem, meaning that
    /// the directory will no longer be automatically deleted.
    ///
    /// If you want to disable automatic cleanup of the temporary directory in-place, keeping the
    /// `TempDir` as-is, use [`TempDir::disable_cleanup`] instead.
    ///
    /// [`TempDir`]: struct.TempDir.html
    /// [`PathBuf`]: http://doc.rust-lang.org/std/path/struct.PathBuf.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs;
    /// use tempfile::TempDir;
    ///
    /// let tmp_dir = TempDir::new()?;
    ///
    /// // Persist the temporary directory to disk,
    /// // getting the path where it is.
    /// let tmp_path = tmp_dir.keep();
    ///
    /// // Delete the temporary directory ourselves.
    /// fs::remove_dir_all(tmp_path)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[must_use]
    pub fn keep(mut self) -> PathBuf {
        self.disable_cleanup(true);
        mem::replace(&mut self.path, PathBuf::new().into_boxed_path()).into()
    }

    /// Disable cleanup of the temporary directory. If `disable_cleanup` is `true`, the temporary
    /// directory will not be deleted when this `TempDir` is dropped. This method is equivalent to
    /// calling [`Builder::disable_cleanup`] when creating the `TempDir`.
    ///
    /// **NOTE:** this method is primarily useful for testing/debugging. If you want to simply turn
    /// a temporary directory into a non-temporary directory, prefer [`TempDir::keep`].
    pub fn disable_cleanup(&mut self, disable_cleanup: bool) {
        self.disable_cleanup = disable_cleanup
    }

    /// Closes and removes the temporary directory, returning a `Result`.
    ///
    /// Although `TempDir` removes the directory on drop, in the destructor
    /// any errors are ignored. To detect errors cleaning up the temporary
    /// directory, call `close` instead.
    ///
    /// # Errors
    ///
    /// This function may return a variety of [`std::io::Error`]s that result from deleting
    /// the files and directories contained with the temporary directory,
    /// as well as from deleting the temporary directory itself. These errors
    /// may be platform specific.
    ///
    /// [`std::io::Error`]: http://doc.rust-lang.org/std/io/struct.Error.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::File;
    /// use std::io::Write;
    /// use tempfile::TempDir;
    ///
    /// // Create a directory inside of `env::temp_dir()`.
    /// let tmp_dir = TempDir::new()?;
    /// let file_path = tmp_dir.path().join("my-temporary-note.txt");
    /// let mut tmp_file = File::create(file_path)?;
    /// writeln!(tmp_file, "Brian was here. Briefly.")?;
    ///
    /// // By closing the `TempDir` explicitly we can check that it has
    /// // been deleted successfully. If we don't close it explicitly,
    /// // the directory will still be deleted when `tmp_dir` goes out
    /// // of scope, but we won't know whether deleting the directory
    /// // succeeded.
    /// drop(tmp_file);
    /// tmp_dir.close()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn close(mut self) -> io::Result<()> {
        let result = remove_dir_all(self.path()).with_err_path(|| self.path());

        // Set self.path to empty Box to release the memory, since an empty
        // Box does not allocate any heap memory.
        self.path = PathBuf::new().into_boxed_path();

        // Prevent the Drop impl from being called.
        mem::forget(self);

        result
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl fmt::Debug for TempDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TempDir")
            .field("path", &self.path())
            .finish()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.disable_cleanup {
            let _ = remove_dir_all(self.path());
        }
    }
}

pub(crate) fn create(
    path: PathBuf,
    permissions: Option<&std::fs::Permissions>,
    disable_cleanup: bool,
) -> io::Result<TempDir> {
    imp::create(path, permissions, disable_cleanup)
}

mod imp;
