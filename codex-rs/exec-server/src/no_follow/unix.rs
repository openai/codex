use crate::FileMetadata;
use rustix::fs::AtFlags;
use rustix::fs::Mode;
use rustix::fs::OFlags;
use rustix::fs::Stat;
#[cfg(target_os = "linux")]
use rustix::fs::Statx;
#[cfg(target_os = "linux")]
use rustix::fs::StatxFlags;
use rustix::fs::fstat;
use rustix::fs::mkdirat;
use rustix::fs::open;
use rustix::fs::openat;
use rustix::fs::statat;
#[cfg(target_os = "linux")]
use rustix::fs::statx;
use rustix::fs::unlinkat;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn directory_access_flags() -> OFlags {
    OFlags::PATH
}

#[cfg(target_vendor = "apple")]
fn directory_access_flags() -> OFlags {
    OFlags::from_bits_retain(libc::O_SEARCH as u32)
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn directory_access_flags() -> OFlags {
    OFlags::RDONLY
}

fn components(path: &Path) -> io::Result<Vec<OsString>> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no-follow filesystem operations require an absolute path",
        ));
    }
    path.components()
        .filter_map(|component| match component {
            Component::RootDir => None,
            Component::Normal(component) => Some(Ok(component.to_os_string())),
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                Some(Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no-follow filesystem operations require a normalized path",
                )))
            }
        })
        .collect()
}

fn root() -> io::Result<OwnedFd> {
    open(
        "/",
        directory_access_flags() | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)
}

fn open_directory(parent: &OwnedFd, name: &OsString) -> io::Result<OwnedFd> {
    openat(
        parent,
        name,
        directory_access_flags() | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)
}

fn parent(path: &Path) -> io::Result<(OwnedFd, OsString)> {
    let mut components = components(path)?;
    let leaf = components
        .pop()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path must name an entry"))?;
    let mut directory = root()?;
    for component in components {
        directory = open_directory(&directory, &component)?;
    }
    Ok((directory, leaf))
}

fn open_entry_sync(path: &Path) -> io::Result<std::fs::File> {
    let (parent, leaf) = parent(path)?;
    let file = openat(
        &parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    Ok(std::fs::File::from(file))
}

fn open_file_sync(path: &Path) -> io::Result<std::fs::File> {
    let file = open_entry_sync(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not a regular file",
        ));
    }
    Ok(file)
}

pub(super) async fn open_file(path: PathBuf) -> io::Result<tokio::fs::File> {
    tokio::task::spawn_blocking(move || open_file_sync(&path).map(tokio::fs::File::from_std))
        .await
        .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

pub(super) async fn write_file(path: PathBuf, contents: Vec<u8>) -> io::Result<()> {
    tokio::task::spawn_blocking(move || {
        let (parent, leaf) = parent(&path)?;
        // Prevent FIFOs and devices from blocking during open before the
        // descriptor can be validated as a regular file below.
        let file = openat(
            &parent,
            leaf,
            OFlags::WRONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o666),
        )
        .map_err(io::Error::from)?;
        let mut file = std::fs::File::from(file);
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path is not a regular file",
            ));
        }
        file.set_len(0)?;
        file.write_all(&contents)
    })
    .await
    .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

fn metadata_sync(path: &Path) -> io::Result<FileMetadata> {
    let components = components(path)?;

    #[cfg(target_os = "linux")]
    {
        let (parent, leaf, flags) = if components.is_empty() {
            (root()?, OsString::new(), AtFlags::EMPTY_PATH)
        } else {
            let (parent, leaf) = parent(path)?;
            (parent, leaf, AtFlags::SYMLINK_NOFOLLOW)
        };
        let metadata = statx(
            &parent,
            &leaf,
            flags,
            StatxFlags::BASIC_STATS | StatxFlags::BTIME,
        );
        match metadata {
            Ok(metadata) => file_metadata_linux(metadata),
            Err(error) if error == rustix::io::Errno::NOSYS || error == rustix::io::Errno::PERM => {
                let metadata = if components.is_empty() {
                    fstat(&parent)
                } else {
                    statat(&parent, &leaf, AtFlags::SYMLINK_NOFOLLOW)
                }
                .map_err(io::Error::from)?;
                file_metadata(metadata, /*created_at_ms*/ 0)
            }
            Err(error) => Err(io::Error::from(error)),
        }
    }

    #[cfg(not(target_os = "linux"))]
    if components.is_empty() {
        let root = root()?;
        let metadata = fstat(&root).map_err(io::Error::from)?;
        let created_at_ms = created_at_ms(&metadata);
        file_metadata(metadata, created_at_ms)
    } else {
        let (parent, leaf) = parent(path)?;
        let metadata =
            statat(&parent, &leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        let created_at_ms = created_at_ms(&metadata);
        file_metadata(metadata, created_at_ms)
    }
}

#[cfg(target_os = "linux")]
fn file_metadata_linux(metadata: Statx) -> io::Result<FileMetadata> {
    let kind = u32::from(metadata.stx_mode) & libc::S_IFMT;
    if kind == libc::S_IFLNK {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a symbolic link",
        ));
    }
    let created_at_ms = if metadata.stx_mask & StatxFlags::BTIME.bits() == 0 {
        0
    } else {
        unix_time_ms(metadata.stx_btime.tv_sec, metadata.stx_btime.tv_nsec)
    };
    Ok(FileMetadata {
        is_directory: kind == libc::S_IFDIR,
        is_file: kind == libc::S_IFREG,
        is_symlink: false,
        size: metadata.stx_size,
        created_at_ms,
        modified_at_ms: unix_time_ms(metadata.stx_mtime.tv_sec, metadata.stx_mtime.tv_nsec),
    })
}

fn file_metadata(metadata: Stat, created_at_ms: i64) -> io::Result<FileMetadata> {
    let kind = metadata.st_mode & libc::S_IFMT;
    if kind == libc::S_IFLNK {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a symbolic link",
        ));
    }
    Ok(FileMetadata {
        is_directory: kind == libc::S_IFDIR,
        is_file: kind == libc::S_IFREG,
        is_symlink: false,
        size: u64::try_from(metadata.st_size).unwrap_or(0),
        created_at_ms,
        modified_at_ms: unix_time_ms(metadata.st_mtime, metadata.st_mtime_nsec),
    })
}

#[cfg(target_vendor = "apple")]
fn created_at_ms(metadata: &Stat) -> i64 {
    unix_time_ms(metadata.st_birthtime, metadata.st_birthtime_nsec)
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
fn created_at_ms(_metadata: &Stat) -> i64 {
    0
}

fn unix_time_ms(seconds: i64, nanoseconds: impl TryInto<i64>) -> i64 {
    let nanoseconds = nanoseconds.try_into().unwrap_or_default();
    seconds
        .saturating_mul(1_000)
        .saturating_add(nanoseconds / 1_000_000)
}

pub(super) async fn metadata(path: PathBuf) -> io::Result<FileMetadata> {
    tokio::task::spawn_blocking(move || metadata_sync(&path))
        .await
        .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

pub(super) async fn create_directory(path: PathBuf, recursive: bool) -> io::Result<()> {
    tokio::task::spawn_blocking(move || {
        let components = components(&path)?;
        if components.is_empty() && !recursive {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "directory already exists",
            ));
        }
        let mut directory = root()?;
        for (index, component) in components.iter().enumerate() {
            let is_leaf = index + 1 == components.len();
            if !recursive && is_leaf {
                mkdirat(&directory, component, Mode::from_raw_mode(0o777))
                    .map_err(io::Error::from)?;
                return Ok(());
            }
            match open_directory(&directory, component) {
                Ok(next) => directory = next,
                Err(error) if recursive && error.kind() == io::ErrorKind::NotFound => {
                    match mkdirat(&directory, component, Mode::from_raw_mode(0o777)) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                        Err(error) => return Err(io::Error::from(error)),
                    }
                    directory = open_directory(&directory, component)?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

pub(super) async fn remove(path: PathBuf, recursive: bool, force: bool) -> io::Result<()> {
    tokio::task::spawn_blocking(move || {
        if recursive {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "recursive no-follow removal is unsupported",
            ));
        }
        let (parent, leaf) = match parent(&path) {
            Ok(value) => value,
            Err(error) if force && error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let metadata = match statat(&parent, &leaf, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => metadata,
            Err(error) if force && error == rustix::io::Errno::NOENT => return Ok(()),
            Err(error) => return Err(io::Error::from(error)),
        };
        let kind = metadata.st_mode & libc::S_IFMT;
        if kind == libc::S_IFLNK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains a symbolic link",
            ));
        }
        let flags = if kind == libc::S_IFDIR {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        unlinkat(&parent, leaf, flags).map_err(io::Error::from)
    })
    .await
    .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}
