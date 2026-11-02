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

fn open_directory(parent: &OwnedFd, name: &std::ffi::OsStr) -> io::Result<OwnedFd> {
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

pub(super) fn inherited_output(path: &Path) -> io::Result<Option<super::BoundOutput>> {
    use std::os::fd::FromRawFd;
    let name = path.to_str().unwrap_or_default();
    let fd = match name {
        "/dev/stdout" => libc::STDOUT_FILENO,
        "/dev/stderr" => libc::STDERR_FILENO,
        _ => {
            let number = name
                .strip_prefix("/dev/fd/")
                .or_else(|| name.strip_prefix("/proc/self/fd/"));
            let Some(fd) = number
                .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|s| s.parse().ok())
            else {
                return Ok(None);
            };
            fd
        }
    };
    // Only caller-supplied descriptors may be selected, including shell process substitution.
    // SAFETY: fcntl validates the integer descriptor; F_DUPFD_CLOEXEC returns a new owned fd.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(super::invalid_output("output descriptor is not inherited"));
    }
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if copy < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { std::fs::File::from_raw_fd(copy) };
    let access = rustix::fs::fcntl_getfl(&file)? & OFlags::ACCMODE;
    if !matches!(access, OFlags::WRONLY | OFlags::RDWR) {
        return Err(super::invalid_output("output descriptor is not writable"));
    }
    Ok(Some(super::BoundOutput {
        file,
        destination: super::Destination::Stream {
            is_stdout: fd == libc::STDOUT_FILENO,
        },
    }))
}

pub(super) fn reject_multiple_links(file: &std::fs::File) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if file.metadata()?.nlink() > 1 {
        return Err(super::invalid_output("output has multiple hard links"));
    }
    Ok(())
}

pub(super) fn output_path(path: &Path) -> io::Result<PathBuf> {
    // Resolve leading ".." against the caller's cwd, never against a movable directory fd.
    let mut suffix = path.strip_prefix(".").unwrap_or(path);
    let mut base = if suffix.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir()?
    };
    while let Ok(rest) = suffix.strip_prefix("..") {
        base.pop();
        suffix = rest;
    }
    let path = base.join(suffix);
    #[cfg(target_os = "macos")]
    {
        // Expand one known root link only. Never canonicalize its target or the caller's suffix.
        if let Some(physical) = ["tmp", "var", "etc"].into_iter().find_map(|name| {
            let root = Path::new("/").join(name);
            let suffix = path.strip_prefix(&root).ok()?;
            let expected = Path::new("private").join(name);
            (std::fs::read_link(root).ok()? == expected)
                .then(|| Path::new("/").join(expected).join(suffix))
        }) {
            return Ok(physical);
        }
    }
    Ok(path)
}

pub(super) fn verify_output_path(file: &std::fs::File, path: &Path) -> io::Result<()> {
    let (dir, leaf) = parent(path)?;
    let named = statat(dir, leaf, AtFlags::SYMLINK_NOFOLLOW)?;
    let bound = fstat(file)?;
    if (named.st_dev, named.st_ino) != (bound.st_dev, bound.st_ino) {
        return Err(super::invalid_output("output path was replaced"));
    }
    Ok(())
}

pub(super) fn open_output(path: &Path) -> io::Result<std::fs::File> {
    let (directory, leaf) = parent(path)?;
    let file = open_writable(&directory, &leaf)?;
    if !file.metadata()?.is_file() {
        let flags = rustix::fs::fcntl_getfl(&file)?;
        rustix::fs::fcntl_setfl(&file, flags & !OFlags::NONBLOCK)?;
    }
    Ok(file)
}

// Opening the leaf must not truncate it or hang on a FIFO without a reader.
fn open_writable(directory: &OwnedFd, leaf: &std::ffi::OsStr) -> io::Result<std::fs::File> {
    let file = openat(
        directory,
        leaf,
        OFlags::WRONLY
            | OFlags::CREATE
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NOCTTY
            | OFlags::NONBLOCK,
        Mode::from_raw_mode(0o666),
    )?;
    Ok(std::fs::File::from(file))
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

pub(super) fn open_file_sync(path: &Path) -> io::Result<std::fs::File> {
    let file = open_entry_sync(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not a regular file",
        ));
    }
    Ok(file)
}

pub(super) async fn write_file(path: PathBuf, contents: Vec<u8>) -> io::Result<()> {
    tokio::task::spawn_blocking(move || {
        let (directory, leaf) = parent(&path)?;
        let mut file = open_writable(&directory, &leaf)?;
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
