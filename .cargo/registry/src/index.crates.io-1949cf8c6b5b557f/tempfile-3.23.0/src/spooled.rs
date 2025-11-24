use crate::file::tempfile;
use crate::tempfile_in;
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A wrapper for the two states of a [`SpooledTempFile`]. Either:
///
/// 1. An in-memory [`Cursor`] representing the state of the file.
/// 2. A temporary [`File`].
#[derive(Debug)]
pub enum SpooledData {
    InMemory(Cursor<Vec<u8>>),
    OnDisk(File),
}

/// An object that behaves like a regular temporary file, but keeps data in
/// memory until it reaches a configured size, at which point the data is
/// written to a temporary file on disk, and further operations use the file
/// on disk.
#[derive(Debug)]
pub struct SpooledTempFile {
    max_size: usize,
    dir: Option<PathBuf>,
    inner: SpooledData,
}

/// Create a new [`SpooledTempFile`]. Also see [`spooled_tempfile_in`].
///
/// # Security
///
/// This variant is secure/reliable in the presence of a pathological temporary
/// file cleaner.
///
/// # Backing Storage
///
/// By default, the underlying temporary file will be created in your operating system's temporary
/// file directory which is _often_ an in-memory filesystem. You may want to consider using
/// [`spooled_tempfile_in`] instead, passing a storage-backed filesystem (e.g., `/var/tmp` on
/// Linux).
///
/// # Resource Leaking
///
/// The temporary file will be automatically removed by the OS when the last
/// handle to it is closed. This doesn't rely on Rust destructors being run, so
/// will (almost) never fail to clean up the temporary file.
///
/// # Examples
///
/// ```
/// use tempfile::spooled_tempfile;
/// use std::io::Write;
///
/// let mut file = spooled_tempfile(15);
///
/// writeln!(file, "short line")?;
/// assert!(!file.is_rolled());
///
/// // as a result of this write call, the size of the data will exceed
/// // `max_size` (15), so it will be written to a temporary file on disk,
/// // and the in-memory buffer will be dropped
/// writeln!(file, "marvin gardens")?;
/// assert!(file.is_rolled());
/// # Ok::<(), std::io::Error>(())
/// ```
#[inline]
pub fn spooled_tempfile(max_size: usize) -> SpooledTempFile {
    SpooledTempFile::new(max_size)
}

/// Construct a new [`SpooledTempFile`], backed by a file in the specified directory. Use this when,
/// e.g., you need the temporary file to be backed by a specific filesystem (e.g., when your default
/// temporary directory is in-memory). Also see [`spooled_tempfile`].
///
/// **NOTE:** The specified path isn't checked until the temporary file is "rolled over" into a real
/// temporary file. If the specified directory isn't writable, writes to the temporary file will
/// fail once the `max_size` is reached.
#[inline]
pub fn spooled_tempfile_in<P: AsRef<Path>>(max_size: usize, dir: P) -> SpooledTempFile {
    SpooledTempFile::new_in(max_size, dir)
}

/// Write a cursor into a temporary file, returning the temporary file.
fn cursor_to_tempfile(cursor: &Cursor<Vec<u8>>, p: &Option<PathBuf>) -> io::Result<File> {
    let mut file = match p {
        Some(p) => tempfile_in(p)?,
        None => tempfile()?,
    };
    file.write_all(cursor.get_ref())?;
    file.seek(SeekFrom::Start(cursor.position()))?;
    Ok(file)
}

impl SpooledTempFile {
    /// Construct a new [`SpooledTempFile`].
    #[must_use]
    pub fn new(max_size: usize) -> SpooledTempFile {
        SpooledTempFile {
            max_size,
            dir: None,
            inner: SpooledData::InMemory(Cursor::new(Vec::new())),
        }
    }

    /// Construct a new [`SpooledTempFile`], backed by a file in the specified directory.
    #[must_use]
    pub fn new_in<P: AsRef<Path>>(max_size: usize, dir: P) -> SpooledTempFile {
        SpooledTempFile {
            max_size,
            dir: Some(dir.as_ref().to_owned()),
            inner: SpooledData::InMemory(Cursor::new(Vec::new())),
        }
    }

    /// Returns true if the file has been rolled over to disk.
    #[must_use]
    pub fn is_rolled(&self) -> bool {
        match self.inner {
            SpooledData::InMemory(_) => false,
            SpooledData::OnDisk(_) => true,
        }
    }

    /// Rolls over to a file on disk, regardless of current size. Does nothing
    /// if already rolled over.
    pub fn roll(&mut self) -> io::Result<()> {
        if let SpooledData::InMemory(cursor) = &mut self.inner {
            self.inner = SpooledData::OnDisk(cursor_to_tempfile(cursor, &self.dir)?);
        }
        Ok(())
    }

    /// Truncate the file to the specified size.
    pub fn set_len(&mut self, size: u64) -> Result<(), io::Error> {
        if size > self.max_size as u64 {
            self.roll()?; // does nothing if already rolled over
        }
        match &mut self.inner {
            SpooledData::InMemory(cursor) => {
                cursor.get_mut().resize(size as usize, 0);
                Ok(())
            }
            SpooledData::OnDisk(file) => file.set_len(size),
        }
    }

    /// Consumes and returns the inner `SpooledData` type.
    #[must_use]
    pub fn into_inner(self) -> SpooledData {
        self.inner
    }

    /// Convert into a regular unnamed temporary file, writing it to disk if necessary.
    pub fn into_file(self) -> io::Result<File> {
        match self.inner {
            SpooledData::InMemory(cursor) => cursor_to_tempfile(&cursor, &self.dir),
            SpooledData::OnDisk(file) => Ok(file),
        }
    }
}

impl Read for SpooledTempFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.read(buf),
            SpooledData::OnDisk(file) => file.read(buf),
        }
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.read_vectored(bufs),
            SpooledData::OnDisk(file) => file.read_vectored(bufs),
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.read_to_end(buf),
            SpooledData::OnDisk(file) => file.read_to_end(buf),
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.read_to_string(buf),
            SpooledData::OnDisk(file) => file.read_to_string(buf),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.read_exact(buf),
            SpooledData::OnDisk(file) => file.read_exact(buf),
        }
    }
}

impl Write for SpooledTempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // roll over to file if necessary
        if matches! {
            &self.inner, SpooledData::InMemory(cursor)
            if cursor.position().saturating_add(buf.len() as u64) > self.max_size as u64
        } {
            self.roll()?;
        }

        // write the bytes
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.write(buf),
            SpooledData::OnDisk(file) => file.write(buf),
        }
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        if matches! {
            &self.inner, SpooledData::InMemory(cursor)
            // Borrowed from the rust standard library.
            if bufs
                .iter()
                .fold(cursor.position(), |a, b| a.saturating_add(b.len() as u64))
                > self.max_size as u64
        } {
            self.roll()?;
        }
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.write_vectored(bufs),
            SpooledData::OnDisk(file) => file.write_vectored(bufs),
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.flush(),
            SpooledData::OnDisk(file) => file.flush(),
        }
    }
}

impl Seek for SpooledTempFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match &mut self.inner {
            SpooledData::InMemory(cursor) => cursor.seek(pos),
            SpooledData::OnDisk(file) => file.seek(pos),
        }
    }
}
