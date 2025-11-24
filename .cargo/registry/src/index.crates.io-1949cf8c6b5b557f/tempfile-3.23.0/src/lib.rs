//! This is a library for creating temporary files and directories that are automatically deleted
//! when no longer referenced (i.e., on drop).
//!
//! - Use [`tempfile()`] when you need a real [`std::fs::File`] but don't need to refer to it
//!   by-path.
//! - Use [`NamedTempFile::new()`] when you need a _named_ temporary file that can be refered to its
//!   path.
//! - Use [`tempdir()`] when you need a temporary directory that will be recursively deleted on drop.
//! - Use [`spooled_tempfile()`] when you need an in-memory buffer that will ultimately be backed by
//!   a temporary file if it gets too large.
//!
//! # Design
//!
//! This crate provides several approaches to creating temporary files and directories.
//! [`tempfile()`] relies on the OS to remove the temporary file once the last handle is closed.
//! [`TempDir`] and [`NamedTempFile`] both rely on Rust destructors for cleanup.
//!
//! ## Resource Leaking
//!
//! `tempfile` will (almost) never fail to cleanup temporary resources. However `TempDir` and
//! `NamedTempFile` will fail if their destructors don't run. This is because `tempfile` relies on
//! the OS to cleanup the underlying file, while `TempDir` and `NamedTempFile` rely on rust
//! destructors to do so. Destructors may fail to run if the process exits through an unhandled
//! signal interrupt (like `SIGINT`), or if the instance is declared statically (like with
//! [`lazy_static`]), among other possible reasons.
//!
//! ## Unexpected File Deletion
//!
//! Most operating systems periodically clean up temporary files that haven't been accessed recently
//! (often on the order of multiple days). This issue does not affect unnamed temporary files but
//! can invalidate the paths associated with named temporary files on Unix-like systems because the
//! temporary file can be unlinked from the filesystem while still open and in-use. See the
//! [temporary file cleaner](#temporary-file-cleaners) section for more security implications.
//!
//! ## Security
//!
//! This section discusses security issues relevant to Unix-like operating systems that use shared
//! temporary directories by default. Importantly, it's not relevant for Windows or macOS as both
//! operating systems use private per-user temporary directories by default.
//!
//! Applications can mitigate the issues described below by using [`env::override_temp_dir`] to
//! change the default temporary directory but should do so if and only if default the temporary
//! directory ([`env::temp_dir`]) is unsuitable (is world readable, world writable, managed by a
//! temporary file cleaner, etc.).
//!
//! ### Temporary File Cleaners
//!
//! In the presence of pathological temporary file cleaner, relying on file paths is unsafe because
//! a temporary file cleaner could delete the temporary file which an attacker could then replace.
//!
//! This isn't an issue for [`tempfile`] as it doesn't rely on file paths. However, [`NamedTempFile`]
//! and temporary directories _do_ rely on file paths for _some_ operations. See the security
//! documentation on the [`NamedTempFile`] and the [`TempDir`] types for more information.
//!
//! Mitigation:
//!
//! - This is rarely an issue for short-lived files as temporary file cleaners usually only remove
//!   temporary files that haven't been modified or accessed within many (10-30) days.
//! - Very long lived temporary files should be placed in directories not managed by temporary file
//!   cleaners.
//!
//! ### Access Permissions
//!
//! Temporary _files_ created with this library are private by default on all operating systems.
//! However, temporary _directories_ are created with the default permissions and will therefore be
//! world-readable by default unless the user has changed their umask and/or default temporary
//! directory.
//!
//! ### Denial of Service
//!
//! If the file-name randomness ([`Builder::rand_bytes`]) is too small and/or this crate is built
//! without the `getrandom` feature, it may be possible for an attacker to predict the random file
//! names chosen by this library, preventing temporary file creation by creating temporary files
//! with these predicted file names. By default, this library mitigates this denial of service
//! attack by:
//!
//! 1. Defaulting to 6 random characters per temporary file forcing an attacker to create billions
//!    of files before random collisions are expected (at which point you probably have larger
//!    problems).
//! 2. Re-seeding the random filename generator from system randomness after 3 failed attempts to
//!    create temporary a file (when the `getrandom` feature is enabled as it is by default on all
//!    major platforms).
//!
//! ## Early drop pitfall
//!
//! Because `TempDir` and `NamedTempFile` rely on their destructors for cleanup, this can lead
//! to an unexpected early removal of the directory/file, usually when working with APIs which are
//! generic over `AsRef<Path>`. Consider the following example:
//!
//! ```no_run
//! use tempfile::tempdir;
//! use std::process::Command;
//!
//! // Create a directory inside of `env::temp_dir()`.
//! let temp_dir = tempdir()?;
//!
//! // Spawn the `touch` command inside the temporary directory and collect the exit status
//! // Note that `temp_dir` is **not** moved into `current_dir`, but passed as a reference
//! let exit_status = Command::new("touch").arg("tmp").current_dir(&temp_dir).status()?;
//! assert!(exit_status.success());
//!
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! This works because a reference to `temp_dir` is passed to `current_dir`, resulting in the
//! destructor of `temp_dir` being run after the `Command` has finished execution. Moving the
//! `TempDir` into the `current_dir` call would result in the `TempDir` being converted into
//! an internal representation, with the original value being dropped and the directory thus
//! being deleted, before the command can be executed.
//!
//! The `touch` command would fail with an `No such file or directory` error.
//!
//! ## Examples
//!
//! Create a temporary file and write some data into it:
//!
//! ```
//! use tempfile::tempfile;
//! use std::io::Write;
//!
//! // Create a file inside of `env::temp_dir()`.
//! let mut file = tempfile()?;
//!
//! writeln!(file, "Brian was here. Briefly.")?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Create a named temporary file and open an independent file handle:
//!
//! ```
//! use tempfile::NamedTempFile;
//! use std::io::{Write, Read};
//!
//! let text = "Brian was here. Briefly.";
//!
//! // Create a file inside of `env::temp_dir()`.
//! let mut file1 = NamedTempFile::new()?;
//!
//! // Re-open it.
//! let mut file2 = file1.reopen()?;
//!
//! // Write some test data to the first handle.
//! file1.write_all(text.as_bytes())?;
//!
//! // Read the test data using the second handle.
//! let mut buf = String::new();
//! file2.read_to_string(&mut buf)?;
//! assert_eq!(buf, text);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Create a temporary directory and add a file to it:
//!
//! ```
//! use tempfile::tempdir;
//! use std::fs::File;
//! use std::io::Write;
//!
//! // Create a directory inside of `env::temp_dir()`.
//! let dir = tempdir()?;
//!
//! let file_path = dir.path().join("my-temporary-note.txt");
//! let mut file = File::create(file_path)?;
//! writeln!(file, "Brian was here. Briefly.")?;
//!
//! // By closing the `TempDir` explicitly, we can check that it has
//! // been deleted successfully. If we don't close it explicitly,
//! // the directory will still be deleted when `dir` goes out
//! // of scope, but we won't know whether deleting the directory
//! // succeeded.
//! drop(file);
//! dir.close()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! [`tempfile()`]: fn.tempfile.html
//! [`tempdir()`]: fn.tempdir.html
//! [`TempDir`]: struct.TempDir.html
//! [`NamedTempFile`]: struct.NamedTempFile.html
//! [`lazy_static`]: https://github.com/rust-lang-nursery/lazy-static.rs/issues/62

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://docs.rs/tempfile/latest"
)]
#![cfg_attr(test, deny(warnings))]
#![deny(rust_2018_idioms)]
#![allow(clippy::redundant_field_names)]
// wasip2 conditionally gates stdlib APIs.
// https://github.com/rust-lang/rust/issues/130323
#![cfg_attr(
    all(feature = "nightly", target_os = "wasi", target_env = "p2"),
    feature(wasip2)
)]
#![cfg_attr(all(feature = "nightly", target_os = "wasi"), feature(wasi_ext))]

#[cfg(doctest)]
doc_comment::doctest!("../README.md");

const NUM_RETRIES: u32 = 65536;
const NUM_RAND_CHARS: usize = 6;

use std::ffi::OsStr;
use std::fs::{OpenOptions, Permissions};
use std::io;
use std::path::Path;

mod dir;
mod error;
mod file;
mod spooled;
mod util;

pub mod env;

pub use crate::dir::{tempdir, tempdir_in, TempDir};
pub use crate::file::{
    tempfile, tempfile_in, NamedTempFile, PathPersistError, PersistError, TempPath,
};
pub use crate::spooled::{spooled_tempfile, spooled_tempfile_in, SpooledData, SpooledTempFile};

/// Create a new temporary file or directory with custom options.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Builder<'a, 'b> {
    random_len: usize,
    prefix: &'a OsStr,
    suffix: &'b OsStr,
    append: bool,
    permissions: Option<Permissions>,
    disable_cleanup: bool,
}

impl Default for Builder<'_, '_> {
    fn default() -> Self {
        Builder {
            random_len: crate::NUM_RAND_CHARS,
            prefix: OsStr::new(".tmp"),
            suffix: OsStr::new(""),
            append: false,
            permissions: None,
            disable_cleanup: false,
        }
    }
}

impl<'a, 'b> Builder<'a, 'b> {
    /// Create a new `Builder`.
    ///
    /// # Examples
    ///
    /// Create a named temporary file and write some data into it:
    ///
    /// ```
    /// use std::ffi::OsStr;
    /// use tempfile::Builder;
    ///
    /// let named_tempfile = Builder::new()
    ///     .prefix("my-temporary-note")
    ///     .suffix(".txt")
    ///     .rand_bytes(5)
    ///     .tempfile()?;
    ///
    /// let name = named_tempfile
    ///     .path()
    ///     .file_name().and_then(OsStr::to_str);
    ///
    /// if let Some(name) = name {
    ///     assert!(name.starts_with("my-temporary-note"));
    ///     assert!(name.ends_with(".txt"));
    ///     assert_eq!(name.len(), "my-temporary-note.txt".len() + 5);
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// Create a temporary directory and add a file to it:
    ///
    /// ```
    /// use std::io::Write;
    /// use std::fs::File;
    /// use std::ffi::OsStr;
    /// use tempfile::Builder;
    ///
    /// let dir = Builder::new()
    ///     .prefix("my-temporary-dir")
    ///     .rand_bytes(5)
    ///     .tempdir()?;
    ///
    /// let file_path = dir.path().join("my-temporary-note.txt");
    /// let mut file = File::create(file_path)?;
    /// writeln!(file, "Brian was here. Briefly.")?;
    ///
    /// // By closing the `TempDir` explicitly, we can check that it has
    /// // been deleted successfully. If we don't close it explicitly,
    /// // the directory will still be deleted when `dir` goes out
    /// // of scope, but we won't know whether deleting the directory
    /// // succeeded.
    /// drop(file);
    /// dir.close()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// Create a temporary directory with a chosen prefix under a chosen folder:
    ///
    /// ```no_run
    /// use tempfile::Builder;
    ///
    /// let dir = Builder::new()
    ///     .prefix("my-temporary-dir")
    ///     .tempdir_in("folder-with-tempdirs")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom filename prefix.
    ///
    /// Path separators are legal but not advisable.
    /// Default: `.tmp`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let named_tempfile = Builder::new()
    ///     .prefix("my-temporary-note")
    ///     .tempfile()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn prefix<S: AsRef<OsStr> + ?Sized>(&mut self, prefix: &'a S) -> &mut Self {
        self.prefix = prefix.as_ref();
        self
    }

    /// Set a custom filename suffix.
    ///
    /// Path separators are legal but not advisable.
    /// Default: empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let named_tempfile = Builder::new()
    ///     .suffix(".txt")
    ///     .tempfile()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn suffix<S: AsRef<OsStr> + ?Sized>(&mut self, suffix: &'b S) -> &mut Self {
        self.suffix = suffix.as_ref();
        self
    }

    /// Set the number of random bytes.
    ///
    /// Default: `6`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let named_tempfile = Builder::new()
    ///     .rand_bytes(5)
    ///     .tempfile()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn rand_bytes(&mut self, rand: usize) -> &mut Self {
        self.random_len = rand;
        self
    }

    /// Configure the file to be opened in append-only mode.
    ///
    /// Default: `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let named_tempfile = Builder::new()
    ///     .append(true)
    ///     .tempfile()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    /// Set the permissions for the new temporary file/directory.
    ///
    /// # Platform Notes
    ///
    /// ## Windows
    ///
    /// This setting is only fully-supported on unix-like platforms. On Windows, if this method is
    /// called with a [`Permissions`] object where `permissions.readonly` returns true, creating
    /// temporary files and directories will fail with an error.
    ///
    /// ## Unix
    ///
    /// On unix-like systems, the actual permission bits set on the tempfile or tempdir will be
    /// affected by the `umask` applied by the underlying syscall. The actual permission bits are
    /// calculated via `permissions & !umask`. In other words, depending on your umask, the
    /// permissions of the created file may be more restrictive (but never more permissive) than the
    /// ones you specified.
    ///
    /// Permissions default to `0o600` for tempfiles and `0o777` for tempdirs. Note, this doesn't
    /// include effects of the current `umask`. For example, combined with the standard umask
    /// `0o022`, the defaults yield `0o600` for tempfiles and `0o755` for tempdirs.
    ///
    /// ## WASI
    ///
    /// While custom permissions are allowed on WASI, they will be ignored as the platform has no
    /// concept of permissions or file modes (or multiple users for that matter).
    ///
    /// # Examples
    ///
    /// Create a named temporary file that is world-readable.
    ///
    /// ```
    /// # #[cfg(unix)]
    /// # {
    /// use tempfile::Builder;
    /// use std::os::unix::fs::PermissionsExt;
    ///
    /// let all_read_write = std::fs::Permissions::from_mode(0o666);
    /// let tempfile = Builder::new().permissions(all_read_write).tempfile()?;
    ///
    /// // Check that this worked and that the file is world-readable.
    /// //
    /// // NOTE: the file likely won't actually be created with 0o666 permissions because it's
    /// // restricted by the user's umask.
    /// //
    /// // NOTE: This test will fail if the user's umask is, e.g., 0o066.
    /// let actual_permissions = tempfile.path().metadata()?.permissions();
    /// assert_eq!(actual_permissions.mode() & 0o044, 0o044);
    /// # }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// Create a named temporary directory that is restricted to the owner.
    ///
    /// ```
    /// # #[cfg(unix)]
    /// # {
    /// use tempfile::Builder;
    /// use std::os::unix::fs::PermissionsExt;
    ///
    /// let owner_rwx = std::fs::Permissions::from_mode(0o700);
    /// let tempdir = Builder::new().permissions(owner_rwx).tempdir()?;
    /// let actual_permissions = tempdir.path().metadata()?.permissions();
    /// assert_eq!(
    ///     actual_permissions.mode() & !0o170000,
    ///     0o700,
    ///     "we get the narrow permissions we asked for"
    /// );
    /// # }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn permissions(&mut self, permissions: Permissions) -> &mut Self {
        self.permissions = Some(permissions);
        self
    }

    /// Disable cleanup of the file/folder to even when the [`NamedTempFile`]/[`TempDir`] goes out
    /// of scope. Prefer [`NamedTempFile::keep`] and [`TempDir::keep`] where possible;
    /// `disable_cleanup` is provided for testing & debugging.
    ///
    /// By default, the file/folder is automatically cleaned up in the destructor of
    /// [`NamedTempFile`]/[`TempDir`]. When `disable_cleanup` is set to `true`, this behavior is
    /// suppressed. If you wish to disable cleanup after creating a temporary file/directory, call
    /// [`NamedTempFile::disable_cleanup`] or [`TempDir::disable_cleanup`].
    ///
    /// # Warnings
    ///
    /// On some platforms (for now, only Windows), temporary files are marked with a special
    /// "temporary file" (`FILE_ATTRIBUTE_TEMPORARY`) attribute. Disabling cleanup _will not_ unset
    /// this attribute while calling [`NamedTempFile::keep`] will.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let named_tempfile = Builder::new()
    ///     .disable_cleanup(true)
    ///     .tempfile()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn disable_cleanup(&mut self, disable_cleanup: bool) -> &mut Self {
        self.disable_cleanup = disable_cleanup;
        self
    }

    /// Deprecated alias for [`Builder::disable_cleanup`].
    #[deprecated = "Use Builder::disable_cleanup"]
    pub fn keep(&mut self, keep: bool) -> &mut Self {
        self.disable_cleanup(keep)
    }

    /// Create the named temporary file.
    ///
    /// # Security
    ///
    /// See [the security][security] docs on `NamedTempFile`.
    ///
    /// # Resource leaking
    ///
    /// See [the resource leaking][resource-leaking] docs on `NamedTempFile`.
    ///
    /// # Errors
    ///
    /// If the file cannot be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let tempfile = Builder::new().tempfile()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [security]: struct.NamedTempFile.html#security
    /// [resource-leaking]: struct.NamedTempFile.html#resource-leaking
    pub fn tempfile(&self) -> io::Result<NamedTempFile> {
        self.tempfile_in(env::temp_dir())
    }

    /// Create the named temporary file in the specified directory.
    ///
    /// # Security
    ///
    /// See [the security][security] docs on `NamedTempFile`.
    ///
    /// # Resource leaking
    ///
    /// See [the resource leaking][resource-leaking] docs on `NamedTempFile`.
    ///
    /// # Errors
    ///
    /// If the file cannot be created, `Err` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use tempfile::Builder;
    ///
    /// let tempfile = Builder::new().tempfile_in("./")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [security]: struct.NamedTempFile.html#security
    /// [resource-leaking]: struct.NamedTempFile.html#resource-leaking
    pub fn tempfile_in<P: AsRef<Path>>(&self, dir: P) -> io::Result<NamedTempFile> {
        util::create_helper(
            dir.as_ref(),
            self.prefix,
            self.suffix,
            self.random_len,
            |path| {
                file::create_named(
                    path,
                    OpenOptions::new().append(self.append),
                    self.permissions.as_ref(),
                    self.disable_cleanup,
                )
            },
        )
    }

    /// Attempts to make a temporary directory inside of [`env::temp_dir()`] whose
    /// name will have the prefix, `prefix`. The directory and
    /// everything inside it will be automatically deleted once the
    /// returned `TempDir` is destroyed.
    ///
    /// # Resource leaking
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
    /// use tempfile::Builder;
    ///
    /// let tmp_dir = Builder::new().tempdir()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [resource-leaking]: struct.TempDir.html#resource-leaking
    pub fn tempdir(&self) -> io::Result<TempDir> {
        self.tempdir_in(env::temp_dir())
    }

    /// Attempts to make a temporary directory inside of `dir`.
    /// The directory and everything inside it will be automatically
    /// deleted once the returned `TempDir` is destroyed.
    ///
    /// # Resource leaking
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
    /// use tempfile::Builder;
    ///
    /// let tmp_dir = Builder::new().tempdir_in("./")?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [resource-leaking]: struct.TempDir.html#resource-leaking
    pub fn tempdir_in<P: AsRef<Path>>(&self, dir: P) -> io::Result<TempDir> {
        util::create_helper(
            dir.as_ref(),
            self.prefix,
            self.suffix,
            self.random_len,
            |path| dir::create(path, self.permissions.as_ref(), self.disable_cleanup),
        )
    }

    /// Attempts to create a temporary file (or file-like object) using the
    /// provided closure. The closure is passed a temporary file path and
    /// returns an [`std::io::Result`]. The path provided to the closure will be
    /// inside of [`env::temp_dir()`]. Use [`Builder::make_in`] to provide
    /// a custom temporary directory. If the closure returns one of the
    /// following errors, then another randomized file path is tried:
    ///  - [`std::io::ErrorKind::AlreadyExists`]
    ///  - [`std::io::ErrorKind::AddrInUse`]
    ///
    /// This can be helpful for taking full control over the file creation, but
    /// leaving the temporary file path construction up to the library. This
    /// also enables creating a temporary UNIX domain socket, since it is not
    /// possible to bind to a socket that already exists.
    ///
    /// Note that [`Builder::append`] is ignored when using [`Builder::make`].
    ///
    /// # Security
    ///
    /// This has the same [security implications][security] as
    /// [`NamedTempFile`], but with additional caveats. Specifically, it is up
    /// to the closure to ensure that the file does not exist and that such a
    /// check is *atomic*. Otherwise, a [time-of-check to time-of-use
    /// bug][TOCTOU] could be introduced.
    ///
    /// For example, the following is **not** secure:
    ///
    /// ```
    /// use std::fs::File;
    /// use tempfile::Builder;
    ///
    /// // This is NOT secure!
    /// let tempfile = Builder::new().make(|path| {
    ///     if path.is_file() {
    ///         return Err(std::io::ErrorKind::AlreadyExists.into());
    ///     }
    ///
    ///     // Between the check above and the usage below, an attacker could
    ///     // have replaced `path` with another file, which would get truncated
    ///     // by `File::create`.
    ///
    ///     File::create(path)
    /// })?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// Note that simply using [`std::fs::File::create`] alone is not correct
    /// because it does not fail if the file already exists:
    ///
    /// ```
    /// use tempfile::Builder;
    /// use std::fs::File;
    ///
    /// // This could overwrite an existing file!
    /// let tempfile = Builder::new().make(|path| File::create(path))?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    /// For creating regular temporary files, use [`Builder::tempfile`] instead
    /// to avoid these problems. This function is meant to enable more exotic
    /// use-cases.
    ///
    /// # Resource leaking
    ///
    /// See [the resource leaking][resource-leaking] docs on `NamedTempFile`.
    ///
    /// # Errors
    ///
    /// If the closure returns any error besides
    /// [`std::io::ErrorKind::AlreadyExists`] or
    /// [`std::io::ErrorKind::AddrInUse`], then `Err` is returned.
    ///
    /// # Examples
    /// ```
    /// # #[cfg(unix)]
    /// # {
    /// use std::os::unix::net::UnixListener;
    /// use tempfile::Builder;
    ///
    /// let tempsock = Builder::new().make(|path| UnixListener::bind(path))?;
    /// # }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use
    /// [security]: struct.NamedTempFile.html#security
    /// [resource-leaking]: struct.NamedTempFile.html#resource-leaking
    pub fn make<F, R>(&self, f: F) -> io::Result<NamedTempFile<R>>
    where
        F: FnMut(&Path) -> io::Result<R>,
    {
        self.make_in(env::temp_dir(), f)
    }

    /// This is the same as [`Builder::make`], except `dir` is used as the base
    /// directory for the temporary file path.
    ///
    /// See [`Builder::make`] for more details and security implications.
    ///
    /// # Examples
    /// ```
    /// # #[cfg(unix)]
    /// # {
    /// use tempfile::Builder;
    /// use std::os::unix::net::UnixListener;
    ///
    /// let tempsock = Builder::new().make_in("./", |path| UnixListener::bind(path))?;
    /// # }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn make_in<F, R, P>(&self, dir: P, mut f: F) -> io::Result<NamedTempFile<R>>
    where
        F: FnMut(&Path) -> io::Result<R>,
        P: AsRef<Path>,
    {
        util::create_helper(
            dir.as_ref(),
            self.prefix,
            self.suffix,
            self.random_len,
            move |path| {
                Ok(NamedTempFile::from_parts(
                    f(&path)?,
                    TempPath::new(path, self.disable_cleanup),
                ))
            },
        )
    }
}
