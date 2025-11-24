use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, FileType};
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::Result;

/// A directory entry.
///
/// This is the type of value that is yielded from the iterators defined in
/// this crate.
///
/// On Unix systems, this type implements the [`DirEntryExt`] trait, which
/// provides efficient access to the inode number of the directory entry.
///
/// # Differences with `std::fs::DirEntry`
///
/// This type mostly mirrors the type by the same name in [`std::fs`]. There
/// are some differences however:
///
/// * All recursive directory iterators must inspect the entry's type.
/// Therefore, the value is stored and its access is guaranteed to be cheap and
/// successful.
/// * [`path`] and [`file_name`] return borrowed variants.
/// * If [`follow_links`] was enabled on the originating iterator, then all
/// operations except for [`path`] operate on the link target. Otherwise, all
/// operations operate on the symbolic link.
///
/// [`std::fs`]: https://doc.rust-lang.org/stable/std/fs/index.html
/// [`path`]: #method.path
/// [`file_name`]: #method.file_name
/// [`follow_links`]: struct.WalkDir.html#method.follow_links
/// [`DirEntryExt`]: trait.DirEntryExt.html
pub struct DirEntry {
    /// The path as reported by the [`fs::ReadDir`] iterator (even if it's a
    /// symbolic link).
    ///
    /// [`fs::ReadDir`]: https://doc.rust-lang.org/stable/std/fs/struct.ReadDir.html
    path: PathBuf,
    /// The file type. Necessary for recursive iteration, so store it.
    ty: FileType,
    /// Is set when this entry was created from a symbolic link and the user
    /// expects the iterator to follow symbolic links.
    follow_link: bool,
    /// The depth at which this entry was generated relative to the root.
    depth: usize,
    /// The underlying inode number (Unix only).
    #[cfg(unix)]
    ino: u64,
    /// The underlying metadata (Windows only). We store this on Windows
    /// because this comes for free while reading a directory.
    ///
    /// We use this to determine whether an entry is a directory or not, which
    /// works around a bug in Rust's standard library:
    /// https://github.com/rust-lang/rust/issues/46484
    #[cfg(windows)]
    metadata: fs::Metadata,
}

impl DirEntry {
    /// The full path that this entry represents.
    ///
    /// The full path is created by joining the parents of this entry up to the
    /// root initially given to [`WalkDir::new`] with the file name of this
    /// entry.
    ///
    /// Note that this *always* returns the path reported by the underlying
    /// directory entry, even when symbolic links are followed. To get the
    /// target path, use [`path_is_symlink`] to (cheaply) check if this entry
    /// corresponds to a symbolic link, and [`std::fs::read_link`] to resolve
    /// the target.
    ///
    /// [`WalkDir::new`]: struct.WalkDir.html#method.new
    /// [`path_is_symlink`]: struct.DirEntry.html#method.path_is_symlink
    /// [`std::fs::read_link`]: https://doc.rust-lang.org/stable/std/fs/fn.read_link.html
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The full path that this entry represents.
    ///
    /// Analogous to [`path`], but moves ownership of the path.
    ///
    /// [`path`]: struct.DirEntry.html#method.path
    pub fn into_path(self) -> PathBuf {
        self.path
    }

    /// Returns `true` if and only if this entry was created from a symbolic
    /// link. This is unaffected by the [`follow_links`] setting.
    ///
    /// When `true`, the value returned by the [`path`] method is a
    /// symbolic link name. To get the full target path, you must call
    /// [`std::fs::read_link(entry.path())`].
    ///
    /// [`path`]: struct.DirEntry.html#method.path
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    /// [`std::fs::read_link(entry.path())`]: https://doc.rust-lang.org/stable/std/fs/fn.read_link.html
    pub fn path_is_symlink(&self) -> bool {
        self.ty.is_symlink() || self.follow_link
    }

    /// Return the metadata for the file that this entry points to.
    ///
    /// This will follow symbolic links if and only if the [`WalkDir`] value
    /// has [`follow_links`] enabled.
    ///
    /// # Platform behavior
    ///
    /// This always calls [`std::fs::symlink_metadata`].
    ///
    /// If this entry is a symbolic link and [`follow_links`] is enabled, then
    /// [`std::fs::metadata`] is called instead.
    ///
    /// # Errors
    ///
    /// Similar to [`std::fs::metadata`], returns errors for path values that
    /// the program does not have permissions to access or if the path does not
    /// exist.
    ///
    /// [`WalkDir`]: struct.WalkDir.html
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    /// [`std::fs::metadata`]: https://doc.rust-lang.org/std/fs/fn.metadata.html
    /// [`std::fs::symlink_metadata`]: https://doc.rust-lang.org/stable/std/fs/fn.symlink_metadata.html
    pub fn metadata(&self) -> Result<fs::Metadata> {
        self.metadata_internal()
    }

    #[cfg(windows)]
    fn metadata_internal(&self) -> Result<fs::Metadata> {
        if self.follow_link {
            fs::metadata(&self.path)
        } else {
            Ok(self.metadata.clone())
        }
        .map_err(|err| Error::from_entry(self, err))
    }

    #[cfg(not(windows))]
    fn metadata_internal(&self) -> Result<fs::Metadata> {
        if self.follow_link {
            fs::metadata(&self.path)
        } else {
            fs::symlink_metadata(&self.path)
        }
        .map_err(|err| Error::from_entry(self, err))
    }

    /// Return the file type for the file that this entry points to.
    ///
    /// If this is a symbolic link and [`follow_links`] is `true`, then this
    /// returns the type of the target.
    ///
    /// This never makes any system calls.
    ///
    /// [`follow_links`]: struct.WalkDir.html#method.follow_links
    pub fn file_type(&self) -> fs::FileType {
        self.ty
    }

    /// Return the file name of this entry.
    ///
    /// If this entry has no file name (e.g., `/`), then the full path is
    /// returned.
    pub fn file_name(&self) -> &OsStr {
        self.path.file_name().unwrap_or_else(|| self.path.as_os_str())
    }

    /// Returns the depth at which this entry was created relative to the root.
    ///
    /// The smallest depth is `0` and always corresponds to the path given
    /// to the `new` function on `WalkDir`. Its direct descendents have depth
    /// `1`, and their descendents have depth `2`, and so on.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Returns true if and only if this entry points to a directory.
    pub(crate) fn is_dir(&self) -> bool {
        self.ty.is_dir()
    }

    #[cfg(windows)]
    pub(crate) fn from_entry(
        depth: usize,
        ent: &fs::DirEntry,
    ) -> Result<DirEntry> {
        let path = ent.path();
        let ty = ent
            .file_type()
            .map_err(|err| Error::from_path(depth, path.clone(), err))?;
        let md = ent
            .metadata()
            .map_err(|err| Error::from_path(depth, path.clone(), err))?;
        Ok(DirEntry { path, ty, follow_link: false, depth, metadata: md })
    }

    #[cfg(unix)]
    pub(crate) fn from_entry(
        depth: usize,
        ent: &fs::DirEntry,
    ) -> Result<DirEntry> {
        use std::os::unix::fs::DirEntryExt;

        let ty = ent
            .file_type()
            .map_err(|err| Error::from_path(depth, ent.path(), err))?;
        Ok(DirEntry {
            path: ent.path(),
            ty,
            follow_link: false,
            depth,
            ino: ent.ino(),
        })
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn from_entry(
        depth: usize,
        ent: &fs::DirEntry,
    ) -> Result<DirEntry> {
        let ty = ent
            .file_type()
            .map_err(|err| Error::from_path(depth, ent.path(), err))?;
        Ok(DirEntry { path: ent.path(), ty, follow_link: false, depth })
    }

    #[cfg(windows)]
    pub(crate) fn from_path(
        depth: usize,
        pb: PathBuf,
        follow: bool,
    ) -> Result<DirEntry> {
        let md = if follow {
            fs::metadata(&pb)
                .map_err(|err| Error::from_path(depth, pb.clone(), err))?
        } else {
            fs::symlink_metadata(&pb)
                .map_err(|err| Error::from_path(depth, pb.clone(), err))?
        };
        Ok(DirEntry {
            path: pb,
            ty: md.file_type(),
            follow_link: follow,
            depth,
            metadata: md,
        })
    }

    #[cfg(unix)]
    pub(crate) fn from_path(
        depth: usize,
        pb: PathBuf,
        follow: bool,
    ) -> Result<DirEntry> {
        use std::os::unix::fs::MetadataExt;

        let md = if follow {
            fs::metadata(&pb)
                .map_err(|err| Error::from_path(depth, pb.clone(), err))?
        } else {
            fs::symlink_metadata(&pb)
                .map_err(|err| Error::from_path(depth, pb.clone(), err))?
        };
        Ok(DirEntry {
            path: pb,
            ty: md.file_type(),
            follow_link: follow,
            depth,
            ino: md.ino(),
        })
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn from_path(
        depth: usize,
        pb: PathBuf,
        follow: bool,
    ) -> Result<DirEntry> {
        let md = if follow {
            fs::metadata(&pb)
                .map_err(|err| Error::from_path(depth, pb.clone(), err))?
        } else {
            fs::symlink_metadata(&pb)
                .map_err(|err| Error::from_path(depth, pb.clone(), err))?
        };
        Ok(DirEntry {
            path: pb,
            ty: md.file_type(),
            follow_link: follow,
            depth,
        })
    }
}

impl Clone for DirEntry {
    #[cfg(windows)]
    fn clone(&self) -> DirEntry {
        DirEntry {
            path: self.path.clone(),
            ty: self.ty,
            follow_link: self.follow_link,
            depth: self.depth,
            metadata: self.metadata.clone(),
        }
    }

    #[cfg(unix)]
    fn clone(&self) -> DirEntry {
        DirEntry {
            path: self.path.clone(),
            ty: self.ty,
            follow_link: self.follow_link,
            depth: self.depth,
            ino: self.ino,
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn clone(&self) -> DirEntry {
        DirEntry {
            path: self.path.clone(),
            ty: self.ty,
            follow_link: self.follow_link,
            depth: self.depth,
        }
    }
}

impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DirEntry({:?})", self.path)
    }
}

/// Unix-specific extension methods for `walkdir::DirEntry`
#[cfg(unix)]
pub trait DirEntryExt {
    /// Returns the underlying `d_ino` field in the contained `dirent`
    /// structure.
    fn ino(&self) -> u64;
}

#[cfg(unix)]
impl DirEntryExt for DirEntry {
    /// Returns the underlying `d_ino` field in the contained `dirent`
    /// structure.
    fn ino(&self) -> u64 {
        self.ino
    }
}
