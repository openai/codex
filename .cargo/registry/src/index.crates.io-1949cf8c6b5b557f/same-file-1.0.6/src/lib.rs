/*!
This crate provides a safe and simple **cross platform** way to determine
whether two file paths refer to the same file or directory.

Most uses of this crate should be limited to the top-level [`is_same_file`]
function, which takes two file paths and returns true if they refer to the
same file or directory:

```rust,no_run
# use std::error::Error;
use same_file::is_same_file;

# fn try_main() -> Result<(), Box<Error>> {
assert!(is_same_file("/bin/sh", "/usr/bin/sh")?);
#    Ok(())
# }
#
# fn main() {
#    try_main().unwrap();
# }
```

Additionally, this crate provides a [`Handle`] type that permits a more efficient
equality check depending on your access pattern. For example, if one wanted to
check whether any path in a list of paths corresponded to the process' stdout
handle, then one could build a handle once for stdout. The equality check for
each file in the list then only requires one stat call instead of two. The code
might look like this:

```rust,no_run
# use std::error::Error;
use same_file::Handle;

# fn try_main() -> Result<(), Box<Error>> {
let candidates = &[
    "examples/is_same_file.rs",
    "examples/is_stderr.rs",
    "examples/stderr",
];
let stdout_handle = Handle::stdout()?;
for candidate in candidates {
    let handle = Handle::from_path(candidate)?;
    if stdout_handle == handle {
        println!("{:?} is stdout!", candidate);
    } else {
        println!("{:?} is NOT stdout!", candidate);
    }
}
#    Ok(())
# }
#
# fn main() {
#     try_main().unwrap();
# }
```

See [`examples/is_stderr.rs`] for a runnable example and compare the output of:

- `cargo run --example is_stderr 2> examples/stderr` and
- `cargo run --example is_stderr`.

[`is_same_file`]: fn.is_same_file.html
[`Handle`]: struct.Handle.html
[`examples/is_stderr.rs`]: https://github.com/BurntSushi/same-file/blob/master/examples/is_same_file.rs

*/

#![allow(bare_trait_objects, unknown_lints)]
#![deny(missing_docs)]

#[cfg(test)]
doc_comment::doctest!("../README.md");

use std::fs::File;
use std::io;
use std::path::Path;

#[cfg(any(target_os = "redox", unix))]
use crate::unix as imp;
#[cfg(not(any(target_os = "redox", unix, windows)))]
use unknown as imp;
#[cfg(windows)]
use win as imp;

#[cfg(any(target_os = "redox", unix))]
mod unix;
#[cfg(not(any(target_os = "redox", unix, windows)))]
mod unknown;
#[cfg(windows)]
mod win;

/// A handle to a file that can be tested for equality with other handles.
///
/// If two files are the same, then any two handles of those files will compare
/// equal. If two files are not the same, then any two handles of those files
/// will compare not-equal.
///
/// A handle consumes an open file resource as long as it exists.
///
/// Equality is determined by comparing inode numbers on Unix and a combination
/// of identifier, volume serial, and file size on Windows. Note that it's
/// possible for comparing two handles to produce a false positive on some
/// platforms. Namely, two handles can compare equal even if the two handles
/// *don't* point to the same file. Check the [source] for specific
/// implementation details.
///
/// [source]: https://github.com/BurntSushi/same-file/tree/master/src
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Handle(imp::Handle);

impl Handle {
    /// Construct a handle from a path.
    ///
    /// Note that the underlying [`File`] is opened in read-only mode on all
    /// platforms.
    ///
    /// [`File`]: https://doc.rust-lang.org/std/fs/struct.File.html
    ///
    /// # Errors
    /// This method will return an [`io::Error`] if the path cannot
    /// be opened, or the file's metadata cannot be obtained.
    /// The most common reasons for this are: the path does not
    /// exist, or there were not enough permissions.
    ///
    /// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
    ///
    /// # Examples
    /// Check that two paths are not the same file:
    ///
    /// ```rust,no_run
    /// # use std::error::Error;
    /// use same_file::Handle;
    ///
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let source = Handle::from_path("./source")?;
    /// let target = Handle::from_path("./target")?;
    /// assert_ne!(source, target, "The files are the same.");
    /// # Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn from_path<P: AsRef<Path>>(p: P) -> io::Result<Handle> {
        imp::Handle::from_path(p).map(Handle)
    }

    /// Construct a handle from a file.
    ///
    /// # Errors
    /// This method will return an [`io::Error`] if the metadata for
    /// the given [`File`] cannot be obtained.
    ///
    /// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
    /// [`File`]: https://doc.rust-lang.org/std/fs/struct.File.html
    ///
    /// # Examples
    /// Check that two files are not in fact the same file:
    ///
    /// ```rust,no_run
    /// # use std::error::Error;
    /// # use std::fs::File;
    /// use same_file::Handle;
    ///
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let source = File::open("./source")?;
    /// let target = File::open("./target")?;
    ///
    /// assert_ne!(
    ///     Handle::from_file(source)?,
    ///     Handle::from_file(target)?,
    ///     "The files are the same."
    /// );
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn from_file(file: File) -> io::Result<Handle> {
        imp::Handle::from_file(file).map(Handle)
    }

    /// Construct a handle from stdin.
    ///
    /// # Errors
    /// This method will return an [`io::Error`] if stdin cannot
    /// be opened due to any I/O-related reason.
    ///
    /// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use same_file::Handle;
    ///
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let stdin = Handle::stdin()?;
    /// let stdout = Handle::stdout()?;
    /// let stderr = Handle::stderr()?;
    ///
    /// if stdin == stdout {
    ///     println!("stdin == stdout");
    /// }
    /// if stdin == stderr {
    ///     println!("stdin == stderr");
    /// }
    /// if stdout == stderr {
    ///     println!("stdout == stderr");
    /// }
    /// #
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// The output differs depending on the platform.
    ///
    /// On Linux:
    ///
    /// ```text
    /// $ ./example
    /// stdin == stdout
    /// stdin == stderr
    /// stdout == stderr
    /// $ ./example > result
    /// $ cat result
    /// stdin == stderr
    /// $ ./example > result 2>&1
    /// $ cat result
    /// stdout == stderr
    /// ```
    ///
    /// Windows:
    ///
    /// ```text
    /// > example
    /// > example > result 2>&1
    /// > type result
    /// stdout == stderr
    /// ```
    pub fn stdin() -> io::Result<Handle> {
        imp::Handle::stdin().map(Handle)
    }

    /// Construct a handle from stdout.
    ///
    /// # Errors
    /// This method will return an [`io::Error`] if stdout cannot
    /// be opened due to any I/O-related reason.
    ///
    /// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
    ///
    /// # Examples
    /// See the example for [`stdin()`].
    ///
    /// [`stdin()`]: #method.stdin
    pub fn stdout() -> io::Result<Handle> {
        imp::Handle::stdout().map(Handle)
    }

    /// Construct a handle from stderr.
    ///
    /// # Errors
    /// This method will return an [`io::Error`] if stderr cannot
    /// be opened due to any I/O-related reason.
    ///
    /// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
    ///
    /// # Examples
    /// See the example for [`stdin()`].
    ///
    /// [`stdin()`]: #method.stdin
    pub fn stderr() -> io::Result<Handle> {
        imp::Handle::stderr().map(Handle)
    }

    /// Return a reference to the underlying file.
    ///
    /// # Examples
    /// Ensure that the target file is not the same as the source one,
    /// and copy the data to it:
    ///
    /// ```rust,no_run
    /// # use std::error::Error;
    /// use std::io::prelude::*;
    /// use std::io::Write;
    /// use std::fs::File;
    /// use same_file::Handle;
    ///
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let source = File::open("source")?;
    /// let target = File::create("target")?;
    ///
    /// let source_handle = Handle::from_file(source)?;
    /// let mut target_handle = Handle::from_file(target)?;
    /// assert_ne!(source_handle, target_handle, "The files are the same.");
    ///
    /// let mut source = source_handle.as_file();
    /// let target = target_handle.as_file_mut();
    ///
    /// let mut buffer = Vec::new();
    /// // data copy is simplified for the purposes of the example
    /// source.read_to_end(&mut buffer)?;
    /// target.write_all(&buffer)?;
    /// #
    /// #    Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #    try_main().unwrap();
    /// # }
    /// ```
    pub fn as_file(&self) -> &File {
        self.0.as_file()
    }

    /// Return a mutable reference to the underlying file.
    ///
    /// # Examples
    /// See the example for [`as_file()`].
    ///
    /// [`as_file()`]: #method.as_file
    pub fn as_file_mut(&mut self) -> &mut File {
        self.0.as_file_mut()
    }

    /// Return the underlying device number of this handle.
    ///
    /// Note that this only works on unix platforms.
    #[cfg(any(target_os = "redox", unix))]
    pub fn dev(&self) -> u64 {
        self.0.dev()
    }

    /// Return the underlying inode number of this handle.
    ///
    /// Note that this only works on unix platforms.
    #[cfg(any(target_os = "redox", unix))]
    pub fn ino(&self) -> u64 {
        self.0.ino()
    }
}

/// Returns true if the two file paths may correspond to the same file.
///
/// Note that it's possible for this to produce a false positive on some
/// platforms. Namely, this can return true even if the two file paths *don't*
/// resolve to the same file.
/// # Errors
/// This function will return an [`io::Error`] if any of the two paths cannot
/// be opened. The most common reasons for this are: the path does not exist,
/// or there were not enough permissions.
///
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
///
/// # Example
///
/// ```rust,no_run
/// use same_file::is_same_file;
///
/// assert!(is_same_file("./foo", "././foo").unwrap_or(false));
/// ```
pub fn is_same_file<P, Q>(path1: P, path2: Q) -> io::Result<bool>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    Ok(Handle::from_path(path1)? == Handle::from_path(path2)?)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::error;
    use std::fs::{self, File};
    use std::io;
    use std::path::{Path, PathBuf};
    use std::result;

    use super::is_same_file;

    type Result<T> = result::Result<T, Box<error::Error + Send + Sync>>;

    /// Create an error from a format!-like syntax.
    macro_rules! err {
        ($($tt:tt)*) => {
            Box::<error::Error + Send + Sync>::from(format!($($tt)*))
        }
    }

    /// A simple wrapper for creating a temporary directory that is
    /// automatically deleted when it's dropped.
    ///
    /// We use this in lieu of tempfile because tempfile brings in too many
    /// dependencies.
    #[derive(Debug)]
    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    impl TempDir {
        /// Create a new empty temporary directory under the system's
        /// configured temporary directory.
        fn new() -> Result<TempDir> {
            #![allow(deprecated)]

            use std::sync::atomic::{
                AtomicUsize, Ordering, ATOMIC_USIZE_INIT,
            };

            static TRIES: usize = 100;
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
        fn path(&self) -> &Path {
            &self.0
        }
    }

    fn tmpdir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[cfg(unix)]
    pub fn soft_link_dir<P: AsRef<Path>, Q: AsRef<Path>>(
        src: P,
        dst: Q,
    ) -> io::Result<()> {
        use std::os::unix::fs::symlink;
        symlink(src, dst)
    }

    #[cfg(unix)]
    pub fn soft_link_file<P: AsRef<Path>, Q: AsRef<Path>>(
        src: P,
        dst: Q,
    ) -> io::Result<()> {
        soft_link_dir(src, dst)
    }

    #[cfg(windows)]
    pub fn soft_link_dir<P: AsRef<Path>, Q: AsRef<Path>>(
        src: P,
        dst: Q,
    ) -> io::Result<()> {
        use std::os::windows::fs::symlink_dir;
        symlink_dir(src, dst)
    }

    #[cfg(windows)]
    pub fn soft_link_file<P: AsRef<Path>, Q: AsRef<Path>>(
        src: P,
        dst: Q,
    ) -> io::Result<()> {
        use std::os::windows::fs::symlink_file;
        symlink_file(src, dst)
    }

    // These tests are rather uninteresting. The really interesting tests
    // would stress the edge cases. On Unix, this might be comparing two files
    // on different mount points with the same inode number. On Windows, this
    // might be comparing two files whose file indices are the same on file
    // systems where such things aren't guaranteed to be unique.
    //
    // Alas, I don't know how to create those environmental conditions. ---AG

    #[test]
    fn same_file_trivial() {
        let tdir = tmpdir();
        let dir = tdir.path();

        File::create(dir.join("a")).unwrap();
        assert!(is_same_file(dir.join("a"), dir.join("a")).unwrap());
    }

    #[test]
    fn same_dir_trivial() {
        let tdir = tmpdir();
        let dir = tdir.path();

        fs::create_dir(dir.join("a")).unwrap();
        assert!(is_same_file(dir.join("a"), dir.join("a")).unwrap());
    }

    #[test]
    fn not_same_file_trivial() {
        let tdir = tmpdir();
        let dir = tdir.path();

        File::create(dir.join("a")).unwrap();
        File::create(dir.join("b")).unwrap();
        assert!(!is_same_file(dir.join("a"), dir.join("b")).unwrap());
    }

    #[test]
    fn not_same_dir_trivial() {
        let tdir = tmpdir();
        let dir = tdir.path();

        fs::create_dir(dir.join("a")).unwrap();
        fs::create_dir(dir.join("b")).unwrap();
        assert!(!is_same_file(dir.join("a"), dir.join("b")).unwrap());
    }

    #[test]
    fn same_file_hard() {
        let tdir = tmpdir();
        let dir = tdir.path();

        File::create(dir.join("a")).unwrap();
        fs::hard_link(dir.join("a"), dir.join("alink")).unwrap();
        assert!(is_same_file(dir.join("a"), dir.join("alink")).unwrap());
    }

    #[test]
    fn same_file_soft() {
        let tdir = tmpdir();
        let dir = tdir.path();

        File::create(dir.join("a")).unwrap();
        soft_link_file(dir.join("a"), dir.join("alink")).unwrap();
        assert!(is_same_file(dir.join("a"), dir.join("alink")).unwrap());
    }

    #[test]
    fn same_dir_soft() {
        let tdir = tmpdir();
        let dir = tdir.path();

        fs::create_dir(dir.join("a")).unwrap();
        soft_link_dir(dir.join("a"), dir.join("alink")).unwrap();
        assert!(is_same_file(dir.join("a"), dir.join("alink")).unwrap());
    }

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<super::Handle>();
    }

    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<super::Handle>();
    }
}
