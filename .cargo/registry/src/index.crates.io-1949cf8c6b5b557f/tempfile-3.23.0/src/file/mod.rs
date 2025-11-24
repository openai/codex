use std::error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::mem;
use std::ops::Deref;
#[cfg(target_os = "wasi")]
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
#[cfg(unix)] // we don't use std::os::fd because that's not available on rust 1.63.
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, RawHandle};
use std::path::{Path, PathBuf};

use crate::env;
use crate::error::IoResultExt;
use crate::Builder;

mod imp;

/// Create a new temporary file. Also see [`tempfile_in`].
///
/// The file will be created in the location returned by [`env::temp_dir()`].
///
/// # Security
///
/// This variant is secure/reliable in the presence of a pathological temporary file cleaner.
///
/// # Resource Leaking
///
/// The temporary file will be automatically removed by the OS when the last handle to it is closed.
/// This doesn't rely on Rust destructors being run, so will (almost) never fail to clean up the temporary file.
///
/// # Errors
///
/// If the file can not be created, `Err` is returned.
///
/// # Examples
///
/// ```
/// use tempfile::tempfile;
/// use std::io::Write;
///
/// // Create a file inside of `env::temp_dir()`.
/// let mut file = tempfile()?;
///
/// writeln!(file, "Brian was here. Briefly.")?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn tempfile() -> io::Result<File> {
    tempfile_in(env::temp_dir())
}

/// Create a new temporary file in the specified directory. Also see [`tempfile`].
///
/// # Security
///
/// This variant is secure/reliable in the presence of a pathological temporary file cleaner.
/// If the temporary file isn't created in [`env::temp_dir()`] then temporary file cleaners aren't an issue.
///
/// # Resource Leaking
///
/// The temporary file will be automatically removed by the OS when the last handle to it is closed.
/// This doesn't rely on Rust destructors being run, so will (almost) never fail to clean up the temporary file.
///
/// # Errors
///
/// If the file can not be created, `Err` is returned.
///
/// # Examples
///
/// ```
/// use tempfile::tempfile_in;
/// use std::io::Write;
///
/// // Create a file inside of the current working directory
/// let mut file = tempfile_in("./")?;
///
/// writeln!(file, "Brian was here. Briefly.")?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn tempfile_in<P: AsRef<Path>>(dir: P) -> io::Result<File> {
    imp::create(dir.as_ref())
}

/// Error returned when persisting a temporary file path fails.
#[derive(Debug)]
pub struct PathPersistError {
    /// The underlying IO error.
    pub error: io::Error,
    /// The temporary file path that couldn't be persisted.
    pub path: TempPath,
}

impl From<PathPersistError> for io::Error {
    #[inline]
    fn from(error: PathPersistError) -> io::Error {
        error.error
    }
}

impl From<PathPersistError> for TempPath {
    #[inline]
    fn from(error: PathPersistError) -> TempPath {
        error.path
    }
}

impl fmt::Display for PathPersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to persist temporary file path: {}", self.error)
    }
}

impl error::Error for PathPersistError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.error)
    }
}

/// A path to a named temporary file without an open file handle.
///
/// This is useful when the temporary file needs to be used by a child process,
/// for example.
///
/// When dropped, the temporary file is deleted unless `disable_cleanup(true)` was called on the
/// builder that constructed this temporary file and/or was called on either this `TempPath` or the
/// `NamedTempFile` from which this `TempPath` was constructed.
pub struct TempPath {
    path: Box<Path>,
    disable_cleanup: bool,
}

impl TempPath {
    /// Close and remove the temporary file.
    ///
    /// Use this if you want to detect errors in deleting the file.
    ///
    /// # Errors
    ///
    /// If the file cannot be deleted, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tempfile::NamedTempFile;
    ///
    /// let file = NamedTempFile::new()?;
    ///
    /// // Close the file, but keep the path to it around.
    /// let path = file.into_temp_path();
    ///
    /// // By closing the `TempPath` explicitly, we can check that it has
    /// // been deleted successfully. If we don't close it explicitly, the
    /// // file will still be deleted when `file` goes out of scope, but we
    /// // won't know whether deleting the file succeeded.
    /// path.close()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn close(mut self) -> io::Result<()> {
        let result = fs::remove_file(&self.path).with_err_path(|| &*self.path);
        self.path = PathBuf::new().into_boxed_path();
        mem::forget(self);
        result
    }

    /// Persist the temporary file at the target path.
    ///
    /// If a file exists at the target path, persist will atomically replace it.
    /// If this method fails, it will return `self` in the resulting
    /// [`PathPersistError`].
    ///
    /// Note: Temporary files cannot be persisted across filesystems. Also
    /// neither the file contents nor the containing directory are
    /// synchronized, so the update may not yet have reached the disk when
    /// `persist` returns.
    ///
    /// # Security
    ///
    /// Only use this method if you're positive that a temporary file cleaner
    /// won't have deleted your file. Otherwise, you might end up persisting an
    /// attacker controlled file.
    ///
    /// # Errors
    ///
    /// If the file cannot be moved to the new location, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use tempfile::NamedTempFile;
    ///
    /// let mut file = NamedTempFile::new()?;
    /// writeln!(file, "Brian was here. Briefly.")?;
    ///
    /// let path = file.into_temp_path();
    /// path.persist("./saved_file.txt")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`PathPersistError`]: struct.PathPersistError.html
    pub fn persist<P: AsRef<Path>>(mut self, new_path: P) -> Result<(), PathPersistError> {
        match imp::persist(&self.path, new_path.as_ref(), true) {
            Ok(_) => {
                // Don't drop `self`. We don't want to try deleting the old
                // temporary file path. (It'll fail, but the failure is never
                // seen.)
                self.path = PathBuf::new().into_boxed_path();
                mem::forget(self);
                Ok(())
            }
            Err(e) => Err(PathPersistError {
                error: e,
                path: self,
            }),
        }
    }

    /// Persist the temporary file at the target path if and only if no file exists there.
    ///
    /// If a file exists at the target path, fail. If this method fails, it will
    /// return `self` in the resulting [`PathPersistError`].
    ///
    /// Note: Temporary files cannot be persisted across filesystems. Also Note:
    /// This method is not atomic. It can leave the original link to the
    /// temporary file behind.
    ///
    /// # Security
    ///
    /// Only use this method if you're positive that a temporary file cleaner
    /// won't have deleted your file. Otherwise, you might end up persisting an
    /// attacker controlled file.
    ///
    /// # Errors
    ///
    /// If the file cannot be moved to the new location or a file already exists
    /// there, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tempfile::NamedTempFile;
    /// use std::io::Write;
    ///
    /// let mut file = NamedTempFile::new()?;
    /// writeln!(file, "Brian was here. Briefly.")?;
    ///
    /// let path = file.into_temp_path();
    /// path.persist_noclobber("./saved_file.txt")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`PathPersistError`]: struct.PathPersistError.html
    pub fn persist_noclobber<P: AsRef<Path>>(
        mut self,
        new_path: P,
    ) -> Result<(), PathPersistError> {
        match imp::persist(&self.path, new_path.as_ref(), false) {
            Ok(_) => {
                // Don't drop `self`. We don't want to try deleting the old
                // temporary file path. (It'll fail, but the failure is never
                // seen.)
                self.path = PathBuf::new().into_boxed_path();
                mem::forget(self);
                Ok(())
            }
            Err(e) => Err(PathPersistError {
                error: e,
                path: self,
            }),
        }
    }

    /// Keep the temporary file from being deleted. This function will turn the
    /// temporary file into a non-temporary file without moving it.
    ///
    /// # Errors
    ///
    /// On some platforms (e.g., Windows), we need to mark the file as
    /// non-temporary. This operation could fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use tempfile::NamedTempFile;
    ///
    /// let mut file = NamedTempFile::new()?;
    /// writeln!(file, "Brian was here. Briefly.")?;
    ///
    /// let path = file.into_temp_path();
    /// let path = path.keep()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`PathPersistError`]: struct.PathPersistError.html
    pub fn keep(mut self) -> Result<PathBuf, PathPersistError> {
        match imp::keep(&self.path) {
            Ok(_) => {
                self.disable_cleanup(true);
                Ok(mem::replace(
                    &mut self.path,
                    // Replace with an empty boxed path buf, this doesn't allocate.
                    PathBuf::new().into_boxed_path(),
                )
                .into_path_buf())
            }
            Err(e) => Err(PathPersistError {
                error: e,
                path: self,
            }),
        }
    }

    /// Disable cleanup of the temporary file. If `disable_cleanup` is `true`, the temporary file
    /// will not be deleted when this `TempPath` is dropped. This method is equivalent to calling
    /// [`Builder::disable_cleanup`] when creating the original `NamedTempFile`, which see for
    /// relevant warnings.
    ///
    /// **NOTE:** this method is primarily useful for testing/debugging. If you want to simply turn
    /// a temporary file-path into a non-temporary file-path, prefer [`TempPath::keep`].
    pub fn disable_cleanup(&mut self, disable_cleanup: bool) {
        self.disable_cleanup = disable_cleanup
    }

    /// Create a new TempPath from an existing path. This can be done even if no
    /// file exists at the given path.
    ///
    /// This is mostly useful for interacting with libraries and external
    /// components that provide files to be consumed or expect a path with no
    /// existing file to be given.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into().into_boxed_path(),
            disable_cleanup: false,
        }
    }

    pub(crate) fn new(path: PathBuf, disable_cleanup: bool) -> Self {
        Self {
            path: path.into_boxed_path(),
            disable_cleanup,
        }
    }
}

impl fmt::Debug for TempPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        if !self.disable_cleanup {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Deref for TempPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<OsStr> for TempPath {
    fn as_ref(&self) -> &OsStr {
        self.path.as_os_str()
    }
}

/// A named temporary file.
///
/// The default constructor, [`NamedTempFile::new()`], creates files in
/// the location returned by [`env::temp_dir()`], but `NamedTempFile`
/// can be configured to manage a temporary file in any location
/// by constructing with [`NamedTempFile::new_in()`].
///
/// # Security
///
/// Most operating systems employ temporary file cleaners to delete old
/// temporary files. Unfortunately these temporary file cleaners don't always
/// reliably _detect_ whether the temporary file is still being used.
///
/// Specifically, the following sequence of events can happen:
///
/// 1. A user creates a temporary file with `NamedTempFile::new()`.
/// 2. Time passes.
/// 3. The temporary file cleaner deletes (unlinks) the temporary file from the
///    filesystem.
/// 4. Some other program creates a new file to replace this deleted temporary
///    file.
/// 5. The user tries to re-open the temporary file (in the same program or in a
///    different program) by path. Unfortunately, they'll end up opening the
///    file created by the other program, not the original file.
///
/// ## Operating System Specific Concerns
///
/// The behavior of temporary files and temporary file cleaners differ by
/// operating system.
///
/// ### Windows
///
/// On Windows, temporary files are, by default, created in per-user temporary
/// file directories so only an application running as the same user would be
/// able to interfere (which they could do anyways). However, an application
/// running as the same user can still _accidentally_ re-create deleted
/// temporary files if the number of random bytes in the temporary file name is
/// too small.
///
/// ### MacOS
///
/// Like on Windows, temporary files are created in per-user temporary file
/// directories by default so calling `NamedTempFile::new()` should be
/// relatively safe.
///
/// ### Linux
///
/// Unfortunately, most _Linux_ distributions don't create per-user temporary
/// file directories. Worse, systemd's tmpfiles daemon (a common temporary file
/// cleaner) will happily remove open temporary files if they haven't been
/// modified within the last 10 days.
///
/// # Resource Leaking
///
/// If the program exits before the `NamedTempFile` destructor is
/// run, the temporary file will not be deleted. This can happen
/// if the process exits using [`std::process::exit()`], a segfault occurs,
/// receiving an interrupt signal like `SIGINT` that is not handled, or by using
/// a statically declared `NamedTempFile` instance (like with [`lazy_static`]).
///
/// Use the [`tempfile()`] function unless you need a named file path.
///
/// [`tempfile()`]: fn.tempfile.html
/// [`NamedTempFile::new()`]: #method.new
/// [`NamedTempFile::new_in()`]: #method.new_in
/// [`std::process::exit()`]: http://doc.rust-lang.org/std/process/fn.exit.html
/// [`lazy_static`]: https://github.com/rust-lang-nursery/lazy-static.rs/issues/62
pub struct NamedTempFile<F = File> {
    path: TempPath,
    file: F,
}

impl<F> fmt::Debug for NamedTempFile<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NamedTempFile({:?})", self.path)
    }
}

impl<F> AsRef<Path> for NamedTempFile<F> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

/// Error returned when persisting a temporary file fails.
pub struct PersistError<F = File> {
    /// The underlying IO error.
    pub error: io::Error,
    /// The temporary file that couldn't be persisted.
    pub file: NamedTempFile<F>,
}

impl<F> fmt::Debug for PersistError<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PersistError({:?})", self.error)
    }
}

impl<F> From<PersistError<F>> for io::Error {
    #[inline]
    fn from(error: PersistError<F>) -> io::Error {
        error.error
    }
}

impl<F> From<PersistError<F>> for NamedTempFile<F> {
    #[inline]
    fn from(error: PersistError<F>) -> NamedTempFile<F> {
        error.file
    }
}

impl<F> fmt::Display for PersistError<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to persist temporary file: {}", self.error)
    }
}

impl<F> error::Error for PersistError<F> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.error)
    }
}

impl NamedTempFile<File> {
    /// Create a new named temporary file.
    ///
    /// See [`Builder`] for more configuration.
    ///
    /// # Security
    ///
    /// This will create a temporary file in the default temporary file
    /// directory (platform dependent). This has security implications on many
    /// platforms so please read the security section of this type's
    /// documentation.
    ///
    /// Reasons to use this method:
    ///
    ///   1. The file has a short lifetime and your temporary file cleaner is
    ///      sane (doesn't delete recently accessed files).
    ///
    ///   2. You trust every user on your system (i.e. you are the only user).
    ///
    ///   3. You have disabled your system's temporary file cleaner or verified
    ///      that your system doesn't have a temporary file cleaner.
    ///
    /// Reasons not to use this method:
    ///
    ///   1. You'll fix it later. No you won't.
    ///
    ///   2. You don't care about the security of the temporary file. If none of
    ///      the "reasons to use this method" apply, referring to a temporary
    ///      file by name may allow an attacker to create/overwrite your
    ///      non-temporary files. There are exceptions but if you don't already
    ///      know them, don't use this method.
    ///
    /// # Errors
    ///
    /// If the file can not be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// Create a named temporary file and write some data to it:
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use tempfile::NamedTempFile;
    ///
    /// let mut file = NamedTempFile::new()?;
    ///
    /// writeln!(file, "Brian was here. Briefly.")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`Builder`]: struct.Builder.html
    pub fn new() -> io::Result<NamedTempFile> {
        Builder::new().tempfile()
    }

    /// Create a new named temporary file in the specified directory.
    ///
    /// This is equivalent to:
    ///
    /// ```ignore
    /// Builder::new().tempfile_in(dir)
    /// ```
    ///
    /// See [`NamedTempFile::new()`] for details.
    ///
    /// [`NamedTempFile::new()`]: #method.new
    pub fn new_in<P: AsRef<Path>>(dir: P) -> io::Result<NamedTempFile> {
        Builder::new().tempfile_in(dir)
    }

    /// Create a new named temporary file with the specified filename suffix.
    ///
    /// See [`NamedTempFile::new()`] for details.
    ///
    /// [`NamedTempFile::new()`]: #method.new
    pub fn with_suffix<S: AsRef<OsStr>>(suffix: S) -> io::Result<NamedTempFile> {
        Builder::new().suffix(&suffix).tempfile()
    }
    /// Create a new named temporary file with the specified filename suffix,
    /// in the specified directory.
    ///
    /// This is equivalent to:
    ///
    /// ```ignore
    /// Builder::new().suffix(&suffix).tempfile_in(directory)
    /// ```
    ///
    /// See [`NamedTempFile::new()`] for details.
    ///
    /// [`NamedTempFile::new()`]: #method.new
    pub fn with_suffix_in<S: AsRef<OsStr>, P: AsRef<Path>>(
        suffix: S,
        dir: P,
    ) -> io::Result<NamedTempFile> {
        Builder::new().suffix(&suffix).tempfile_in(dir)
    }

    /// Create a new named temporary file with the specified filename prefix.
    ///
    /// See [`NamedTempFile::new()`] for details.
    ///
    /// [`NamedTempFile::new()`]: #method.new
    pub fn with_prefix<S: AsRef<OsStr>>(prefix: S) -> io::Result<NamedTempFile> {
        Builder::new().prefix(&prefix).tempfile()
    }
    /// Create a new named temporary file with the specified filename prefix,
    /// in the specified directory.
    ///
    /// This is equivalent to:
    ///
    /// ```ignore
    /// Builder::new().prefix(&prefix).tempfile_in(directory)
    /// ```
    ///
    /// See [`NamedTempFile::new()`] for details.
    ///
    /// [`NamedTempFile::new()`]: #method.new
    pub fn with_prefix_in<S: AsRef<OsStr>, P: AsRef<Path>>(
        prefix: S,
        dir: P,
    ) -> io::Result<NamedTempFile> {
        Builder::new().prefix(&prefix).tempfile_in(dir)
    }
}

impl<F> NamedTempFile<F> {
    /// Get the temporary file's path.
    ///
    /// # Security
    ///
    /// Referring to a temporary file's path may not be secure in all cases.
    /// Please read the security section on the top level documentation of this
    /// type for details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tempfile::NamedTempFile;
    ///
    /// let file = NamedTempFile::new()?;
    ///
    /// println!("{:?}", file.path());
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Close and remove the temporary file.
    ///
    /// Use this if you want to detect errors in deleting the file.
    ///
    /// # Errors
    ///
    /// If the file cannot be deleted, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tempfile::NamedTempFile;
    ///
    /// let file = NamedTempFile::new()?;
    ///
    /// // By closing the `NamedTempFile` explicitly, we can check that it has
    /// // been deleted successfully. If we don't close it explicitly,
    /// // the file will still be deleted when `file` goes out
    /// // of scope, but we won't know whether deleting the file
    /// // succeeded.
    /// file.close()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn close(self) -> io::Result<()> {
        let NamedTempFile { path, .. } = self;
        path.close()
    }

    /// Persist the temporary file at the target path.
    ///
    /// If a file exists at the target path, persist will atomically replace it.
    /// If this method fails, it will return `self` in the resulting
    /// [`PersistError`].
    ///
    /// **Note:** Temporary files cannot be persisted across filesystems. Also
    /// neither the file contents nor the containing directory are
    /// synchronized, so the update may not yet have reached the disk when
    /// `persist` returns.
    ///
    /// # Security
    ///
    /// This method persists the temporary file using its path and may not be
    /// secure in all cases. Please read the security section on the top
    /// level documentation of this type for details.
    ///
    /// # Errors
    ///
    /// If the file cannot be moved to the new location, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use tempfile::NamedTempFile;
    ///
    /// let file = NamedTempFile::new()?;
    ///
    /// let mut persisted_file = file.persist("./saved_file.txt")?;
    /// writeln!(persisted_file, "Brian was here. Briefly.")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`PersistError`]: struct.PersistError.html
    pub fn persist<P: AsRef<Path>>(self, new_path: P) -> Result<F, PersistError<F>> {
        let NamedTempFile { path, file } = self;
        match path.persist(new_path) {
            Ok(_) => Ok(file),
            Err(err) => {
                let PathPersistError { error, path } = err;
                Err(PersistError {
                    file: NamedTempFile { path, file },
                    error,
                })
            }
        }
    }

    /// Persist the temporary file at the target path if and only if no file exists there.
    ///
    /// If a file exists at the target path, fail. If this method fails, it will
    /// return `self` in the resulting PersistError.
    ///
    /// **Note:** Temporary files cannot be persisted across filesystems.
    ///
    /// **Atomicity:** This method is not guaranteed to be atomic on all platforms, although it will
    /// generally be atomic on Windows and modern Linux filesystems. While it will never overwrite a
    /// file at the target path, it may leave the original link to the temporary file behind leaving
    /// you with two [hard links][hardlink] in your filesystem pointing at the same underlying file.
    /// This can happen if either (a) we lack permission to "unlink" the original filename; (b) this
    /// program crashes while persisting the temporary file; or (c) the filesystem is removed,
    /// unmounted, etc. while we're performing this operation.
    ///
    /// # Security
    ///
    /// This method persists the temporary file using its path and may not be
    /// secure in all cases. Please read the security section on the top
    /// level documentation of this type for details.
    ///
    /// # Errors
    ///
    /// If the file cannot be moved to the new location or a file already exists there,
    /// `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use tempfile::NamedTempFile;
    ///
    /// let file = NamedTempFile::new()?;
    ///
    /// let mut persisted_file = file.persist_noclobber("./saved_file.txt")?;
    /// writeln!(persisted_file, "Brian was here. Briefly.")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [hardlink]: https://en.wikipedia.org/wiki/Hard_link
    pub fn persist_noclobber<P: AsRef<Path>>(self, new_path: P) -> Result<F, PersistError<F>> {
        let NamedTempFile { path, file } = self;
        match path.persist_noclobber(new_path) {
            Ok(_) => Ok(file),
            Err(err) => {
                let PathPersistError { error, path } = err;
                Err(PersistError {
                    file: NamedTempFile { path, file },
                    error,
                })
            }
        }
    }

    /// Keep the temporary file from being deleted. This function will turn the
    /// temporary file into a non-temporary file without moving it.
    ///
    /// # Errors
    ///
    /// On some platforms (e.g., Windows), we need to mark the file as
    /// non-temporary. This operation could fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use tempfile::NamedTempFile;
    ///
    /// let mut file = NamedTempFile::new()?;
    /// writeln!(file, "Brian was here. Briefly.")?;
    ///
    /// let (file, path) = file.keep()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [`PathPersistError`]: struct.PathPersistError.html
    pub fn keep(self) -> Result<(F, PathBuf), PersistError<F>> {
        let (file, path) = (self.file, self.path);
        match path.keep() {
            Ok(path) => Ok((file, path)),
            Err(PathPersistError { error, path }) => Err(PersistError {
                file: NamedTempFile { path, file },
                error,
            }),
        }
    }

    /// Disable cleanup of the temporary file. If `disable_cleanup` is `true`, the temporary file
    /// will not be deleted when this `TempPath` is dropped. This method is equivalent to calling
    /// [`Builder::disable_cleanup`] when creating the original `NamedTempFile`, which see for
    /// relevant warnings.
    ///
    /// **NOTE:** this method is primarily useful for testing/debugging. If you want to simply turn
    /// a temporary file into a non-temporary file, prefer [`NamedTempFile::keep`].
    pub fn disable_cleanup(&mut self, disable_cleanup: bool) {
        self.path.disable_cleanup(disable_cleanup)
    }

    /// Get a reference to the underlying file.
    pub fn as_file(&self) -> &F {
        &self.file
    }

    /// Get a mutable reference to the underlying file.
    pub fn as_file_mut(&mut self) -> &mut F {
        &mut self.file
    }

    /// Turn this named temporary file into an "unnamed" temporary file as if you
    /// had constructed it with [`tempfile()`].
    ///
    /// The underlying file will be removed from the filesystem but the returned [`File`]
    /// can still be read/written.
    pub fn into_file(self) -> F {
        self.file
    }

    /// Closes the file, leaving only the temporary file path.
    ///
    /// This is useful when another process must be able to open the temporary
    /// file.
    pub fn into_temp_path(self) -> TempPath {
        self.path
    }

    /// Converts the named temporary file into its constituent parts.
    ///
    /// Note: When the path is dropped, the underlying file will be removed from the filesystem but
    /// the returned [`File`] can still be read/written.
    pub fn into_parts(self) -> (F, TempPath) {
        (self.file, self.path)
    }

    /// Creates a `NamedTempFile` from its constituent parts.
    ///
    /// This can be used with [`NamedTempFile::into_parts`] to reconstruct the
    /// `NamedTempFile`.
    pub fn from_parts(file: F, path: TempPath) -> Self {
        Self { file, path }
    }
}

impl NamedTempFile<File> {
    /// Securely reopen the temporary file.
    ///
    /// This function is useful when you need multiple independent handles to
    /// the same file. It's perfectly fine to drop the original `NamedTempFile`
    /// while holding on to `File`s returned by this function; the `File`s will
    /// remain usable. However, they may not be nameable.
    ///
    /// # Errors
    ///
    /// If the file cannot be reopened, `Err` is returned.
    ///
    /// # Security
    ///
    /// Unlike `File::open(my_temp_file.path())`, `NamedTempFile::reopen()`
    /// guarantees that the re-opened file is the _same_ file, even in the
    /// presence of pathological temporary file cleaners.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tempfile::NamedTempFile;
    ///
    /// let file = NamedTempFile::new()?;
    ///
    /// let another_handle = file.reopen()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn reopen(&self) -> io::Result<File> {
        imp::reopen(self.as_file(), NamedTempFile::path(self))
            .with_err_path(|| NamedTempFile::path(self))
    }
}

impl<F: Read> Read for NamedTempFile<F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.as_file_mut().read(buf).with_err_path(|| self.path())
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.as_file_mut()
            .read_vectored(bufs)
            .with_err_path(|| self.path())
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.as_file_mut()
            .read_to_end(buf)
            .with_err_path(|| self.path())
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.as_file_mut()
            .read_to_string(buf)
            .with_err_path(|| self.path())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.as_file_mut()
            .read_exact(buf)
            .with_err_path(|| self.path())
    }
}

impl Read for &NamedTempFile<File> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.as_file().read(buf).with_err_path(|| self.path())
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.as_file()
            .read_vectored(bufs)
            .with_err_path(|| self.path())
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.as_file()
            .read_to_end(buf)
            .with_err_path(|| self.path())
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.as_file()
            .read_to_string(buf)
            .with_err_path(|| self.path())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.as_file().read_exact(buf).with_err_path(|| self.path())
    }
}

impl<F: Write> Write for NamedTempFile<F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.as_file_mut().write(buf).with_err_path(|| self.path())
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.as_file_mut().flush().with_err_path(|| self.path())
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.as_file_mut()
            .write_vectored(bufs)
            .with_err_path(|| self.path())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.as_file_mut()
            .write_all(buf)
            .with_err_path(|| self.path())
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        self.as_file_mut()
            .write_fmt(fmt)
            .with_err_path(|| self.path())
    }
}

impl Write for &NamedTempFile<File> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.as_file().write(buf).with_err_path(|| self.path())
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.as_file().flush().with_err_path(|| self.path())
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.as_file()
            .write_vectored(bufs)
            .with_err_path(|| self.path())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.as_file().write_all(buf).with_err_path(|| self.path())
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        self.as_file().write_fmt(fmt).with_err_path(|| self.path())
    }
}

impl<F: Seek> Seek for NamedTempFile<F> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.as_file_mut().seek(pos).with_err_path(|| self.path())
    }
}

impl Seek for &NamedTempFile<File> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.as_file().seek(pos).with_err_path(|| self.path())
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl<F: AsFd> AsFd for NamedTempFile<F> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_file().as_fd()
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl<F: AsRawFd> AsRawFd for NamedTempFile<F> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_file().as_raw_fd()
    }
}

#[cfg(windows)]
impl<F: AsHandle> AsHandle for NamedTempFile<F> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.as_file().as_handle()
    }
}

#[cfg(windows)]
impl<F: AsRawHandle> AsRawHandle for NamedTempFile<F> {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.as_file().as_raw_handle()
    }
}

pub(crate) fn create_named(
    path: PathBuf,
    open_options: &mut OpenOptions,
    permissions: Option<&std::fs::Permissions>,
    keep: bool,
) -> io::Result<NamedTempFile> {
    imp::create_named(&path, open_options, permissions)
        .with_err_path(|| path.clone())
        .map(|file| NamedTempFile {
            path: TempPath {
                path: path.into_boxed_path(),
                disable_cleanup: keep,
            },
            file,
        })
}
