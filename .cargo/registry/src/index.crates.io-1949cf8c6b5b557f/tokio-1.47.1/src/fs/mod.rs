#![cfg(not(loom))]

//! Asynchronous file utilities.
//!
//! This module contains utility methods for working with the file system
//! asynchronously. This includes reading/writing to files, and working with
//! directories.
//!
//! Be aware that most operating systems do not provide asynchronous file system
//! APIs. Because of that, Tokio will use ordinary blocking file operations
//! behind the scenes. This is done using the [`spawn_blocking`] threadpool to
//! run them in the background.
//!
//! The `tokio::fs` module should only be used for ordinary files. Trying to use
//! it with e.g., a named pipe on Linux can result in surprising behavior,
//! such as hangs during runtime shutdown. For special files, you should use a
//! dedicated type such as [`tokio::net::unix::pipe`] or [`AsyncFd`] instead.
//!
//! Currently, Tokio will always use [`spawn_blocking`] on all platforms, but it
//! may be changed to use asynchronous file system APIs such as io_uring in the
//! future.
//!
//! # Usage
//!
//! The easiest way to use this module is to use the utility functions that
//! operate on entire files:
//!
//!  * [`tokio::fs::read`](fn@crate::fs::read)
//!  * [`tokio::fs::read_to_string`](fn@crate::fs::read_to_string)
//!  * [`tokio::fs::write`](fn@crate::fs::write)
//!
//! The two `read` functions reads the entire file and returns its contents.
//! The `write` function takes the contents of the file and writes those
//! contents to the file. It overwrites the existing file, if any.
//!
//! For example, to read the file:
//!
//! ```
//! # async fn dox() -> std::io::Result<()> {
//! let contents = tokio::fs::read_to_string("my_file.txt").await?;
//!
//! println!("File has {} lines.", contents.lines().count());
//! # Ok(())
//! # }
//! ```
//!
//! To overwrite the file:
//!
//! ```
//! # async fn dox() -> std::io::Result<()> {
//! let contents = "First line.\nSecond line.\nThird line.\n";
//!
//! tokio::fs::write("my_file.txt", contents.as_bytes()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Using `File`
//!
//! The main type for interacting with files is [`File`]. It can be used to read
//! from and write to a given file. This is done using the [`AsyncRead`] and
//! [`AsyncWrite`] traits. This type is generally used when you want to do
//! something more complex than just reading or writing the entire contents in
//! one go.
//!
//! **Note:** It is important to use [`flush`] when writing to a Tokio
//! [`File`]. This is because calls to `write` will return before the write has
//! finished, and [`flush`] will wait for the write to finish. (The write will
//! happen even if you don't flush; it will just happen later.) This is
//! different from [`std::fs::File`], and is due to the fact that `File` uses
//! `spawn_blocking` behind the scenes.
//!
//! For example, to count the number of lines in a file without loading the
//! entire file into memory:
//!
//! ```no_run
//! use tokio::fs::File;
//! use tokio::io::AsyncReadExt;
//!
//! # async fn dox() -> std::io::Result<()> {
//! let mut file = File::open("my_file.txt").await?;
//!
//! let mut chunk = vec![0; 4096];
//! let mut number_of_lines = 0;
//! loop {
//!     let len = file.read(&mut chunk).await?;
//!     if len == 0 {
//!         // Length of zero means end of file.
//!         break;
//!     }
//!     for &b in &chunk[..len] {
//!         if b == b'\n' {
//!             number_of_lines += 1;
//!         }
//!     }
//! }
//!
//! println!("File has {} lines.", number_of_lines);
//! # Ok(())
//! # }
//! ```
//!
//! For example, to write a file line-by-line:
//!
//! ```no_run
//! use tokio::fs::File;
//! use tokio::io::AsyncWriteExt;
//!
//! # async fn dox() -> std::io::Result<()> {
//! let mut file = File::create("my_file.txt").await?;
//!
//! file.write_all(b"First line.\n").await?;
//! file.write_all(b"Second line.\n").await?;
//! file.write_all(b"Third line.\n").await?;
//!
//! // Remember to call `flush` after writing!
//! file.flush().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Tuning your file IO
//!
//! Tokio's file uses [`spawn_blocking`] behind the scenes, and this has serious
//! performance consequences. To get good performance with file IO on Tokio, it
//! is recommended to batch your operations into as few `spawn_blocking` calls
//! as possible.
//!
//! One example of this difference can be seen by comparing the two reading
//! examples above. The first example uses [`tokio::fs::read`], which reads the
//! entire file in a single `spawn_blocking` call, and then returns it. The
//! second example will read the file in chunks using many `spawn_blocking`
//! calls. This means that the second example will most likely be more expensive
//! for large files. (Of course, using chunks may be necessary for very large
//! files that don't fit in memory.)
//!
//! The following examples will show some strategies for this:
//!
//! When creating a file, write the data to a `String` or `Vec<u8>` and then
//! write the entire file in a single `spawn_blocking` call with
//! `tokio::fs::write`.
//!
//! ```no_run
//! # async fn dox() -> std::io::Result<()> {
//! let mut contents = String::new();
//!
//! contents.push_str("First line.\n");
//! contents.push_str("Second line.\n");
//! contents.push_str("Third line.\n");
//!
//! tokio::fs::write("my_file.txt", contents.as_bytes()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Use [`BufReader`] and [`BufWriter`] to buffer many small reads or writes
//! into a few large ones. This example will most likely only perform one
//! `spawn_blocking` call.
//!
//! ```no_run
//! use tokio::fs::File;
//! use tokio::io::{AsyncWriteExt, BufWriter};
//!
//! # async fn dox() -> std::io::Result<()> {
//! let mut file = BufWriter::new(File::create("my_file.txt").await?);
//!
//! file.write_all(b"First line.\n").await?;
//! file.write_all(b"Second line.\n").await?;
//! file.write_all(b"Third line.\n").await?;
//!
//! // Due to the BufWriter, the actual write and spawn_blocking
//! // call happens when you flush.
//! file.flush().await?;
//! # Ok(())
//! # }
//! ```
//!
//! Manually use [`std::fs`] inside [`spawn_blocking`].
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::{self, Write};
//! use tokio::task::spawn_blocking;
//!
//! # async fn dox() -> std::io::Result<()> {
//! spawn_blocking(move || {
//!     let mut file = File::create("my_file.txt")?;
//!
//!     file.write_all(b"First line.\n")?;
//!     file.write_all(b"Second line.\n")?;
//!     file.write_all(b"Third line.\n")?;
//!
//!     // Unlike Tokio's file, the std::fs file does
//!     // not need flush.
//!
//!     io::Result::Ok(())
//! }).await.unwrap()?;
//! # Ok(())
//! # }
//! ```
//!
//! It's also good to be aware of [`File::set_max_buf_size`], which controls the
//! maximum amount of bytes that Tokio's [`File`] will read or write in a single
//! [`spawn_blocking`] call. The default is two megabytes, but this is subject
//! to change.
//!
//! [`spawn_blocking`]: fn@crate::task::spawn_blocking
//! [`AsyncRead`]: trait@crate::io::AsyncRead
//! [`AsyncWrite`]: trait@crate::io::AsyncWrite
//! [`BufReader`]: struct@crate::io::BufReader
//! [`BufWriter`]: struct@crate::io::BufWriter
//! [`tokio::net::unix::pipe`]: crate::net::unix::pipe
//! [`AsyncFd`]: crate::io::unix::AsyncFd
//! [`flush`]: crate::io::AsyncWriteExt::flush
//! [`tokio::fs::read`]: fn@crate::fs::read

mod canonicalize;
pub use self::canonicalize::canonicalize;

mod create_dir;
pub use self::create_dir::create_dir;

mod create_dir_all;
pub use self::create_dir_all::create_dir_all;

mod dir_builder;
pub use self::dir_builder::DirBuilder;

mod file;
pub use self::file::File;

mod hard_link;
pub use self::hard_link::hard_link;

mod metadata;
pub use self::metadata::metadata;

mod open_options;
pub use self::open_options::OpenOptions;

mod read;
pub use self::read::read;

mod read_dir;
pub use self::read_dir::{read_dir, DirEntry, ReadDir};

mod read_link;
pub use self::read_link::read_link;

mod read_to_string;
pub use self::read_to_string::read_to_string;

mod remove_dir;
pub use self::remove_dir::remove_dir;

mod remove_dir_all;
pub use self::remove_dir_all::remove_dir_all;

mod remove_file;
pub use self::remove_file::remove_file;

mod rename;
pub use self::rename::rename;

mod set_permissions;
pub use self::set_permissions::set_permissions;

mod symlink_metadata;
pub use self::symlink_metadata::symlink_metadata;

mod write;
pub use self::write::write;

mod copy;
pub use self::copy::copy;

mod try_exists;
pub use self::try_exists::try_exists;

#[cfg(test)]
mod mocks;

feature! {
    #![unix]

    mod symlink;
    pub use self::symlink::symlink;
}

cfg_windows! {
    mod symlink_dir;
    pub use self::symlink_dir::symlink_dir;

    mod symlink_file;
    pub use self::symlink_file::symlink_file;
}

use std::io;

#[cfg(not(test))]
use crate::blocking::spawn_blocking;
#[cfg(test)]
use mocks::spawn_blocking;

pub(crate) async fn asyncify<F, T>(f: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    match spawn_blocking(f).await {
        Ok(res) => res,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::Other,
            "background task failed",
        )),
    }
}
